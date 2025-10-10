// Static configuration loaded from configuration file
// Similar to Redis's redis.conf

use std::collections::HashMap;
use std::fs;
use std::path::Path;
use anyhow::{Result, Context};

use super::parser::ConfigParser;

/// Represents different types of configuration values
#[derive(Debug, Clone, PartialEq)]
pub enum ConfigValue {
    String(String),
    Int(i64),
    Bool(bool),
    Float(f64),
    List(Vec<String>),
}

impl ConfigValue {
    pub fn to_string(&self) -> String {
        match self {
            ConfigValue::String(s) => s.clone(),
            ConfigValue::Int(i) => i.to_string(),
            ConfigValue::Bool(b) => if *b { "yes" } else { "no" }.to_string(),
            ConfigValue::Float(f) => f.to_string(),
            ConfigValue::List(list) => list.join(" "),
        }
    }

    pub fn as_int(&self) -> Option<i64> {
        match self {
            ConfigValue::Int(i) => Some(*i),
            ConfigValue::String(s) => s.parse().ok(),
            _ => None,
        }
    }

    pub fn as_bool(&self) -> Option<bool> {
        match self {
            ConfigValue::Bool(b) => Some(*b),
            ConfigValue::String(s) => match s.to_lowercase().as_str() {
                "yes" | "true" | "1" => Some(true),
                "no" | "false" | "0" => Some(false),
                _ => None,
            },
            ConfigValue::Int(i) => Some(*i != 0),
            _ => None,
        }
    }
}

/// Static configuration settings
#[derive(Debug, Clone)]
pub struct StaticConfig {
    values: HashMap<String, ConfigValue>,
}

impl StaticConfig {
    /// Create a new static configuration with default values
    pub fn new() -> Self {
        let mut values = HashMap::new();

        // Network settings
        values.insert("bind".to_string(), ConfigValue::String("127.0.0.1".to_string()));
        values.insert("port".to_string(), ConfigValue::Int(6379));
        values.insert("tcp-backlog".to_string(), ConfigValue::Int(511));
        values.insert("timeout".to_string(), ConfigValue::Int(0));
        values.insert("tcp-keepalive".to_string(), ConfigValue::Int(300));

        // General settings
        values.insert("daemonize".to_string(), ConfigValue::Bool(false));
        values.insert("databases".to_string(), ConfigValue::Int(16));
        values.insert("loglevel".to_string(), ConfigValue::String("notice".to_string()));
        values.insert("logfile".to_string(), ConfigValue::String("".to_string()));

        // Snapshotting (RDB)
        values.insert("save".to_string(), ConfigValue::List(vec![
            "900".to_string(), "1".to_string(),
            "300".to_string(), "10".to_string(),
            "60".to_string(), "10000".to_string(),
        ]));
        values.insert("stop-writes-on-bgsave-error".to_string(), ConfigValue::Bool(true));
        values.insert("rdbcompression".to_string(), ConfigValue::Bool(true));
        values.insert("rdbchecksum".to_string(), ConfigValue::Bool(true));
        values.insert("dbfilename".to_string(), ConfigValue::String("dump.rdb".to_string()));
        values.insert("dir".to_string(), ConfigValue::String("./".to_string()));

        // Replication
        values.insert("replica-serve-stale-data".to_string(), ConfigValue::Bool(true));
        values.insert("replica-read-only".to_string(), ConfigValue::Bool(true));
        values.insert("repl-diskless-sync".to_string(), ConfigValue::Bool(false));
        values.insert("repl-diskless-sync-delay".to_string(), ConfigValue::Int(5));

        // Security
        values.insert("requirepass".to_string(), ConfigValue::String("".to_string()));

        // Limits
        values.insert("maxclients".to_string(), ConfigValue::Int(10000));
        values.insert("maxmemory".to_string(), ConfigValue::Int(0));
        values.insert("maxmemory-policy".to_string(), ConfigValue::String("noeviction".to_string()));

        // Append Only File (AOF)
        values.insert("appendonly".to_string(), ConfigValue::Bool(false));
        values.insert("appendfilename".to_string(), ConfigValue::String("appendonly.aof".to_string()));
        values.insert("appendfsync".to_string(), ConfigValue::String("everysec".to_string()));
        values.insert("no-appendfsync-on-rewrite".to_string(), ConfigValue::Bool(false));
        values.insert("auto-aof-rewrite-percentage".to_string(), ConfigValue::Int(100));
        values.insert("auto-aof-rewrite-min-size".to_string(), ConfigValue::Int(67108864)); // 64MB

        // Slow log
        values.insert("slowlog-log-slower-than".to_string(), ConfigValue::Int(10000));
        values.insert("slowlog-max-len".to_string(), ConfigValue::Int(128));

        // Advanced config
        values.insert("hash-max-ziplist-entries".to_string(), ConfigValue::Int(512));
        values.insert("hash-max-ziplist-value".to_string(), ConfigValue::Int(64));
        values.insert("list-max-ziplist-size".to_string(), ConfigValue::Int(-2));
        values.insert("set-max-intset-entries".to_string(), ConfigValue::Int(512));
        values.insert("zset-max-ziplist-entries".to_string(), ConfigValue::Int(128));
        values.insert("zset-max-ziplist-value".to_string(), ConfigValue::Int(64));

        // Cluster
        values.insert("cluster-enabled".to_string(), ConfigValue::Bool(false));
        values.insert("cluster-config-file".to_string(), ConfigValue::String("nodes.conf".to_string()));
        values.insert("cluster-node-timeout".to_string(), ConfigValue::Int(15000));

        Self { values }
    }

    /// Load configuration from a file
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        let content = fs::read_to_string(path.as_ref())
            .context("Failed to read configuration file")?;

        let parser = ConfigParser::new(&content);
        let parsed_values = parser.parse()?;

        let mut config = Self::new();
        for (key, value) in parsed_values {
            config.values.insert(key, value);
        }

        Ok(config)
    }

    /// Get a configuration value
    pub fn get(&self, key: &str) -> Option<&ConfigValue> {
        self.values.get(key)
    }

    /// Get all configuration values
    pub fn get_all(&self) -> &HashMap<String, ConfigValue> {
        &self.values
    }

    /// Get configuration value as string
    pub fn get_string(&self, key: &str) -> Option<String> {
        self.values.get(key).map(|v| v.to_string())
    }

    /// Get configuration value as integer
    pub fn get_int(&self, key: &str) -> Option<i64> {
        self.values.get(key).and_then(|v| v.as_int())
    }

    /// Get configuration value as boolean
    pub fn get_bool(&self, key: &str) -> Option<bool> {
        self.values.get(key).and_then(|v| v.as_bool())
    }
}

impl Default for StaticConfig {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = StaticConfig::new();
        assert_eq!(config.get_int("port"), Some(6379));
        assert_eq!(config.get_int("databases"), Some(16));
        assert_eq!(config.get_bool("appendonly"), Some(false));
        assert_eq!(config.get_string("bind"), Some("127.0.0.1".to_string()));
    }

    #[test]
    fn test_config_value_conversion() {
        let value = ConfigValue::Bool(true);
        assert_eq!(value.to_string(), "yes");
        assert_eq!(value.as_bool(), Some(true));

        let value = ConfigValue::Int(42);
        assert_eq!(value.to_string(), "42");
        assert_eq!(value.as_int(), Some(42));
    }
}
