// E2E Test Framework - Common utilities for end-to-end testing
//
// This module provides utilities to start/stop Redis-Rust servers,
// create test clusters, and interact with them using the redis crate.

use redis::{Client, Connection, RedisResult};
use std::process::{Child, Command};
use std::time::Duration;
use tokio::time::sleep;

/// A test Redis server instance
pub struct TestRedisServer {
    #[allow(dead_code)]
    process: Option<Child>,
    pub port: u16,
    pub client: Client,
}

impl TestRedisServer {
    /// Start a new Redis server instance for testing
    pub async fn start() -> Self {
        Self::start_with_port(6379).await
    }

    /// Start a Redis server on a specific port
    pub async fn start_with_port(port: u16) -> Self {
        // TODO: Start the actual redis-rust server process
        // For now, this is a placeholder that assumes a server is already running

        let client = Client::open(format!("redis://127.0.0.1:{}", port))
            .expect("Failed to create Redis client");

        // Wait for server to be ready
        sleep(Duration::from_millis(100)).await;

        Self {
            process: None,
            port,
            client,
        }
    }

    /// Get an async connection to the server
    pub async fn get_async_connection(&self) -> RedisResult<redis::aio::Connection> {
        self.client.get_async_connection().await
    }

    /// Get a blocking connection to the server
    pub fn get_connection(&self) -> RedisResult<Connection> {
        self.client.get_connection()
    }

    /// Flush all data from the server
    pub async fn flush_all(&self) -> RedisResult<()> {
        let mut conn = self.get_async_connection().await?;
        redis::cmd("FLUSHALL").query_async(&mut conn).await
    }

    /// Stop the server
    pub async fn stop(mut self) {
        if let Some(mut process) = self.process.take() {
            let _ = process.kill();
            let _ = process.wait();
        }
    }
}

impl Drop for TestRedisServer {
    fn drop(&mut self) {
        if let Some(mut process) = self.process.take() {
            let _ = process.kill();
        }
    }
}

/// A test Redis cluster
pub struct TestCluster {
    pub nodes: Vec<TestRedisServer>,
}

impl TestCluster {
    /// Create a new test cluster with specified number of masters and replicas per master
    pub async fn create(masters: usize, replicas_per_master: usize) -> Self {
        let mut nodes = Vec::new();
        let base_port = 7000;

        // Create master nodes
        for i in 0..masters {
            let port = base_port + i as u16;
            nodes.push(TestRedisServer::start_with_port(port).await);
        }

        // Create replica nodes
        for i in 0..masters {
            for j in 0..replicas_per_master {
                let port = base_port + (masters as u16) + (i * replicas_per_master + j) as u16;
                nodes.push(TestRedisServer::start_with_port(port).await);
            }
        }

        Self { nodes }
    }

    /// Get a specific node by index
    pub fn node(&self, index: usize) -> &TestRedisServer {
        &self.nodes[index]
    }

    /// Stop all nodes in the cluster
    pub async fn stop(self) {
        for node in self.nodes {
            node.stop().await;
        }
    }
}

/// Helper function to compare Redis command results
pub fn assert_redis_eq<T: std::fmt::Debug + PartialEq>(actual: T, expected: T) {
    assert_eq!(actual, expected, "Redis command result mismatch");
}

/// Helper function to execute a command and return the result
#[allow(dead_code)]
pub async fn exec_cmd<T: redis::FromRedisValue>(
    conn: &mut redis::aio::Connection,
    cmd: &str,
    args: &[&str],
) -> RedisResult<T> {
    let mut command = redis::cmd(cmd);
    for arg in args {
        command.arg(*arg);
    }
    command.query_async(conn).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[ignore] // Ignore until server is implemented
    async fn test_server_start_stop() {
        let server = TestRedisServer::start().await;
        assert_eq!(server.port, 6379);
        server.stop().await;
    }

    #[tokio::test]
    #[ignore] // Ignore until server is implemented
    async fn test_cluster_creation() {
        let cluster = TestCluster::create(3, 1).await;
        assert_eq!(cluster.nodes.len(), 6); // 3 masters + 3 replicas
        cluster.stop().await;
    }
}
