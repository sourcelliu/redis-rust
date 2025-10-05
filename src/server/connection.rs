// Connection handler

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
    /// Current selected database (0-15)
    db_index: usize,
    /// Transaction state
    transaction: Transaction,
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
            db_index: 0,
            transaction: Transaction::new(),
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
