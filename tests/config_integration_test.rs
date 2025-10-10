// Integration tests for static configuration file loading

use redis_rust::config::{ConfigManager, StaticConfig};
use std::fs;
use tempfile::TempDir;

#[test]
fn test_static_config_default() {
    let config = StaticConfig::new();

    assert_eq!(config.get_int("port"), Some(6379));
    assert_eq!(config.get_int("databases"), Some(16));
    assert_eq!(config.get_bool("appendonly"), Some(false));
    assert_eq!(config.get_string("bind"), Some("127.0.0.1".to_string()));
}

#[test]
fn test_static_config_from_file() {
    let temp_dir = TempDir::new().unwrap();
    let config_path = temp_dir.path().join("test.conf");

    let config_content = r#"
port 6380
databases 32
bind 0.0.0.0
timeout 300
maxclients 5000
appendonly yes
"#;

    fs::write(&config_path, config_content).unwrap();

    let config = StaticConfig::from_file(&config_path).unwrap();

    assert_eq!(config.get_int("port"), Some(6380));
    assert_eq!(config.get_int("databases"), Some(32));
    assert_eq!(config.get_string("bind"), Some("0.0.0.0".to_string()));
    assert_eq!(config.get_int("timeout"), Some(300));
    assert_eq!(config.get_int("maxclients"), Some(5000));
    assert_eq!(config.get_bool("appendonly"), Some(true));
}

#[test]
fn test_config_manager_default() {
    let config = ConfigManager::new();

    assert_eq!(config.get("port"), Some("6379".to_string()));
    assert_eq!(config.get("databases"), Some("16".to_string()));
}

#[test]
fn test_config_manager_from_file() {
    let temp_dir = TempDir::new().unwrap();
    let config_path = temp_dir.path().join("redis.conf");

    let config_content = r#"
port 6380
timeout 500
maxmemory 1000000
"#;

    fs::write(&config_path, config_content).unwrap();

    let config = ConfigManager::from_file(config_path.to_str().unwrap()).unwrap();

    assert_eq!(config.get("port"), Some("6380".to_string()));
    assert_eq!(config.get("timeout"), Some("500".to_string()));
    assert_eq!(config.get("maxmemory"), Some("1000000".to_string()));
}

#[test]
fn test_config_manager_set_get() {
    let config = ConfigManager::new();

    config.set("timeout".to_string(), "300".to_string()).unwrap();
    assert_eq!(config.get("timeout"), Some("300".to_string()));

    config.set("maxmemory".to_string(), "2000000".to_string()).unwrap();
    assert_eq!(config.get("maxmemory"), Some("2000000".to_string()));
}

#[test]
fn test_config_manager_read_only_keys() {
    let config = ConfigManager::new();

    // Try to set read-only keys (should fail)
    let result = config.set("port".to_string(), "6380".to_string());
    assert!(result.is_err());

    let result = config.set("databases".to_string(), "32".to_string());
    assert!(result.is_err());
}

#[test]
fn test_config_manager_typed_getters() {
    let config = ConfigManager::new();

    config.set("timeout".to_string(), "300".to_string()).unwrap();
    assert_eq!(config.get_int("timeout"), Some(300));

    config.set("appendonly".to_string(), "yes".to_string()).unwrap();
    assert_eq!(config.get_bool("appendonly"), Some(true));

    config.set("appendonly".to_string(), "no".to_string()).unwrap();
    assert_eq!(config.get_bool("appendonly"), Some(false));
}

#[test]
fn test_config_manager_rewrite() {
    let temp_dir = TempDir::new().unwrap();
    let config_path = temp_dir.path().join("redis.conf");

    // Create initial config
    let config_content = "port 6379\ntimeout 0\n";
    fs::write(&config_path, config_content).unwrap();

    let config = ConfigManager::from_file(config_path.to_str().unwrap()).unwrap();

    // Modify configuration
    config.set("timeout".to_string(), "300".to_string()).unwrap();
    config.set("maxmemory".to_string(), "1000000".to_string()).unwrap();

    // Rewrite config file
    config.rewrite().unwrap();

    // Read the file and verify
    let content = fs::read_to_string(&config_path).unwrap();
    assert!(content.contains("timeout"));
    assert!(content.contains("maxmemory"));
}

#[test]
fn test_config_validation() {
    let config = ConfigManager::new();

    // Test invalid timeout (negative)
    let result = config.set("timeout".to_string(), "-1".to_string());
    assert!(result.is_err());

    // Test invalid maxmemory-policy
    let result = config.set("maxmemory-policy".to_string(), "invalid".to_string());
    assert!(result.is_err());

    // Test valid maxmemory-policy
    let result = config.set("maxmemory-policy".to_string(), "allkeys-lru".to_string());
    assert!(result.is_ok());
}

#[test]
fn test_config_with_comments() {
    let temp_dir = TempDir::new().unwrap();
    let config_path = temp_dir.path().join("redis.conf");

    let config_content = r#"
# This is a comment
port 6379

# Another comment
timeout 300
databases 16
"#;

    fs::write(&config_path, config_content).unwrap();

    let config = StaticConfig::from_file(&config_path).unwrap();

    assert_eq!(config.get_int("port"), Some(6379));
    assert_eq!(config.get_int("timeout"), Some(300));
    assert_eq!(config.get_int("databases"), Some(16));
}

#[test]
fn test_config_list_values() {
    let temp_dir = TempDir::new().unwrap();
    let config_path = temp_dir.path().join("redis.conf");

    let config_content = "save 900 1 300 10 60 10000\n";

    fs::write(&config_path, config_content).unwrap();

    let config = StaticConfig::from_file(&config_path).unwrap();

    let save_value = config.get("save").unwrap();
    let save_str = save_value.to_string();
    assert!(save_str.contains("900"));
    assert!(save_str.contains("10000"));
}
