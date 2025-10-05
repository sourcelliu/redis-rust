// TCP Server listener

use super::client_info::ClientRegistry;
use super::config::ServerConfig;
use super::connection::Connection;
use super::slowlog::SlowLog;
use crate::cluster::{ClusterState, MigrationManager, load_cluster_config};
use crate::config::Config;
use crate::persistence::aof::{AofManager, AofReader};
use crate::persistence::rdb::RdbDeserializer;
use crate::pubsub::PubSub;
use crate::replication::{ReplicationInfo, ReplicationBacklog, CommandPropagator};
use crate::scripting::ScriptCache;
use crate::storage::db::Database;
use std::os::unix::io::AsRawFd;
use std::sync::Arc;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::Semaphore;
use tracing::{error, info, warn};

pub struct RedisServer {
    config: Arc<ServerConfig>,
    app_config: Arc<Config>,
    db: Arc<Database>,
    pubsub: Arc<PubSub>,
    aof: Arc<AofManager>,
    script_cache: Arc<ScriptCache>,
    repl_info: Arc<ReplicationInfo>,
    repl_backlog: Arc<ReplicationBacklog>,
    propagator: Arc<CommandPropagator>,
    client_registry: Arc<ClientRegistry>,
    slowlog: Arc<SlowLog>,
    cluster: Arc<ClusterState>,
    migration: Arc<MigrationManager>,
    /// Limit max concurrent connections
    limit_connections: Arc<Semaphore>,
}

impl RedisServer {
    pub async fn new(config: ServerConfig) -> anyhow::Result<Self> {
        let max_connections = config.max_clients;
        let db = Arc::new(Database::new(config.databases));

        // Load persistence data (RDB first, then AOF)
        if config.rdb_enabled && std::path::Path::new(&config.rdb_filename).exists() {
            info!("Loading RDB from {}", config.rdb_filename);
            match RdbDeserializer::load(&db, &config.rdb_filename).await {
                Ok(_) => info!("RDB loaded successfully"),
                Err(e) => warn!("Failed to load RDB: {}", e),
            }
        }

        // Initialize AOF manager
        let aof = if config.aof_enabled {
            let aof_path = Some(&config.aof_filename);
            AofManager::new(true, aof_path, config.aof_sync_policy).await?
        } else {
            AofManager::new(false, None::<&str>, config.aof_sync_policy).await?
        };

        // Load AOF (overrides RDB if both exist)
        if config.aof_enabled && std::path::Path::new(&config.aof_filename).exists() {
            info!("Loading AOF from {}", config.aof_filename);
            let reader = AofReader::new(&config.aof_filename);
            match reader.load(&db).await {
                Ok(count) => info!("AOF loaded {} commands", count),
                Err(e) => warn!("Failed to load AOF: {}", e),
            }
        }

        let repl_backlog = Arc::new(ReplicationBacklog::new());
        let propagator = Arc::new(CommandPropagator::new(Arc::clone(&repl_backlog)));

        // Initialize cluster if enabled
        let cluster = Arc::new(ClusterState::new(config.cluster_enabled));
        let migration = Arc::new(MigrationManager::new());

        // Load cluster configuration if exists and cluster is enabled
        if config.cluster_enabled && std::path::Path::new(&config.cluster_config_file).exists() {
            info!("Loading cluster config from {}", config.cluster_config_file);
            match load_cluster_config(&cluster, &config.cluster_config_file) {
                Ok(epoch) => info!("Cluster config loaded, epoch: {}", epoch),
                Err(e) => warn!("Failed to load cluster config: {}", e),
            }
        }

        Ok(Self {
            db,
            pubsub: Arc::new(PubSub::new()),
            aof: Arc::new(aof),
            app_config: Arc::new(Config::new()),
            script_cache: Arc::new(ScriptCache::new()),
            repl_info: Arc::new(ReplicationInfo::new()),
            repl_backlog,
            propagator,
            client_registry: Arc::new(ClientRegistry::new()),
            slowlog: Arc::new(SlowLog::new()),
            cluster,
            migration,
            config: Arc::new(config),
            limit_connections: Arc::new(Semaphore::new(max_connections)),
        })
    }

    pub async fn run(&self) -> anyhow::Result<()> {
        let listener = TcpListener::bind(self.config.addr()).await?;
        info!(
            "Redis-Rust server listening on {}",
            self.config.addr()
        );

        loop {
            // Wait for permit to accept new connection
            let permit = self
                .limit_connections
                .clone()
                .acquire_owned()
                .await
                .unwrap();

            let (socket, addr) = match listener.accept().await {
                Ok(conn) => conn,
                Err(e) => {
                    error!("Failed to accept connection: {}", e);
                    continue;
                }
            };

            info!("Accepted connection from {}", addr);

            // Get socket file descriptor (for client tracking)
            let fd = socket.as_raw_fd() as u64;
            let client_id = self.client_registry.register(addr.to_string(), fd);

            let db = self.db.clone();
            let pubsub = self.pubsub.clone();
            let config = self.config.clone();
            let app_config = self.app_config.clone();
            let aof = self.aof.clone();
            let script_cache = self.script_cache.clone();
            let repl_info = self.repl_info.clone();
            let repl_backlog = self.repl_backlog.clone();
            let propagator = self.propagator.clone();
            let client_registry = self.client_registry.clone();
            let client_registry_for_cleanup = client_registry.clone();
            let slowlog = self.slowlog.clone();
            let cluster = self.cluster.clone();
            let migration = self.migration.clone();

            // Spawn a new task to handle this connection
            tokio::spawn(async move {
                if let Err(e) = Self::handle_connection(
                    socket,
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
                ).await {
                    error!("Connection error: {}", e);
                }
                // Unregister client when connection closes
                client_registry_for_cleanup.unregister(client_id);
                // Drop permit when connection closes
                drop(permit);
            });
        }
    }

    async fn handle_connection(
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
    ) -> anyhow::Result<()> {
        let mut connection = Connection::new(
            socket,
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
        );
        connection.process().await
    }

    pub fn db(&self) -> &Arc<Database> {
        &self.db
    }

    pub fn aof(&self) -> &Arc<AofManager> {
        &self.aof
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_server_creation() {
        let config = ServerConfig::default();
        let server = RedisServer::new(config);
        assert_eq!(server.config.port, 6379);
    }
}
