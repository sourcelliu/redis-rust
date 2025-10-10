// End-to-end tests for configuration management (CONFIG commands)

use redis::{Commands, RedisResult};
use std::time::Duration;

mod common;

#[tokio::test]
async fn test_config_get() -> RedisResult<()> {
    let (server, mut client) = common::setup_test_server().await?;

    // Test CONFIG GET with specific key
    let result: Vec<String> = redis::cmd("CONFIG")
        .arg("GET")
        .arg("port")
        .query(&mut client)?;

    assert_eq!(result.len(), 2);
    assert_eq!(result[0], "port");
    assert_eq!(result[1], "6379");

    // Test CONFIG GET with pattern
    let result: Vec<String> = redis::cmd("CONFIG")
        .arg("GET")
        .arg("max*")
        .query(&mut client)?;

    assert!(result.len() >= 2);
    assert!(result.contains(&"maxclients".to_string()));

    common::teardown_test_server(server).await;
    Ok(())
}

#[tokio::test]
async fn test_config_set() -> RedisResult<()> {
    let (server, mut client) = common::setup_test_server().await?;

    // Test CONFIG SET
    let result: String = redis::cmd("CONFIG")
        .arg("SET")
        .arg("timeout")
        .arg("300")
        .query(&mut client)?;

    assert_eq!(result, "OK");

    // Verify the change
    let result: Vec<String> = redis::cmd("CONFIG")
        .arg("GET")
        .arg("timeout")
        .query(&mut client)?;

    assert_eq!(result[1], "300");

    common::teardown_test_server(server).await;
    Ok(())
}

#[tokio::test]
async fn test_config_set_validation() -> RedisResult<()> {
    let (server, mut client) = common::setup_test_server().await?;

    // Test setting invalid value
    let result: Result<String, redis::RedisError> = redis::cmd("CONFIG")
        .arg("SET")
        .arg("timeout")
        .arg("-1")
        .query(&mut client);

    assert!(result.is_err());

    // Test setting read-only parameter
    let result: Result<String, redis::RedisError> = redis::cmd("CONFIG")
        .arg("SET")
        .arg("port")
        .arg("6380")
        .query(&mut client);

    assert!(result.is_err());

    common::teardown_test_server(server).await;
    Ok(())
}

#[tokio::test]
async fn test_config_multiple_settings() -> RedisResult<()> {
    let (server, mut client) = common::setup_test_server().await?;

    // Set multiple configuration parameters
    let configs = vec![
        ("timeout", "300"),
        ("maxmemory", "1000000"),
        ("maxmemory-policy", "allkeys-lru"),
        ("slowlog-log-slower-than", "5000"),
    ];

    for (key, value) in &configs {
        let result: String = redis::cmd("CONFIG")
            .arg("SET")
            .arg(key)
            .arg(value)
            .query(&mut client)?;

        assert_eq!(result, "OK");
    }

    // Verify all changes
    for (key, expected_value) in &configs {
        let result: Vec<String> = redis::cmd("CONFIG")
            .arg("GET")
            .arg(key)
            .query(&mut client)?;

        assert_eq!(result[1], *expected_value);
    }

    common::teardown_test_server(server).await;
    Ok(())
}

#[tokio::test]
async fn test_config_resetstat() -> RedisResult<()> {
    let (server, mut client) = common::setup_test_server().await?;

    // Test CONFIG RESETSTAT
    let result: String = redis::cmd("CONFIG")
        .arg("RESETSTAT")
        .query(&mut client)?;

    assert_eq!(result, "OK");

    common::teardown_test_server(server).await;
    Ok(())
}

#[tokio::test]
async fn test_config_rewrite() -> RedisResult<()> {
    let (server, mut client) = common::setup_test_server().await?;

    // Set a configuration parameter
    let _: String = redis::cmd("CONFIG")
        .arg("SET")
        .arg("timeout")
        .arg("500")
        .query(&mut client)?;

    // Test CONFIG REWRITE
    let result: String = redis::cmd("CONFIG")
        .arg("REWRITE")
        .query(&mut client)?;

    assert_eq!(result, "OK");

    common::teardown_test_server(server).await;
    Ok(())
}

#[tokio::test]
async fn test_config_help() -> RedisResult<()> {
    let (server, mut client) = common::setup_test_server().await?;

    // Test CONFIG HELP
    let result: Vec<String> = redis::cmd("CONFIG")
        .arg("HELP")
        .query(&mut client)?;

    assert!(!result.is_empty());
    assert!(result.iter().any(|s| s.contains("GET")));
    assert!(result.iter().any(|s| s.contains("SET")));
    assert!(result.iter().any(|s| s.contains("REWRITE")));

    common::teardown_test_server(server).await;
    Ok(())
}

#[tokio::test]
async fn test_config_get_all() -> RedisResult<()> {
    let (server, mut client) = common::setup_test_server().await?;

    // Test CONFIG GET with wildcard to get all settings
    let result: Vec<String> = redis::cmd("CONFIG")
        .arg("GET")
        .arg("*")
        .query(&mut client)?;

    // Should return key-value pairs
    assert!(result.len() > 0);
    assert!(result.len() % 2 == 0);

    // Check for some expected keys
    assert!(result.contains(&"port".to_string()));
    assert!(result.contains(&"databases".to_string()));
    assert!(result.contains(&"timeout".to_string()));

    common::teardown_test_server(server).await;
    Ok(())
}

#[tokio::test]
async fn test_config_loglevel() -> RedisResult<()> {
    let (server, mut client) = common::setup_test_server().await?;

    let loglevels = vec!["debug", "verbose", "notice", "warning"];

    for level in loglevels {
        let result: String = redis::cmd("CONFIG")
            .arg("SET")
            .arg("loglevel")
            .arg(level)
            .query(&mut client)?;

        assert_eq!(result, "OK");

        let result: Vec<String> = redis::cmd("CONFIG")
            .arg("GET")
            .arg("loglevel")
            .query(&mut client)?;

        assert_eq!(result[1], level);
    }

    // Test invalid loglevel
    let result: Result<String, redis::RedisError> = redis::cmd("CONFIG")
        .arg("SET")
        .arg("loglevel")
        .arg("invalid")
        .query(&mut client);

    assert!(result.is_err());

    common::teardown_test_server(server).await;
    Ok(())
}

#[tokio::test]
async fn test_config_maxmemory_policy() -> RedisResult<()> {
    let (server, mut client) = common::setup_test_server().await?;

    let policies = vec![
        "volatile-lru",
        "allkeys-lru",
        "volatile-lfu",
        "allkeys-lfu",
        "volatile-random",
        "allkeys-random",
        "volatile-ttl",
        "noeviction",
    ];

    for policy in policies {
        let result: String = redis::cmd("CONFIG")
            .arg("SET")
            .arg("maxmemory-policy")
            .arg(policy)
            .query(&mut client)?;

        assert_eq!(result, "OK");

        let result: Vec<String> = redis::cmd("CONFIG")
            .arg("GET")
            .arg("maxmemory-policy")
            .query(&mut client)?;

        assert_eq!(result[1], policy);
    }

    common::teardown_test_server(server).await;
    Ok(())
}
