use redis_rust::server::{RedisServer, ServerConfig};
use tracing::info;
use tracing_subscriber;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_target(false)
        .with_thread_ids(true)
        .with_level(true)
        .init();

    info!("Redis-Rust server starting...");

    // Create server configuration
    let config = ServerConfig::default();

    info!("Server will bind to {}", config.addr());
    info!("AOF enabled: {}", config.aof_enabled);
    info!("RDB enabled: {}", config.rdb_enabled);

    // Create and run server
    let server = RedisServer::new(config).await?;
    server.run().await?;

    Ok(())
}
