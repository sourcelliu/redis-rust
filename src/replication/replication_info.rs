// Replication information and state management

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, RwLock};


/// Server role in replication
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReplicationRole {
    /// This server is a master
    Master,
    /// This server is a replica of another server
    Replica {
        /// Master host
        master_host: String,
        /// Master port
        master_port: u16,
        /// Connection state
        state: ReplicaState,
    },
}

/// Replica connection state
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReplicaState {
    /// Not connected
    Disconnected,
    /// Connecting to master
    Connecting,
    /// Sending PING to master
    SendingPing,
    /// Waiting for PONG
    WaitingPong,
    /// Sending REPLCONF
    SendingReplconf,
    /// Waiting for full sync
    WaitingFullSync,
    /// Receiving RDB from master
    ReceivingRdb,
    /// Connected and syncing
    Connected,
}

/// Replica information
#[derive(Debug, Clone)]
pub struct ReplicaInfo {
    /// Replica ID (client ID)
    pub id: String,
    /// IP address
    pub ip: String,
    /// Port
    pub port: u16,
    /// Replication offset
    pub offset: u64,
    /// Last interaction time
    pub last_interaction: std::time::Instant,
}

/// Replication information manager
pub struct ReplicationInfo {
    /// Server role
    role: Arc<RwLock<ReplicationRole>>,
    /// Replication ID (changes when becoming master)
    replication_id: Arc<RwLock<String>>,
    /// Master replication offset
    master_offset: Arc<AtomicU64>,
    /// Connected replicas (only for master)
    replicas: Arc<RwLock<Vec<ReplicaInfo>>>,
}

impl ReplicationInfo {
    /// Create new replication info (default: master)
    pub fn new() -> Self {
        Self {
            role: Arc::new(RwLock::new(ReplicationRole::Master)),
            replication_id: Arc::new(RwLock::new(Self::generate_replication_id())),
            master_offset: Arc::new(AtomicU64::new(0)),
            replicas: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// Generate a new replication ID (40 character random hex string)
    fn generate_replication_id() -> String {
        use rand::Rng;
        let mut rng = rand::thread_rng();
        (0..40)
            .map(|_| format!("{:x}", rng.gen::<u8>() % 16))
            .collect()
    }

    /// Get current role
    pub fn role(&self) -> ReplicationRole {
        self.role.read().unwrap().clone()
    }

    /// Check if this server is a master
    pub fn is_master(&self) -> bool {
        matches!(*self.role.read().unwrap(), ReplicationRole::Master)
    }

    /// Check if this server is a replica
    pub fn is_replica(&self) -> bool {
        matches!(*self.role.read().unwrap(), ReplicationRole::Replica { .. })
    }

    /// Set as master
    pub fn set_master(&self) {
        let mut role = self.role.write().unwrap();
        *role = ReplicationRole::Master;

        // Generate new replication ID when becoming master
        let mut repl_id = self.replication_id.write().unwrap();
        *repl_id = Self::generate_replication_id();
    }

    /// Set as replica
    pub fn set_replica(&self, master_host: String, master_port: u16) {
        let mut role = self.role.write().unwrap();
        *role = ReplicationRole::Replica {
            master_host,
            master_port,
            state: ReplicaState::Disconnected,
        };
    }

    /// Update replica state
    pub fn update_replica_state(&self, new_state: ReplicaState) {
        let mut role = self.role.write().unwrap();
        if let ReplicationRole::Replica { master_host, master_port, .. } = &*role {
            *role = ReplicationRole::Replica {
                master_host: master_host.clone(),
                master_port: *master_port,
                state: new_state,
            };
        }
    }

    /// Get replication ID
    pub fn replication_id(&self) -> String {
        self.replication_id.read().unwrap().clone()
    }

    /// Get master replication offset
    pub fn master_offset(&self) -> u64 {
        self.master_offset.load(Ordering::SeqCst)
    }

    /// Increment master offset
    pub fn increment_offset(&self, bytes: u64) {
        self.master_offset.fetch_add(bytes, Ordering::SeqCst);
    }

    /// Set master offset
    pub fn set_offset(&self, offset: u64) {
        self.master_offset.store(offset, Ordering::SeqCst);
    }

    /// Add a replica
    pub fn add_replica(&self, replica: ReplicaInfo) {
        let mut replicas = self.replicas.write().unwrap();
        replicas.push(replica);
    }

    /// Remove a replica
    pub fn remove_replica(&self, replica_id: &str) {
        let mut replicas = self.replicas.write().unwrap();
        replicas.retain(|r| r.id != replica_id);
    }

    /// Get all replicas
    pub fn replicas(&self) -> Vec<ReplicaInfo> {
        self.replicas.read().unwrap().clone()
    }

    /// Get replica count
    pub fn replica_count(&self) -> usize {
        self.replicas.read().unwrap().len()
    }

    /// Update replica offset
    pub fn update_replica_offset(&self, replica_id: &str, offset: u64) {
        let mut replicas = self.replicas.write().unwrap();
        if let Some(replica) = replicas.iter_mut().find(|r| r.id == replica_id) {
            replica.offset = offset;
            replica.last_interaction = std::time::Instant::now();
        }
    }
}

impl Default for ReplicationInfo {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_replication_info_master() {
        let info = ReplicationInfo::new();
        assert!(info.is_master());
        assert!(!info.is_replica());
        assert_eq!(info.replica_count(), 0);
    }

    #[test]
    fn test_set_replica() {
        let info = ReplicationInfo::new();
        info.set_replica("127.0.0.1".to_string(), 6379);

        assert!(!info.is_master());
        assert!(info.is_replica());

        match info.role() {
            ReplicationRole::Replica { master_host, master_port, state } => {
                assert_eq!(master_host, "127.0.0.1");
                assert_eq!(master_port, 6379);
                assert_eq!(state, ReplicaState::Disconnected);
            }
            _ => panic!("Expected Replica role"),
        }
    }

    #[test]
    fn test_offset_management() {
        let info = ReplicationInfo::new();

        assert_eq!(info.master_offset(), 0);

        info.increment_offset(100);
        assert_eq!(info.master_offset(), 100);

        info.increment_offset(50);
        assert_eq!(info.master_offset(), 150);

        info.set_offset(200);
        assert_eq!(info.master_offset(), 200);
    }

    #[test]
    fn test_replica_management() {
        let info = ReplicationInfo::new();

        let replica = ReplicaInfo {
            id: "replica1".to_string(),
            ip: "127.0.0.1".to_string(),
            port: 6380,
            offset: 0,
            last_interaction: std::time::Instant::now(),
        };

        info.add_replica(replica);
        assert_eq!(info.replica_count(), 1);

        info.update_replica_offset("replica1", 100);
        let replicas = info.replicas();
        assert_eq!(replicas[0].offset, 100);

        info.remove_replica("replica1");
        assert_eq!(info.replica_count(), 0);
    }

    #[test]
    fn test_replication_id_generation() {
        let id = ReplicationInfo::generate_replication_id();
        assert_eq!(id.len(), 40);

        // IDs should be different
        let id2 = ReplicationInfo::generate_replication_id();
        assert_ne!(id, id2);
    }
}
