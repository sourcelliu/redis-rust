// AOF (Append-Only File) persistence

use crate::protocol::{RespSerializer, RespValue};
use crate::storage::db::Database;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::fs::{File, OpenOptions};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, BufWriter};
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

/// AOF sync policies
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AofSyncPolicy {
    /// Sync every write (safest, slowest)
    Always,
    /// Sync every second (balanced)
    EverySecond,
    /// Let OS decide when to sync (fastest, least safe)
    No,
}

/// AOF writer - handles appending commands to the AOF file
pub struct AofWriter {
    file: Arc<RwLock<BufWriter<File>>>,
    path: PathBuf,
    sync_policy: AofSyncPolicy,
}

impl AofWriter {
    /// Create a new AOF writer
    pub async fn new(path: impl AsRef<Path>, sync_policy: AofSyncPolicy) -> anyhow::Result<Self> {
        let path = path.as_ref().to_path_buf();

        // Create parent directory if it doesn't exist
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        // Open file in append mode, create if it doesn't exist
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .await?;

        let writer = BufWriter::new(file);

        info!("AOF writer created at {:?} with policy {:?}", path, sync_policy);

        Ok(Self {
            file: Arc::new(RwLock::new(writer)),
            path,
            sync_policy,
        })
    }

    /// Append a command to the AOF file
    /// Commands are stored in RESP format for easy replay
    pub async fn append_command(&self, db_index: usize, args: &[Vec<u8>]) -> anyhow::Result<()> {
        let mut writer = self.file.write().await;

        // If not default database, prepend SELECT command
        if db_index != 0 {
            let select_cmd = vec![
                RespValue::BulkString(Some(b"SELECT".to_vec())),
                RespValue::BulkString(Some(db_index.to_string().into_bytes())),
            ];
            let select_data = RespSerializer::serialize(&RespValue::Array(Some(select_cmd)));
            writer.write_all(&select_data).await?;
        }

        // Convert args to RESP array
        let resp_args: Vec<RespValue> = args
            .iter()
            .map(|arg| RespValue::BulkString(Some(arg.clone())))
            .collect();

        let command = RespValue::Array(Some(resp_args));
        let data = RespSerializer::serialize(&command);

        writer.write_all(&data).await?;

        // Apply sync policy
        match self.sync_policy {
            AofSyncPolicy::Always => {
                writer.flush().await?;
            }
            AofSyncPolicy::EverySecond => {
                // Flush is handled by background task
            }
            AofSyncPolicy::No => {
                // Let OS handle flushing
            }
        }

        Ok(())
    }

    /// Flush the buffer to disk
    pub async fn flush(&self) -> anyhow::Result<()> {
        let mut writer = self.file.write().await;
        writer.flush().await?;
        Ok(())
    }

    /// Get the path of the AOF file
    pub fn path(&self) -> &Path {
        &self.path
    }
}

/// AOF reader - handles loading commands from AOF file
pub struct AofReader {
    path: PathBuf,
}

impl AofReader {
    /// Create a new AOF reader
    pub fn new(path: impl AsRef<Path>) -> Self {
        Self {
            path: path.as_ref().to_path_buf(),
        }
    }

    /// Load AOF file and replay commands into the database
    pub async fn load(&self, db: &Arc<Database>) -> anyhow::Result<usize> {
        if !self.path.exists() {
            info!("AOF file does not exist, skipping load");
            return Ok(0);
        }

        info!("Loading AOF from {:?}", self.path);

        let file = File::open(&self.path).await?;
        let reader = BufReader::new(file);
        let mut lines = reader.lines();

        let mut current_db = 0;
        let mut commands_loaded = 0;
        let mut buffer = String::new();

        while let Some(line) = lines.next_line().await? {
            buffer.push_str(&line);
            buffer.push('\n');

            // Try to parse complete RESP value
            if let Ok(Some(value)) = self.try_parse_resp(&buffer) {
                // Process the command
                if let Err(e) = self.replay_command(db, &mut current_db, &value).await {
                    error!("Error replaying command: {}", e);
                }
                commands_loaded += 1;
                buffer.clear();
            }
        }

        info!("AOF loaded {} commands", commands_loaded);
        Ok(commands_loaded)
    }

    /// Try to parse a complete RESP value from buffer
    fn try_parse_resp(&self, buffer: &str) -> anyhow::Result<Option<RespValue>> {
        use crate::protocol::RespParser;
        use bytes::BytesMut;

        let bytes = BytesMut::from(buffer.as_bytes());
        match RespParser::check_complete(&bytes) {
            Ok(Some(_)) => {
                let value = RespParser::parse(&bytes)?;
                Ok(Some(value))
            }
            Ok(None) => Ok(None),
            Err(_) => Ok(None),
        }
    }

    /// Replay a single command into the database
    async fn replay_command(
        &self,
        db: &Arc<Database>,
        current_db: &mut usize,
        value: &RespValue,
    ) -> anyhow::Result<()> {
        // Extract command array
        let args = match value {
            RespValue::Array(Some(arr)) => arr,
            _ => {
                warn!("Invalid AOF entry: not an array");
                return Ok(());
            }
        };

        if args.is_empty() {
            return Ok(());
        }

        // Extract command name
        let cmd_bytes = match &args[0] {
            RespValue::BulkString(Some(data)) => data,
            _ => return Ok(()),
        };

        let cmd = String::from_utf8_lossy(cmd_bytes).to_uppercase();

        // Handle SELECT command specially
        if cmd == "SELECT" {
            if args.len() >= 2 {
                if let RespValue::BulkString(Some(db_bytes)) = &args[1] {
                    if let Ok(db_str) = std::str::from_utf8(db_bytes) {
                        if let Ok(db_index) = db_str.parse::<usize>() {
                            *current_db = db_index;
                        }
                    }
                }
            }
            return Ok(());
        }

        // Replay the command using the command dispatcher
        let mut cmd_args: Vec<Vec<u8>> = Vec::new();
        for arg in args {
            match arg {
                RespValue::BulkString(Some(data)) => cmd_args.push(data.clone()),
                _ => {}
            }
        }

        // Use a simple command executor for replay (avoid circular dependencies)
        // In production, you'd use the actual CommandDispatcher
        self.execute_command_for_replay(db, *current_db, &cmd_args).await?;

        Ok(())
    }

    /// Execute command during AOF replay
    /// This is a simplified executor that doesn't use the full dispatcher
    async fn execute_command_for_replay(
        &self,
        db: &Arc<Database>,
        db_index: usize,
        args: &[Vec<u8>],
    ) -> anyhow::Result<()> {
        if args.is_empty() {
            return Ok(());
        }

        let cmd = String::from_utf8_lossy(&args[0]).to_uppercase();

        // Import command handlers
        use crate::commands::{expiration, hash, list, set, string, zset};

        // Execute the command (simplified - just call handlers directly)
        let _ = match cmd.as_str() {
            // String commands
            "SET" => string::set(db, db_index, args[1..].to_vec()).await,
            "DEL" => string::del(db, db_index, args[1..].to_vec()).await,
            "APPEND" => string::append(db, db_index, args[1..].to_vec()).await,
            "INCR" => string::incr(db, db_index, args[1..].to_vec()).await,
            "DECR" => string::decr(db, db_index, args[1..].to_vec()).await,
            "INCRBY" => string::incrby(db, db_index, args[1..].to_vec()).await,
            "DECRBY" => string::decrby(db, db_index, args[1..].to_vec()).await,
            "SETRANGE" => string::setrange(db, db_index, args[1..].to_vec()).await,
            "MSET" => string::mset(db, db_index, args[1..].to_vec()).await,

            // List commands
            "LPUSH" => list::lpush(db, db_index, args[1..].to_vec()).await,
            "RPUSH" => list::rpush(db, db_index, args[1..].to_vec()).await,
            "LPOP" => list::lpop(db, db_index, args[1..].to_vec()).await,
            "RPOP" => list::rpop(db, db_index, args[1..].to_vec()).await,
            "LSET" => list::lset(db, db_index, args[1..].to_vec()).await,
            "LTRIM" => list::ltrim(db, db_index, args[1..].to_vec()).await,

            // Hash commands
            "HSET" => hash::hset(db, db_index, args[1..].to_vec()).await,
            "HDEL" => hash::hdel(db, db_index, args[1..].to_vec()).await,

            // Set commands
            "SADD" => set::sadd(db, db_index, args[1..].to_vec()).await,
            "SREM" => set::srem(db, db_index, args[1..].to_vec()).await,
            "SPOP" => set::spop(db, db_index, args[1..].to_vec()).await,

            // ZSet commands
            "ZADD" => zset::zadd(db, db_index, args[1..].to_vec()).await,
            "ZREM" => zset::zrem(db, db_index, args[1..].to_vec()).await,

            // Expiration commands
            "EXPIRE" => expiration::expire(db, db_index, args[1..].to_vec()).await,
            "EXPIREAT" => expiration::expireat(db, db_index, args[1..].to_vec()).await,
            "PEXPIRE" => expiration::pexpire(db, db_index, args[1..].to_vec()).await,
            "PEXPIREAT" => expiration::pexpireat(db, db_index, args[1..].to_vec()).await,
            "PERSIST" => expiration::persist(db, db_index, args[1..].to_vec()).await,

            _ => {
                debug!("Skipping unknown command during AOF replay: {}", cmd);
                RespValue::SimpleString("OK".to_string())
            }
        };

        Ok(())
    }
}

/// AOF manager - coordinates writing and rewriting
pub struct AofManager {
    writer: Option<Arc<AofWriter>>,
    enabled: bool,
}

impl AofManager {
    /// Create a new AOF manager
    pub async fn new(
        enabled: bool,
        path: Option<impl AsRef<Path>>,
        sync_policy: AofSyncPolicy,
    ) -> anyhow::Result<Self> {
        let writer = if enabled && path.is_some() {
            let writer = AofWriter::new(path.unwrap(), sync_policy).await?;
            Some(Arc::new(writer))
        } else {
            None
        };

        Ok(Self { writer, enabled })
    }

    /// Append a command to the AOF
    pub async fn append(&self, db_index: usize, args: &[Vec<u8>]) -> anyhow::Result<()> {
        if !self.enabled || self.writer.is_none() {
            return Ok(());
        }

        if let Some(writer) = &self.writer {
            writer.append_command(db_index, args).await?;
        }

        Ok(())
    }

    /// Flush the AOF to disk
    pub async fn flush(&self) -> anyhow::Result<()> {
        if let Some(writer) = &self.writer {
            writer.flush().await?;
        }
        Ok(())
    }

    /// Check if AOF is enabled
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Rewrite the AOF file by taking a snapshot of current database state
    pub async fn rewrite(&self, db: &Arc<Database>, new_path: impl AsRef<Path>) -> anyhow::Result<()> {
        info!("Starting AOF rewrite");

        let new_path = new_path.as_ref();

        // Create a new temporary AOF file
        let temp_writer = AofWriter::new(new_path, AofSyncPolicy::Always).await?;

        // Iterate through all databases and write current state
        for db_index in 0..16 {  // Assume 16 databases
            let keys = db.keys(db_index, "*").await;

            for key in keys {
                // Get the value and write appropriate command
                if let Some(db_instance) = db.get_db(db_index) {
                    if let Some(value) = db_instance.get(&key) {
                        // Generate commands to recreate this key
                        self.write_value_to_aof(&temp_writer, db_index, &key, &value).await?;

                        // If key has expiration, write PEXPIREAT command
                        let ttl_ms = db_instance.get_ttl_ms(&key);
                        if ttl_ms > 0 {
                            let expire_at = crate::storage::db::current_timestamp_ms() + ttl_ms as u64;
                            let args = vec![
                                b"PEXPIREAT".to_vec(),
                                key.as_bytes().to_vec(),
                                expire_at.to_string().into_bytes(),
                            ];
                            temp_writer.append_command(db_index, &args).await?;
                        }
                    }
                }
            }
        }

        temp_writer.flush().await?;
        info!("AOF rewrite completed");

        Ok(())
    }

    /// Write a single key-value pair to AOF during rewrite
    async fn write_value_to_aof(
        &self,
        writer: &AofWriter,
        db_index: usize,
        key: &str,
        value: &crate::storage::types::RedisValue,
    ) -> anyhow::Result<()> {
        use crate::storage::types::RedisValue;

        match value {
            RedisValue::String(data) => {
                let args = vec![
                    b"SET".to_vec(),
                    key.as_bytes().to_vec(),
                    data.to_vec(),
                ];
                writer.append_command(db_index, &args).await?;
            }
            RedisValue::List(list) => {
                if !list.is_empty() {
                    let mut args = vec![b"RPUSH".to_vec(), key.as_bytes().to_vec()];
                    for item in list.iter() {
                        args.push(item.to_vec());
                    }
                    writer.append_command(db_index, &args).await?;
                }
            }
            RedisValue::Set(set) => {
                if !set.is_empty() {
                    let mut args = vec![b"SADD".to_vec(), key.as_bytes().to_vec()];
                    for member in set.iter() {
                        args.push(member.to_vec());
                    }
                    writer.append_command(db_index, &args).await?;
                }
            }
            RedisValue::Hash(hash) => {
                if !hash.is_empty() {
                    for (field, val) in hash.iter() {
                        let args = vec![
                            b"HSET".to_vec(),
                            key.as_bytes().to_vec(),
                            field.to_vec(),
                            val.to_vec(),
                        ];
                        writer.append_command(db_index, &args).await?;
                    }
                }
            }
            RedisValue::ZSet(zset) => {
                if !zset.members.is_empty() {
                    for (member, score) in zset.members.iter() {
                        let args = vec![
                            b"ZADD".to_vec(),
                            key.as_bytes().to_vec(),
                            score.to_string().into_bytes(),
                            member.to_vec(),
                        ];
                        writer.append_command(db_index, &args).await?;
                    }
                }
            }
            RedisValue::Stream(stream) => {
                // Write stream entries using XADD
                for (id, entry) in &stream.entries {
                    let mut args = vec![
                        b"XADD".to_vec(),
                        key.as_bytes().to_vec(),
                        id.to_string().into_bytes(),
                    ];
                    for (field, value) in &entry.fields {
                        args.push(field.to_vec());
                        args.push(value.to_vec());
                    }
                    writer.append_command(db_index, &args).await?;
                }
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_aof_write_and_read() {
        let temp_dir = TempDir::new().unwrap();
        let aof_path = temp_dir.path().join("test.aof");

        // Write some commands
        let writer = AofWriter::new(&aof_path, AofSyncPolicy::Always).await.unwrap();

        let cmd1 = vec![b"SET".to_vec(), b"key1".to_vec(), b"value1".to_vec()];
        writer.append_command(0, &cmd1).await.unwrap();

        let cmd2 = vec![b"SET".to_vec(), b"key2".to_vec(), b"value2".to_vec()];
        writer.append_command(0, &cmd2).await.unwrap();

        writer.flush().await.unwrap();

        // Verify file exists and has content
        assert!(aof_path.exists());
        let metadata = tokio::fs::metadata(&aof_path).await.unwrap();
        assert!(metadata.len() > 0);
    }

    #[tokio::test]
    async fn test_aof_load() {
        let temp_dir = TempDir::new().unwrap();
        let aof_path = temp_dir.path().join("load_test.aof");

        // Create a database and write some data via AOF
        let db = Arc::new(Database::new(16));
        let writer = AofWriter::new(&aof_path, AofSyncPolicy::Always).await.unwrap();

        let cmd1 = vec![b"SET".to_vec(), b"key1".to_vec(), b"value1".to_vec()];
        writer.append_command(0, &cmd1).await.unwrap();
        writer.flush().await.unwrap();

        // Load the AOF into a fresh database
        let db2 = Arc::new(Database::new(16));
        let reader = AofReader::new(&aof_path);
        let count = reader.load(&db2).await.unwrap();
        assert_eq!(count, 1);
    }
}