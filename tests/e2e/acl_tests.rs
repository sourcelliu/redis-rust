// End-to-end tests for ACL (Access Control List) functionality

use redis::{Commands, RedisResult};
use std::time::Duration;

mod common;

#[tokio::test]
async fn test_acl_list() -> RedisResult<()> {
    let (server, mut client) = common::setup_test_server().await?;

    // Test ACL LIST
    let result: Vec<String> = redis::cmd("ACL")
        .arg("LIST")
        .query(&mut client)?;

    // Should have at least the default user
    assert!(!result.is_empty());
    assert!(result.iter().any(|s| s.contains("default")));

    common::teardown_test_server(server).await;
    Ok(())
}

#[tokio::test]
async fn test_acl_users() -> RedisResult<()> {
    let (server, mut client) = common::setup_test_server().await?;

    // Test ACL USERS
    let result: Vec<String> = redis::cmd("ACL")
        .arg("USERS")
        .query(&mut client)?;

    // Should have at least the default user
    assert!(result.contains(&"default".to_string()));

    common::teardown_test_server(server).await;
    Ok(())
}

#[tokio::test]
async fn test_acl_setuser_basic() -> RedisResult<()> {
    let (server, mut client) = common::setup_test_server().await?;

    // Create a new user
    let result: String = redis::cmd("ACL")
        .arg("SETUSER")
        .arg("alice")
        .arg("on")
        .arg(">password123")
        .arg("allkeys")
        .arg("+@all")
        .query(&mut client)?;

    assert_eq!(result, "OK");

    // Verify the user was created
    let users: Vec<String> = redis::cmd("ACL")
        .arg("USERS")
        .query(&mut client)?;

    assert!(users.contains(&"alice".to_string()));

    common::teardown_test_server(server).await;
    Ok(())
}

#[tokio::test]
async fn test_acl_setuser_rules() -> RedisResult<()> {
    let (server, mut client) = common::setup_test_server().await?;

    // Create user with specific permissions
    let result: String = redis::cmd("ACL")
        .arg("SETUSER")
        .arg("bob")
        .arg("on")
        .arg(">secret")
        .arg("~user:*")
        .arg("+@read")
        .arg("+@write")
        .query(&mut client)?;

    assert_eq!(result, "OK");

    // Get user details
    let result: Vec<redis::Value> = redis::cmd("ACL")
        .arg("GETUSER")
        .arg("bob")
        .query(&mut client)?;

    assert!(!result.is_empty());

    common::teardown_test_server(server).await;
    Ok(())
}

#[tokio::test]
async fn test_acl_getuser() -> RedisResult<()> {
    let (server, mut client) = common::setup_test_server().await?;

    // Create a user first
    let _: String = redis::cmd("ACL")
        .arg("SETUSER")
        .arg("charlie")
        .arg("on")
        .arg(">mypassword")
        .arg("allkeys")
        .arg("+@all")
        .query(&mut client)?;

    // Get user details
    let result: Vec<redis::Value> = redis::cmd("ACL")
        .arg("GETUSER")
        .arg("charlie")
        .query(&mut client)?;

    assert!(!result.is_empty());

    common::teardown_test_server(server).await;
    Ok(())
}

#[tokio::test]
async fn test_acl_deluser() -> RedisResult<()> {
    let (server, mut client) = common::setup_test_server().await?;

    // Create users
    let _: String = redis::cmd("ACL")
        .arg("SETUSER")
        .arg("temp1")
        .arg("on")
        .query(&mut client)?;

    let _: String = redis::cmd("ACL")
        .arg("SETUSER")
        .arg("temp2")
        .arg("on")
        .query(&mut client)?;

    // Delete users
    let result: i32 = redis::cmd("ACL")
        .arg("DELUSER")
        .arg("temp1")
        .arg("temp2")
        .query(&mut client)?;

    assert_eq!(result, 2);

    // Verify users are deleted
    let users: Vec<String> = redis::cmd("ACL")
        .arg("USERS")
        .query(&mut client)?;

    assert!(!users.contains(&"temp1".to_string()));
    assert!(!users.contains(&"temp2".to_string()));

    common::teardown_test_server(server).await;
    Ok(())
}

#[tokio::test]
async fn test_acl_cat() -> RedisResult<()> {
    let (server, mut client) = common::setup_test_server().await?;

    // List all categories
    let result: Vec<String> = redis::cmd("ACL")
        .arg("CAT")
        .query(&mut client)?;

    assert!(!result.is_empty());
    assert!(result.contains(&"@read".to_string()));
    assert!(result.contains(&"@write".to_string()));
    assert!(result.contains(&"@admin".to_string()));

    // List commands in a category
    let result: Vec<String> = redis::cmd("ACL")
        .arg("CAT")
        .arg("@read")
        .query(&mut client)?;

    assert!(!result.is_empty());
    assert!(result.contains(&"GET".to_string()) || result.iter().any(|s| s.to_uppercase().contains("GET")));

    common::teardown_test_server(server).await;
    Ok(())
}

#[tokio::test]
async fn test_acl_whoami() -> RedisResult<()> {
    let (server, mut client) = common::setup_test_server().await?;

    // Test ACL WHOAMI
    let result: String = redis::cmd("ACL")
        .arg("WHOAMI")
        .query(&mut client)?;

    assert_eq!(result, "default");

    common::teardown_test_server(server).await;
    Ok(())
}

#[tokio::test]
async fn test_acl_save_load() -> RedisResult<()> {
    let (server, mut client) = common::setup_test_server().await?;

    // Create a user
    let _: String = redis::cmd("ACL")
        .arg("SETUSER")
        .arg("persistent")
        .arg("on")
        .arg(">password")
        .query(&mut client)?;

    // Save ACL
    let result: String = redis::cmd("ACL")
        .arg("SAVE")
        .query(&mut client)?;

    assert_eq!(result, "OK");

    // Load ACL
    let result: String = redis::cmd("ACL")
        .arg("LOAD")
        .query(&mut client)?;

    assert_eq!(result, "OK");

    common::teardown_test_server(server).await;
    Ok(())
}

#[tokio::test]
async fn test_acl_help() -> RedisResult<()> {
    let (server, mut client) = common::setup_test_server().await?;

    // Test ACL HELP
    let result: Vec<String> = redis::cmd("ACL")
        .arg("HELP")
        .query(&mut client)?;

    assert!(!result.is_empty());
    assert!(result.iter().any(|s| s.contains("LIST")));
    assert!(result.iter().any(|s| s.contains("SETUSER")));
    assert!(result.iter().any(|s| s.contains("DELUSER")));

    common::teardown_test_server(server).await;
    Ok(())
}

#[tokio::test]
async fn test_acl_user_disabled() -> RedisResult<()> {
    let (server, mut client) = common::setup_test_server().await?;

    // Create a disabled user
    let result: String = redis::cmd("ACL")
        .arg("SETUSER")
        .arg("disabled_user")
        .arg("off")
        .arg(">password")
        .arg("allkeys")
        .arg("+@all")
        .query(&mut client)?;

    assert_eq!(result, "OK");

    // Verify user exists but is disabled
    let users: Vec<String> = redis::cmd("ACL")
        .arg("USERS")
        .query(&mut client)?;

    assert!(users.contains(&"disabled_user".to_string()));

    common::teardown_test_server(server).await;
    Ok(())
}

#[tokio::test]
async fn test_acl_key_patterns() -> RedisResult<()> {
    let (server, mut client) = common::setup_test_server().await?;

    // Create user with specific key patterns
    let result: String = redis::cmd("ACL")
        .arg("SETUSER")
        .arg("restricted")
        .arg("on")
        .arg(">password")
        .arg("~user:*")
        .arg("~session:*")
        .arg("+@all")
        .query(&mut client)?;

    assert_eq!(result, "OK");

    // Get user details
    let result: Vec<redis::Value> = redis::cmd("ACL")
        .arg("GETUSER")
        .arg("restricted")
        .query(&mut client)?;

    assert!(!result.is_empty());

    common::teardown_test_server(server).await;
    Ok(())
}

#[tokio::test]
async fn test_acl_command_categories() -> RedisResult<()> {
    let (server, mut client) = common::setup_test_server().await?;

    // Create user with specific command categories
    let result: String = redis::cmd("ACL")
        .arg("SETUSER")
        .arg("readonly")
        .arg("on")
        .arg(">password")
        .arg("allkeys")
        .arg("+@read")
        .arg("-@write")
        .query(&mut client)?;

    assert_eq!(result, "OK");

    // Create user with multiple categories
    let result: String = redis::cmd("ACL")
        .arg("SETUSER")
        .arg("multi_cat")
        .arg("on")
        .arg(">password")
        .arg("allkeys")
        .arg("+@read")
        .arg("+@string")
        .arg("+@hash")
        .query(&mut client)?;

    assert_eq!(result, "OK");

    common::teardown_test_server(server).await;
    Ok(())
}

#[tokio::test]
async fn test_acl_nopass_user() -> RedisResult<()> {
    let (server, mut client) = common::setup_test_server().await?;

    // Create user without password
    let result: String = redis::cmd("ACL")
        .arg("SETUSER")
        .arg("nopass_user")
        .arg("on")
        .arg("nopass")
        .arg("allkeys")
        .arg("+@all")
        .query(&mut client)?;

    assert_eq!(result, "OK");

    // Verify user exists
    let users: Vec<String> = redis::cmd("ACL")
        .arg("USERS")
        .query(&mut client)?;

    assert!(users.contains(&"nopass_user".to_string()));

    common::teardown_test_server(server).await;
    Ok(())
}
