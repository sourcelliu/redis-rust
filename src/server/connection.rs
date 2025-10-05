// Connection handler

use crate::cluster::{ClusterState, MigrationManager};
use crate::commands::dispatcher::CommandDispatcher;
use crate::config::Config;
use crate::persistence::aof::AofManager;
use crate::protocol::{RespParser, RespSerializer, RespValue};
use crate::pubsub::PubSub;
use crate::replication::{ReplicationInfo, ReplicationBacklog, CommandPropagator};
use crate::scripting::ScriptCache;
use crate::server::client_info::ClientRegistry;
use crate::server::config::ServerConfig;
use crate::server::slowlog::SlowLog;
use crate::storage::db::Database;
use crate::transaction::Transaction;
use bytes::BytesMut;
use std::sync::Arc;
use std::time::Instant;
use tokio::io::{AsyncReadExt, AsyncWriteExt, BufWriter};
use tokio::net::TcpStream;
use tracing::{debug, error};

pub struct Connection {
    stream: BufWriter<TcpStream>,
    buffer: BytesMut,
    client_id: u64,
    db: Arc<Database>,
    pubsub: Arc<PubSub>,
    config: Arc<ServerConfig>,
    app_config: Arc<Config>,
    aof: Arc<AofManager>,
    script_cache: Arc<ScriptCache>,
    repl_info: Arc<ReplicationInfo>,
    repl_backlog: Arc<ReplicationBacklog>,
    propagator: Arc<CommandPropagator>,
    client_registry: Arc<ClientRegistry>,
    slowlog: Arc<SlowLog>,
    cluster: Arc<ClusterState>,
    migration: Arc<MigrationManager>,
    /// Current selected database (0-15)
    db_index: usize,
    /// Transaction state
    transaction: Transaction,
    /// ASKING flag for cluster redirection
    asking: bool,
}

impl Connection {
    pub fn new(
        socket: TcpStream,
        client_id: u64,
        db: Arc<Database>,
        pubsub: Arc<PubSub>,
        config: Arc<ServerConfig>,
        app_config: Arc<Config>,
        aof: Arc<AofManager>,
        script_cache: Arc<ScriptCache>,
        repl_info: Arc<ReplicationInfo>,
        repl_backlog: Arc<ReplicationBacklog>,
        propagator: Arc<CommandPropagator>,
        client_registry: Arc<ClientRegistry>,
        slowlog: Arc<SlowLog>,
        cluster: Arc<ClusterState>,
        migration: Arc<MigrationManager>,
    ) -> Self {
        Self {
            stream: BufWriter::new(socket),
            buffer: BytesMut::with_capacity(4096),
            client_id,
            db,
            pubsub,
            config,
            app_config,
            aof,
            script_cache,
            repl_info,
            repl_backlog,
            propagator,
            client_registry,
            slowlog,
            cluster,
            migration,
            db_index: 0,
            transaction: Transaction::new(),
            asking: false,
        }
    }

    /// Main processing loop for this connection
    pub async fn process(&mut self) -> anyhow::Result<()> {
        loop {
            // Try to parse a complete frame from buffer
            match self.parse_frame()? {
                Some(frame) => {
                    debug!("Received frame: {:?}", frame);
                    let response = self.handle_frame(frame).await;
                    self.write_response(response).await?;
                }
                None => {
                    // Need more data
                    if self.read_frame().await? == 0 {
                        // Connection closed by client
                        if self.buffer.is_empty() {
                            return Ok(());
                        } else {
                            return Err(anyhow::anyhow!("Connection reset by peer"));
                        }
                    }
                }
            }
        }
    }

    /// Try to parse a complete frame from the buffer
    fn parse_frame(&mut self) -> anyhow::Result<Option<RespValue>> {
        use crate::protocol::RespError;

        match RespParser::check_complete(&self.buffer) {
            Ok(Some(len)) => {
                let frame_data = self.buffer.split_to(len);
                let frame = RespParser::parse(&frame_data)?;
                Ok(Some(frame))
            }
            Ok(None) => Ok(None),
            Err(RespError::Incomplete) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    /// Read data from socket into buffer
    async fn read_frame(&mut self) -> anyhow::Result<usize> {
        let stream = self.stream.get_mut();
        let mut read_buf = [0u8; 4096];
        let n = stream.read(&mut read_buf).await?;
        if n > 0 {
            self.buffer.extend_from_slice(&read_buf[..n]);
        }
        Ok(n)
    }

    /// Handle a parsed frame and generate response
    async fn handle_frame(&mut self, frame: RespValue) -> RespValue {
        // Start timing
        let start = Instant::now();

        // Extract command and args from array
        let args = match frame {
            RespValue::Array(Some(arr)) if !arr.is_empty() => arr,
            _ => {
                return RespValue::Error("ERR invalid command format".to_string());
            }
        };

        // Convert args to byte vectors
        let mut cmd_args: Vec<Vec<u8>> = Vec::new();
        for arg in args {
            match arg {
                RespValue::BulkString(Some(data)) => cmd_args.push(data),
                RespValue::SimpleString(s) => cmd_args.push(s.into_bytes()),
                _ => {
                    return RespValue::Error("ERR invalid argument type".to_string());
                }
            }
        }

        if cmd_args.is_empty() {
            return RespValue::Error("ERR empty command".to_string());
        }

        // Track client activity
        let cmd_name = std::str::from_utf8(&cmd_args[0])
            .unwrap_or("unknown")
            .to_uppercase();
        self.client_registry.mark_activity(self.client_id, cmd_name.clone(), self.db_index);

        // Handle ASKING command (sets asking flag for next command)
        if cmd_name == "ASKING" {
            self.asking = true;
            return RespValue::SimpleString("OK".to_string());
        }

        // Handle CLUSTER commands directly (need access to cluster state)
        if cmd_name == "CLUSTER" {
            return self.handle_cluster_command(&cmd_args[1..]);
        }

        // Check cluster redirection before executing command (skip for CLUSTER commands)
        if self.cluster.enabled && !cmd_name.starts_with("COMMAND") {
            // Extract key from command for slot calculation
            if let Some(redirection_error) = self.check_cluster_redirection(&cmd_name, &cmd_args) {
                // Reset ASKING flag after using it
                self.asking = false;
                return redirection_error;
            }
        }

        // Reset ASKING flag after command (whether redirected or not)
        let asking_was_set = self.asking;
        self.asking = false;

        // Convert command args to strings for slow log
        let cmd_strings: Vec<String> = cmd_args
            .iter()
            .map(|arg| String::from_utf8_lossy(arg).to_string())
            .collect();

        // Determine if command should be logged to AOF
        let should_log_aof = self.should_log_to_aof(&cmd_args);

        // Dispatch command
        let dispatcher = CommandDispatcher::new();
        let response = dispatcher.dispatch(
            &mut self.db_index,
            &self.db,
            &self.pubsub,
            &self.aof,
            &self.script_cache,
            &self.repl_info,
            &self.repl_backlog,
            &self.propagator,
            &self.client_registry,
            self.client_id,
            &self.slowlog,
            &self.app_config,
            &mut self.transaction,
            cmd_args.clone(),
        ).await;

        // Log to AOF if command modifies data and succeeded
        if should_log_aof && self.is_success_response(&response) {
            if let Err(e) = self.aof.append(self.db_index, &cmd_args).await {
                error!("Failed to append to AOF: {}", e);
            }

            // Propagate to replicas if we're a master
            if self.repl_info.is_master() {
                let offset = self.repl_info.master_offset();
                self.propagator.propagate(self.db_index, &cmd_args, offset).await;
                self.repl_info.increment_offset(1);
            }
        }

        // Handle MULTI mode - queue commands instead of executing
        if self.transaction.in_multi {
            // Check if this is EXEC command
            if let RespValue::SimpleString(ref s) = response {
                if s == "__EXEC__" {
                    // Execute all queued commands
                    let commands = self.transaction.exec();
                    let mut results = Vec::new();

                    for queued_cmd in commands {
                        let dispatcher = CommandDispatcher::new();
                        let result = dispatcher.dispatch(
                            &mut self.db_index,
                            &self.db,
                            &self.pubsub,
                            &self.aof,
                            &self.script_cache,
                            &self.repl_info,
                            &self.repl_backlog,
                            &self.propagator,
                            &self.client_registry,
                            self.client_id,
                            &self.slowlog,
                            &self.app_config,
                            &mut self.transaction,
                            queued_cmd.clone(),
                        ).await;

                        // Log each executed command to AOF
                        if self.should_log_to_aof(&queued_cmd) && self.is_success_response(&result) {
                            if let Err(e) = self.aof.append(self.db_index, &queued_cmd).await {
                                error!("Failed to append to AOF: {}", e);
                            }

                            // Propagate to replicas if we're a master
                            if self.repl_info.is_master() {
                                let offset = self.repl_info.master_offset();
                                self.propagator.propagate(self.db_index, &queued_cmd, offset).await;
                                self.repl_info.increment_offset(1);
                            }
                        }

                        results.push(result);
                    }

                    self.transaction.unwatch();
                    return RespValue::Array(Some(results));
                }
            }

            // Queue the command
            self.transaction.queue_command(cmd_args);
            return RespValue::SimpleString("QUEUED".to_string());
        }

        // Log to slow log if needed
        let duration = start.elapsed();
        let client_addr = self.client_registry
            .get(self.client_id)
            .map(|c| c.addr)
            .unwrap_or_else(|| "unknown".to_string());
        let client_name = self.client_registry.get_name(self.client_id);
        self.slowlog.add_if_slow(duration, cmd_strings, client_addr, client_name);

        response
    }

    /// Check if a command should be logged to AOF
    fn should_log_to_aof(&self, args: &[Vec<u8>]) -> bool {
        if args.is_empty() {
            return false;
        }

        let cmd = String::from_utf8_lossy(&args[0]).to_uppercase();

        // Only log write commands, not read commands
        matches!(cmd.as_str(),
            // String write commands
            "SET" | "DEL" | "APPEND" | "INCR" | "DECR" | "INCRBY" | "DECRBY" |
            "SETRANGE" | "MSET" |
            // List write commands
            "LPUSH" | "RPUSH" | "LPOP" | "RPOP" | "LSET" | "LTRIM" |
            // Hash write commands
            "HSET" | "HDEL" |
            // Set write commands
            "SADD" | "SREM" | "SPOP" |
            // ZSet write commands
            "ZADD" | "ZREM" |
            // Expiration commands
            "EXPIRE" | "EXPIREAT" | "PEXPIRE" | "PEXPIREAT" | "PERSIST" |
            // Database commands
            "FLUSHDB" | "FLUSHALL" | "SELECT"
        )
    }

    /// Check if response indicates success
    fn is_success_response(&self, response: &RespValue) -> bool {
        !matches!(response, RespValue::Error(_))
    }

    /// Write response to client
    async fn write_response(&mut self, response: RespValue) -> anyhow::Result<()> {
        let data = RespSerializer::serialize(&response);
        self.stream.write_all(&data).await?;
        self.stream.flush().await?;
        Ok(())
    }

    pub fn current_db(&self) -> usize {
        self.db_index
    }

    pub fn select_db(&mut self, index: usize) -> Result<(), String> {
        if index >= self.config.databases {
            return Err(format!("ERR DB index out of range (0-{})", self.config.databases - 1));
        }
        self.db_index = index;
        Ok(())
    }

    /// Check if command needs cluster redirection
    /// Returns Some(error) if redirection is needed, None if command can execute locally
    fn check_cluster_redirection(&self, cmd_name: &str, cmd_args: &[Vec<u8>]) -> Option<RespValue> {
        use crate::cluster::{key_hash_slot, check_slot_ownership};

        // Skip commands that don't access keys
        let keyless_commands = [
            "PING", "ECHO", "SELECT", "FLUSHDB", "FLUSHALL", "DBSIZE",
            "INFO", "TIME", "LASTSAVE", "SAVE", "BGSAVE",
            "SHUTDOWN", "CLIENT", "CONFIG", "SLOWLOG", "ROLE",
            "MULTI", "EXEC", "DISCARD", "WATCH", "UNWATCH"
        ];

        if keyless_commands.contains(&cmd_name) {
            return None;
        }

        // Extract first key based on command
        let key_index = match cmd_name {
            // Commands where key is at position 1
            "GET" | "SET" | "DEL" | "EXISTS" | "EXPIRE" | "TTL" | "TYPE" |
            "APPEND" | "STRLEN" | "INCR" | "DECR" | "LPUSH" | "RPUSH" |
            "LPOP" | "RPOP" | "LLEN" | "HSET" | "HGET" | "HDEL" | "HLEN" |
            "SADD" | "SREM" | "SMEMBERS" | "SCARD" | "ZADD" | "ZREM" |
            "ZCARD" | "ZSCORE" | "GETEX" | "GETDEL" | "SETEX" | "SETNX" |
            "INCRBY" | "DECRBY" | "INCRBYFLOAT" | "PSETEX" => 0,

            // Multi-key commands (we'll check if they're in same slot)
            "MGET" | "MSET" | "MSETNX" => {
                // For multi-key commands, use check_multi_key_slot
                if cmd_args.len() > 1 {
                    use crate::cluster::check_multi_key_slot;
                    let keys: Vec<&[u8]> = cmd_args.iter().skip(1).step_by(if cmd_name == "MGET" { 1 } else { 2 }).map(|k| k.as_slice()).collect();

                    match check_multi_key_slot(&keys) {
                        Ok(slot) => {
                            // All keys in same slot, check if we own it
                            return check_slot_ownership(&self.cluster, &cmd_args[1], self.asking);
                        }
                        Err(msg) => {
                            // Keys in different slots - CROSSSLOT error
                            return Some(RespValue::Error(msg));
                        }
                    }
                }
                return None;
            }

            // Commands we don't handle yet
            _ => return None,
        };

        // Get the key
        if cmd_args.len() <= key_index + 1 {
            return None; // Not enough arguments
        }

        let key = &cmd_args[key_index + 1];

        // Check slot ownership and return redirection if needed
        check_slot_ownership(&self.cluster, key, self.asking)
    }

    /// Handle CLUSTER commands with access to cluster state
    fn handle_cluster_command(&self, args: &[Vec<u8>]) -> RespValue {
        use crate::commands::cluster::*;
        use crate::cluster::migration::*;

        if args.is_empty() {
            return RespValue::Error("ERR wrong number of arguments for 'cluster' command".to_string());
        }

        let subcommand = match std::str::from_utf8(&args[0]) {
            Ok(s) => s.to_uppercase(),
            Err(_) => return RespValue::Error("ERR invalid subcommand".to_string()),
        };

        match subcommand.as_str() {
            "KEYSLOT" => {
                if args.len() != 2 {
                    return RespValue::Error("ERR wrong number of arguments for 'cluster keyslot'".to_string());
                }
                cluster_keyslot(&args[1])
            }
            "INFO" => cluster_info(&self.cluster),
            "MYID" => cluster_myid(&self.cluster),
            "NODES" => cluster_nodes(&self.cluster),
            "SLOTS" => cluster_slots(&self.cluster),
            "ADDSLOTS" => {
                // Parse slot numbers
                let mut slots = Vec::new();
                for arg in &args[1..] {
                    match std::str::from_utf8(arg).ok().and_then(|s| s.parse::<u16>().ok()) {
                        Some(slot) if slot < 16384 => slots.push(slot),
                        _ => return RespValue::Error("ERR Invalid slot number".to_string()),
                    }
                }
                cluster_addslots(&self.cluster, slots)
            }
            "DELSLOTS" => {
                // Parse slot numbers
                let mut slots = Vec::new();
                for arg in &args[1..] {
                    match std::str::from_utf8(arg).ok().and_then(|s| s.parse::<u16>().ok()) {
                        Some(slot) if slot < 16384 => slots.push(slot),
                        _ => return RespValue::Error("ERR Invalid slot number".to_string()),
                    }
                }
                cluster_delslots(&self.cluster, slots)
            }
            "SETSLOT" => {
                if args.len() < 3 {
                    return RespValue::Error("ERR wrong number of arguments for 'cluster setslot'".to_string());
                }

                let slot = match std::str::from_utf8(&args[1]).ok().and_then(|s| s.parse::<u16>().ok()) {
                    Some(s) if s < 16384 => s,
                    _ => return RespValue::Error("ERR Invalid slot number".to_string()),
                };

                let action = match std::str::from_utf8(&args[2]) {
                    Ok(s) => s.to_uppercase(),
                    Err(_) => return RespValue::Error("ERR invalid action".to_string()),
                };

                match action.as_str() {
                    "IMPORTING" => {
                        if args.len() != 4 {
                            return RespValue::Error("ERR wrong number of arguments".to_string());
                        }
                        let node_id = String::from_utf8_lossy(&args[3]).to_string();
                        cluster_setslot_importing(&self.cluster, &self.migration, slot, node_id)
                    }
                    "MIGRATING" => {
                        if args.len() != 4 {
                            return RespValue::Error("ERR wrong number of arguments".to_string());
                        }
                        let node_id = String::from_utf8_lossy(&args[3]).to_string();
                        cluster_setslot_migrating(&self.cluster, &self.migration, slot, node_id)
                    }
                    "STABLE" => cluster_setslot_stable(&self.cluster, &self.migration, slot),
                    "NODE" => {
                        if args.len() != 4 {
                            return RespValue::Error("ERR wrong number of arguments".to_string());
                        }
                        let node_id = String::from_utf8_lossy(&args[3]).to_string();
                        cluster_setslot_node(&self.cluster, &self.migration, slot, node_id)
                    }
                    _ => RespValue::Error(format!("ERR Unknown SETSLOT action '{}'", action)),
                }
            }
            "GETKEYSINSLOT" => {
                if args.len() != 3 {
                    return RespValue::Error("ERR wrong number of arguments".to_string());
                }
                let slot = match std::str::from_utf8(&args[1]).ok().and_then(|s| s.parse::<u16>().ok()) {
                    Some(s) if s < 16384 => s,
                    _ => return RespValue::Error("ERR Invalid slot number".to_string()),
                };
                let count = match std::str::from_utf8(&args[2]).ok().and_then(|s| s.parse::<i64>().ok()) {
                    Some(c) => c,
                    _ => return RespValue::Error("ERR Invalid count".to_string()),
                };
                crate::commands::cluster::cluster_getkeysinslot(&self.cluster, slot, count)
            }
            "COUNTKEYSINSLOT" => {
                if args.len() != 2 {
                    return RespValue::Error("ERR wrong number of arguments".to_string());
                }
                let slot = match std::str::from_utf8(&args[1]).ok().and_then(|s| s.parse::<u16>().ok()) {
                    Some(s) if s < 16384 => s,
                    _ => return RespValue::Error("ERR Invalid slot number".to_string()),
                };
                crate::commands::cluster::cluster_countkeysinslot(&self.cluster, slot)
            }
            _ => RespValue::Error(format!("ERR Unknown CLUSTER subcommand '{}'", subcommand)),
        }
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_db_selection() {
        let config = ServerConfig::default();
        let db = Database::new(16);
        let socket = std::net::TcpStream::connect("127.0.0.1:1").unwrap_or_else(|_| {
            panic!("This test doesn't actually need a connection")
        });

        // This test is just for API demonstration
        // Real tests would use a mock TcpStream
    }
}
