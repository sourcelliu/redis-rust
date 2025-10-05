// Server configuration

use crate::persistence::aof::AofSyncPolicy;
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct ServerConfig {
    /// Address to bind to
    pub bind: String,
    /// Port to listen on
    pub port: u16,
    /// TCP backlog
    pub tcp_backlog: u32,
    /// Connection timeout
    pub timeout: Duration,
    /// Maximum number of concurrent clients
    pub max_clients: usize,
    /// Number of databases (default 16)
    pub databases: usize,
    /// Enable AOF persistence
    pub aof_enabled: bool,
    /// AOF file path
    pub aof_filename: String,
    /// AOF sync policy
    pub aof_sync_policy: AofSyncPolicy,
    /// Enable RDB persistence
    pub rdb_enabled: bool,
    /// RDB file path
    pub rdb_filename: String,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            bind: "127.0.0.1".to_string(),
            port: 6379,
            tcp_backlog: 511,
            timeout: Duration::from_secs(0), // 0 means no timeout
            max_clients: 10000,
            databases: 16,
            aof_enabled: true,
            aof_filename: "appendonly.aof".to_string(),
            aof_sync_policy: AofSyncPolicy::EverySecond,
            rdb_enabled: true,
            rdb_filename: "dump.rdb".to_string(),
        }
    }
}

impl ServerConfig {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_port(mut self, port: u16) -> Self {
        self.port = port;
        self
    }

    pub fn with_bind(mut self, bind: String) -> Self {
        self.bind = bind;
        self
    }

    pub fn with_max_clients(mut self, max_clients: usize) -> Self {
        self.max_clients = max_clients;
        self
    }

    pub fn addr(&self) -> String {
        format!("{}:{}", self.bind, self.port)
    }
}
