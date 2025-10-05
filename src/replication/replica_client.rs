// Replica client - Connects to master and handles replication

use crate::persistence::rdb::RdbDeserializer;
use crate::protocol::{RespParser, RespSerializer, RespValue};
use crate::replication::{ReplicationInfo, ReplicationBacklog};
use crate::storage::db::Database;
use bytes::{Buf, BytesMut};
use std::sync::{atomic::AtomicU64, atomic::Ordering, Arc, Mutex};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tracing::{debug, error, info, warn};

/// Replica client that connects to master
pub struct ReplicaClient {
    master_host: String,
    master_port: u16,
    db: Arc<Database>,
    repl_info: Arc<ReplicationInfo>,
    backlog: Arc<ReplicationBacklog>,
    /// Current database index
    db_index: Arc<Mutex<usize>>,
    /// Replica offset
    replica_offset: Arc<AtomicU64>,
}

impl ReplicaClient {
    pub fn new(
        master_host: String,
        master_port: u16,
        db: Arc<Database>,
        repl_info: Arc<ReplicationInfo>,
        backlog: Arc<ReplicationBacklog>,
    ) -> Self {
        Self {
            master_host,
            master_port,
            db,
            repl_info,
            backlog,
            db_index: Arc::new(Mutex::new(0)),
            replica_offset: Arc::new(AtomicU64::new(0)),
        }
    }

    /// Start replication connection to master
    pub async fn start(&self) -> anyhow::Result<()> {
        info!(
            "Starting replication from {}:{}",
            self.master_host, self.master_port
        );

        // Connect to master
        let mut stream = TcpStream::connect(format!("{}:{}", self.master_host, self.master_port))
            .await?;

        info!("Connected to master");

        // Perform handshake
        self.handshake(&mut stream).await?;

        // Receive sync data (RDB or command stream)
        self.receive_sync(&mut stream).await?;

        // Process command stream
        self.process_command_stream(&mut stream).await?;

        Ok(())
    }

    /// Perform handshake with master
    async fn handshake(&self, stream: &mut TcpStream) -> anyhow::Result<()> {
        // Step 1: Send PING
        debug!("Sending PING to master");
        let ping_cmd = RespValue::Array(Some(vec![RespValue::BulkString(Some(
            b"PING".to_vec(),
        ))]));
        let ping_data = RespSerializer::serialize(&ping_cmd);
        stream.write_all(&ping_data).await?;
        stream.flush().await?;

        // Read PONG response
        let response = self.read_response(stream).await?;
        debug!("Received PING response: {:?}", response);

        // Step 2: Send REPLCONF listening-port
        debug!("Sending REPLCONF listening-port");
        let replconf_cmd = RespValue::Array(Some(vec![
            RespValue::BulkString(Some(b"REPLCONF".to_vec())),
            RespValue::BulkString(Some(b"listening-port".to_vec())),
            RespValue::BulkString(Some(self.master_port.to_string().into_bytes())),
        ]));
        let replconf_data = RespSerializer::serialize(&replconf_cmd);
        stream.write_all(&replconf_data).await?;
        stream.flush().await?;

        // Read OK response
        let response = self.read_response(stream).await?;
        debug!("Received REPLCONF response: {:?}", response);

        // Step 3: Send REPLCONF capa (capabilities)
        debug!("Sending REPLCONF capa");
        let capa_cmd = RespValue::Array(Some(vec![
            RespValue::BulkString(Some(b"REPLCONF".to_vec())),
            RespValue::BulkString(Some(b"capa".to_vec())),
            RespValue::BulkString(Some(b"psync2".to_vec())),
        ]));
        let capa_data = RespSerializer::serialize(&capa_cmd);
        stream.write_all(&capa_data).await?;
        stream.flush().await?;

        // Read OK response
        let response = self.read_response(stream).await?;
        debug!("Received CAPA response: {:?}", response);

        // Step 4: Send PSYNC
        debug!("Sending PSYNC");
        let psync_cmd = RespValue::Array(Some(vec![
            RespValue::BulkString(Some(b"PSYNC".to_vec())),
            RespValue::BulkString(Some(b"?".to_vec())), // ? means we don't have a replication ID
            RespValue::BulkString(Some(b"-1".to_vec())), // -1 means full resync
        ]));
        let psync_data = RespSerializer::serialize(&psync_cmd);
        stream.write_all(&psync_data).await?;
        stream.flush().await?;

        info!("Handshake completed, waiting for PSYNC response");

        Ok(())
    }

    /// Receive sync data from master (RDB or continuation)
    async fn receive_sync(&self, stream: &mut TcpStream) -> anyhow::Result<()> {
        // Read PSYNC response
        let response = self.read_response(stream).await?;
        debug!("Received PSYNC response: {:?}", response);

        match response {
            RespValue::SimpleString(s) if s.starts_with("FULLRESYNC") => {
                info!("Full resync required");
                // Parse replication ID and offset
                let parts: Vec<&str> = s.split_whitespace().collect();
                if parts.len() >= 3 {
                    let repl_id = parts[1].to_string();
                    info!("Master replication ID: {}", repl_id);
                }

                // Receive RDB data
                self.receive_rdb(stream).await?;
            }
            RespValue::SimpleString(s) if s.starts_with("CONTINUE") => {
                info!("Partial resync possible");
                // Continue with existing data, just process command stream
            }
            _ => {
                return Err(anyhow::anyhow!("Unexpected PSYNC response: {:?}", response));
            }
        }

        Ok(())
    }

    /// Receive RDB data from master
    async fn receive_rdb(&self, stream: &mut TcpStream) -> anyhow::Result<()> {
        info!("Receiving RDB data from master");

        // Read RDB as bulk string
        // Format: $<length>\r\n<data>
        let mut buffer = BytesMut::with_capacity(4096);

        // Read until we have the length prefix
        loop {
            let n = stream.read_buf(&mut buffer).await?;
            if n == 0 {
                return Err(anyhow::anyhow!("Connection closed while reading RDB"));
            }

            // Try to parse the bulk string length
            if buffer.starts_with(b"$") {
                if let Some(pos) = buffer.windows(2).position(|w| w == b"\r\n") {
                    let len_str = std::str::from_utf8(&buffer[1..pos])?;
                    let rdb_len: usize = len_str.parse()?;

                    info!("RDB size: {} bytes", rdb_len);

                    // Skip the length prefix
                    buffer.advance(pos + 2);

                    // Read the RDB data
                    let mut rdb_data = vec![0u8; rdb_len];
                    let mut read_total = 0;

                    // Copy what we already have
                    let available = buffer.len().min(rdb_len);
                    rdb_data[..available].copy_from_slice(&buffer[..available]);
                    read_total += available;
                    buffer.advance(available);

                    // Read the rest
                    while read_total < rdb_len {
                        let n = stream.read(&mut rdb_data[read_total..]).await?;
                        if n == 0 {
                            return Err(anyhow::anyhow!("Connection closed while reading RDB data"));
                        }
                        read_total += n;
                    }

                    info!("Received complete RDB data: {} bytes", read_total);

                    // Save RDB to temporary file and load it
                    let temp_path = "/tmp/replica_sync.rdb";
                    tokio::fs::write(temp_path, &rdb_data).await?;

                    info!("Loading RDB into database");
                    RdbDeserializer::load(&self.db, temp_path).await?;

                    info!("RDB loaded successfully");

                    // Clean up temp file
                    let _ = tokio::fs::remove_file(temp_path).await;

                    break;
                }
            }
        }

        Ok(())
    }

    /// Process command stream from master
    async fn process_command_stream(&self, stream: &mut TcpStream) -> anyhow::Result<()> {
        info!("Processing command stream from master");

        let mut buffer = BytesMut::with_capacity(4096);
        let mut last_ack = std::time::Instant::now();
        let ack_interval = std::time::Duration::from_secs(1); // Send ACK every second

        loop {
            // Send ACK periodically
            if last_ack.elapsed() >= ack_interval {
                self.send_ack(stream).await?;
                last_ack = std::time::Instant::now();
            }

            // Read data from master with timeout
            let read_result = tokio::time::timeout(
                std::time::Duration::from_millis(100),
                stream.read_buf(&mut buffer)
            ).await;

            match read_result {
                Ok(Ok(0)) => {
                    warn!("Connection to master closed");
                    return Ok(());
                }
                Ok(Ok(_)) => {
                    // Data received, parse commands
                }
                Ok(Err(e)) => {
                    return Err(e.into());
                }
                Err(_) => {
                    // Timeout, continue loop to check ACK
                    continue;
                }
            }

            // Try to parse commands from buffer
            while let Ok(Some(len)) = RespParser::check_complete(&buffer) {
                let frame_data = buffer.split_to(len);

                // Update replica offset by bytes consumed
                self.replica_offset.fetch_add(len as u64, Ordering::SeqCst);

                match RespParser::parse(&frame_data) {
                    Ok(frame) => {
                        debug!("Received command from master: {:?}", frame);

                        // Apply command to local database
                        if let Err(e) = self.apply_command(frame).await {
                            error!("Failed to apply command: {}", e);
                        }
                    }
                    Err(e) => {
                        error!("Failed to parse command: {}", e);
                    }
                }
            }
        }
    }

    /// Send REPLCONF ACK to master
    async fn send_ack(&self, stream: &mut TcpStream) -> anyhow::Result<()> {
        let offset = self.replica_offset.load(Ordering::SeqCst);
        debug!("Sending ACK to master with offset {}", offset);

        let ack_cmd = RespValue::Array(Some(vec![
            RespValue::BulkString(Some(b"REPLCONF".to_vec())),
            RespValue::BulkString(Some(b"ACK".to_vec())),
            RespValue::BulkString(Some(offset.to_string().into_bytes())),
        ]));

        let ack_data = RespSerializer::serialize(&ack_cmd);
        stream.write_all(&ack_data).await?;
        stream.flush().await?;

        Ok(())
    }

    /// Apply a command received from master to local database
    async fn apply_command(&self, frame: RespValue) -> anyhow::Result<()> {
        // Extract command array
        let args = match frame {
            RespValue::Array(Some(arr)) => arr,
            _ => return Ok(()), // Ignore non-array frames
        };

        if args.is_empty() {
            return Ok(());
        }

        // Convert to byte vectors
        let mut cmd_args: Vec<Vec<u8>> = Vec::new();
        for arg in args {
            match arg {
                RespValue::BulkString(Some(data)) => cmd_args.push(data),
                RespValue::SimpleString(s) => cmd_args.push(s.into_bytes()),
                _ => {}
            }
        }

        if cmd_args.is_empty() {
            return Ok(());
        }

        let cmd = String::from_utf8_lossy(&cmd_args[0]).to_uppercase();

        // Handle SELECT command specially to track database index
        if cmd == "SELECT" && cmd_args.len() >= 2 {
            if let Ok(index_str) = std::str::from_utf8(&cmd_args[1]) {
                if let Ok(index) = index_str.parse::<usize>() {
                    let mut db_index = self.db_index.lock().unwrap();
                    *db_index = index;
                    debug!("Switched to database {}", index);
                    return Ok(());
                }
            }
        }

        // Apply command directly to database
        // For replica, we apply commands simply without going through full dispatcher
        // This avoids circular dependencies and is more efficient
        debug!("Received command to apply: {}", cmd);

        // TODO: Implement direct command application
        // For now, we just log that we received it
        // Full implementation would apply each command type directly to the database

        Ok(())
    }

    /// Read a RESP response from stream
    async fn read_response(&self, stream: &mut TcpStream) -> anyhow::Result<RespValue> {
        let mut buffer = BytesMut::with_capacity(4096);

        loop {
            let n = stream.read_buf(&mut buffer).await?;
            if n == 0 {
                return Err(anyhow::anyhow!("Connection closed"));
            }

            match RespParser::check_complete(&buffer) {
                Ok(Some(len)) => {
                    let frame_data = buffer.split_to(len);
                    return Ok(RespParser::parse(&frame_data)?);
                }
                Ok(None) => continue,
                Err(_) => continue,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_replica_client_creation() {
        let db = Arc::new(Database::new(16));
        let repl_info = Arc::new(ReplicationInfo::new());
        let backlog = Arc::new(ReplicationBacklog::new());

        let client = ReplicaClient::new(
            "127.0.0.1".to_string(),
            6379,
            db,
            repl_info,
            backlog,
        );

        assert_eq!(client.master_host, "127.0.0.1");
        assert_eq!(client.master_port, 6379);
    }
}
