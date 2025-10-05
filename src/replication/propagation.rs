// Command propagation - Master propagates write commands to replicas

use crate::protocol::{RespSerializer, RespValue};
use crate::replication::ReplicationBacklog;
use std::sync::Arc;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpStream;
use tokio::sync::RwLock;
use tracing::{debug, error, warn};

/// Manages command propagation from master to replicas
pub struct CommandPropagator {
    /// Active replica connections
    replicas: Arc<RwLock<Vec<ReplicaConnection>>>,
    /// Replication backlog for partial resync
    backlog: Arc<ReplicationBacklog>,
}

/// Represents an active replica connection
pub struct ReplicaConnection {
    pub stream: Arc<RwLock<TcpStream>>,
    pub ip: String,
    pub port: u16,
    pub offset: u64,
}

impl CommandPropagator {
    pub fn new(backlog: Arc<ReplicationBacklog>) -> Self {
        Self {
            replicas: Arc::new(RwLock::new(Vec::new())),
            backlog,
        }
    }

    /// Add a replica connection for command propagation
    pub async fn add_replica(&self, stream: TcpStream, ip: String, port: u16, offset: u64) {
        let replica = ReplicaConnection {
            stream: Arc::new(RwLock::new(stream)),
            ip: ip.clone(),
            port,
            offset,
        };

        let mut replicas = self.replicas.write().await;
        replicas.push(replica);
        debug!("Added replica {}:{} for command propagation", ip, port);
    }

    /// Remove a replica connection (on disconnect)
    pub async fn remove_replica(&self, ip: &str, port: u16) {
        let mut replicas = self.replicas.write().await;
        replicas.retain(|r| !(r.ip == ip && r.port == port));
        debug!("Removed replica {}:{} from propagation", ip, port);
    }

    /// Propagate a write command to all replicas
    pub async fn propagate(&self, db_index: usize, cmd_args: &[Vec<u8>], offset: u64) {
        // Add to backlog for partial resync
        let cmd_resp = Self::encode_command(db_index, cmd_args);
        self.backlog.add(offset, cmd_resp.clone());

        // Propagate to all connected replicas
        let replicas = self.replicas.read().await;

        for replica in replicas.iter() {
            let stream = replica.stream.clone();
            let cmd_data = cmd_resp.clone();
            let ip = replica.ip.clone();
            let port = replica.port;

            // Spawn task to send to replica (non-blocking)
            tokio::spawn(async move {
                if let Err(e) = Self::send_to_replica(stream, &cmd_data).await {
                    error!("Failed to propagate to replica {}:{}: {}", ip, port, e);
                }
            });
        }
    }

    /// Send command data to a specific replica
    async fn send_to_replica(
        stream: Arc<RwLock<TcpStream>>,
        cmd_data: &[u8],
    ) -> anyhow::Result<()> {
        let mut stream = stream.write().await;
        stream.write_all(cmd_data).await?;
        stream.flush().await?;
        Ok(())
    }

    /// Encode command as RESP array for transmission
    fn encode_command(db_index: usize, cmd_args: &[Vec<u8>]) -> Vec<u8> {
        // If db_index != 0, prepend SELECT command
        let mut commands = Vec::new();

        if db_index != 0 {
            // SELECT db_index
            let select_cmd = RespValue::Array(Some(vec![
                RespValue::BulkString(Some(b"SELECT".to_vec())),
                RespValue::BulkString(Some(db_index.to_string().into_bytes())),
            ]));
            commands.push(RespSerializer::serialize(&select_cmd));
        }

        // Original command
        let cmd_value = RespValue::Array(Some(
            cmd_args
                .iter()
                .map(|arg| RespValue::BulkString(Some(arg.clone())))
                .collect(),
        ));
        commands.push(RespSerializer::serialize(&cmd_value));

        commands.concat()
    }

    /// Get replica count
    pub async fn replica_count(&self) -> usize {
        self.replicas.read().await.len()
    }

    /// Get replica info for ROLE command
    pub async fn get_replica_info(&self) -> Vec<(String, u16, u64)> {
        self.replicas
            .read()
            .await
            .iter()
            .map(|r| (r.ip.clone(), r.port, r.offset))
            .collect()
    }

    /// Update replica offset (from ACK)
    pub async fn update_replica_offset(&self, ip: &str, port: u16, offset: u64) {
        let mut replicas = self.replicas.write().await;
        if let Some(replica) = replicas.iter_mut().find(|r| r.ip == ip && r.port == port) {
            replica.offset = offset;
            debug!("Updated replica {}:{} offset to {}", ip, port, offset);
        } else {
            warn!("Replica {}:{} not found for offset update", ip, port);
        }
    }

    /// Wait for replicas to acknowledge offset (for WAIT command)
    pub async fn wait_for_replicas(
        &self,
        min_replicas: usize,
        timeout_ms: u64,
        target_offset: u64,
    ) -> usize {
        let start = std::time::Instant::now();
        let timeout = std::time::Duration::from_millis(timeout_ms);

        loop {
            let replicas = self.replicas.read().await;
            let acked = replicas.iter().filter(|r| r.offset >= target_offset).count();

            if acked >= min_replicas {
                return acked;
            }

            if start.elapsed() >= timeout {
                return acked;
            }

            drop(replicas);
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_encode_command() {
        let cmd_args = vec![b"SET".to_vec(), b"key".to_vec(), b"value".to_vec()];
        let encoded = CommandPropagator::encode_command(0, &cmd_args);

        // Should be a RESP array
        assert!(encoded.starts_with(b"*3\r\n"));
    }

    #[tokio::test]
    async fn test_encode_command_with_select() {
        let cmd_args = vec![b"SET".to_vec(), b"key".to_vec(), b"value".to_vec()];
        let encoded = CommandPropagator::encode_command(5, &cmd_args);

        // Should start with SELECT command
        assert!(encoded.starts_with(b"*2\r\n"));
        assert!(encoded.contains(&b"SELECT"[..]));
    }

    #[tokio::test]
    async fn test_replica_count() {
        let backlog = Arc::new(ReplicationBacklog::new());
        let propagator = CommandPropagator::new(backlog);

        assert_eq!(propagator.replica_count().await, 0);
    }
}
