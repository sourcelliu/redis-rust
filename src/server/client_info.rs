// Client connection tracking and management

use dashmap::DashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

/// Global client ID counter
static CLIENT_ID_COUNTER: AtomicU64 = AtomicU64::new(1);

/// Client connection information
#[derive(Debug, Clone)]
pub struct ClientInfo {
    /// Unique client ID
    pub id: u64,
    /// Client address (IP:port)
    pub addr: String,
    /// File descriptor (socket ID)
    pub fd: u64,
    /// Client name (set by CLIENT SETNAME)
    pub name: Option<String>,
    /// Connection age in seconds
    pub age: u64,
    /// Idle time in seconds
    pub idle: u64,
    /// Current database index
    pub db: usize,
    /// Flags (N=normal, M=master, S=slave, etc.)
    pub flags: String,
    /// Number of subscriptions
    pub sub: usize,
    /// Number of pattern subscriptions
    pub psub: usize,
    /// Last command executed
    pub cmd: String,
    /// Connection creation timestamp
    pub created_at: u64,
    /// Last activity timestamp
    pub last_activity: u64,
}

impl ClientInfo {
    /// Create a new client info with generated ID
    pub fn new(addr: String, fd: u64) -> Self {
        let id = CLIENT_ID_COUNTER.fetch_add(1, Ordering::SeqCst);
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        Self {
            id,
            addr,
            fd,
            name: None,
            age: 0,
            idle: 0,
            db: 0,
            flags: "N".to_string(), // Normal client
            sub: 0,
            psub: 0,
            cmd: "".to_string(),
            created_at: now,
            last_activity: now,
        }
    }

    /// Update age and idle time based on current time
    pub fn update_timing(&mut self) {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        self.age = now - self.created_at;
        self.idle = now - self.last_activity;
    }

    /// Mark activity (command execution)
    pub fn mark_activity(&mut self, cmd: String, db_index: usize) {
        self.last_activity = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        self.cmd = cmd;
        self.db = db_index;
    }

    /// Format as CLIENT LIST entry
    pub fn to_list_entry(&self) -> String {
        format!(
            "id={} addr={} fd={} name={} age={} idle={} flags={} db={} sub={} psub={} multi=-1 qbuf=0 qbuf-free=0 obl=0 oll=0 omem=0 events=r cmd={}",
            self.id,
            self.addr,
            self.fd,
            self.name.as_deref().unwrap_or(""),
            self.age,
            self.idle,
            self.flags,
            self.db,
            self.sub,
            self.psub,
            self.cmd
        )
    }
}

/// Client registry for managing all active connections
#[derive(Clone)]
pub struct ClientRegistry {
    clients: Arc<DashMap<u64, ClientInfo>>,
}

impl ClientRegistry {
    /// Create a new client registry
    pub fn new() -> Self {
        Self {
            clients: Arc::new(DashMap::new()),
        }
    }

    /// Register a new client connection
    pub fn register(&self, addr: String, fd: u64) -> u64 {
        let client = ClientInfo::new(addr, fd);
        let id = client.id;
        self.clients.insert(id, client);
        id
    }

    /// Unregister a client connection
    pub fn unregister(&self, id: u64) {
        self.clients.remove(&id);
    }

    /// Get client info by ID
    pub fn get(&self, id: u64) -> Option<ClientInfo> {
        self.clients.get(&id).map(|entry| entry.clone())
    }

    /// Update client name
    pub fn set_name(&self, id: u64, name: String) {
        if let Some(mut entry) = self.clients.get_mut(&id) {
            entry.name = Some(name);
        }
    }

    /// Get client name
    pub fn get_name(&self, id: u64) -> Option<String> {
        self.clients.get(&id).and_then(|entry| entry.name.clone())
    }

    /// Mark client activity
    pub fn mark_activity(&self, id: u64, cmd: String, db_index: usize) {
        if let Some(mut entry) = self.clients.get_mut(&id) {
            entry.mark_activity(cmd, db_index);
        }
    }

    /// Get all clients as formatted list
    pub fn list(&self) -> String {
        let mut result = String::new();
        for entry in self.clients.iter() {
            let mut client = entry.value().clone();
            client.update_timing();
            result.push_str(&client.to_list_entry());
            result.push('\n');
        }
        result
    }

    /// Get total number of clients
    pub fn count(&self) -> usize {
        self.clients.len()
    }

    /// Kill client by ID
    pub fn kill(&self, id: u64) -> bool {
        self.clients.remove(&id).is_some()
    }

    /// Kill client by address
    pub fn kill_by_addr(&self, addr: &str) -> usize {
        let mut killed = 0;
        let to_remove: Vec<u64> = self
            .clients
            .iter()
            .filter(|entry| entry.value().addr == addr)
            .map(|entry| *entry.key())
            .collect();

        for id in to_remove {
            self.clients.remove(&id);
            killed += 1;
        }
        killed
    }
}

impl Default for ClientRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_client_info_creation() {
        let client = ClientInfo::new("127.0.0.1:54321".to_string(), 8);
        assert_eq!(client.addr, "127.0.0.1:54321");
        assert_eq!(client.fd, 8);
        assert_eq!(client.name, None);
        assert_eq!(client.db, 0);
        assert!(client.id > 0);
    }

    #[test]
    fn test_client_registry() {
        let registry = ClientRegistry::new();
        let id1 = registry.register("127.0.0.1:1111".to_string(), 1);
        let id2 = registry.register("127.0.0.1:2222".to_string(), 2);

        assert_eq!(registry.count(), 2);

        registry.set_name(id1, "client1".to_string());
        assert_eq!(registry.get_name(id1), Some("client1".to_string()));

        registry.unregister(id2);
        assert_eq!(registry.count(), 1);
    }

    #[test]
    fn test_client_list_format() {
        let client = ClientInfo::new("127.0.0.1:6379".to_string(), 8);
        let list_entry = client.to_list_entry();
        assert!(list_entry.contains("addr=127.0.0.1:6379"));
        assert!(list_entry.contains("fd=8"));
        assert!(list_entry.contains("db=0"));
    }

    #[test]
    fn test_client_activity_tracking() {
        let registry = ClientRegistry::new();
        let id = registry.register("127.0.0.1:1111".to_string(), 1);

        registry.mark_activity(id, "GET".to_string(), 2);

        let client = registry.get(id).unwrap();
        assert_eq!(client.cmd, "GET");
        assert_eq!(client.db, 2);
    }
}
