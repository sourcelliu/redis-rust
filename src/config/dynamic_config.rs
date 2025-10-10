// Dynamic configuration that can be modified at runtime
// Similar to Redis CONFIG SET/GET commands

use std::collections::HashMap;
use std::sync::RwLock;
use std::fs;
use anyhow::{Result, bail};

use super::static_config::{StaticConfig, ConfigValue};
use super::parser::format_config;

/// Dynamic configuration that can be changed at runtime
pub struct DynamicConfig {
    /// Configuration values
    values: RwLock<HashMap<String, String>>,
    /// List of read-only configuration keys that cannot be changed at runtime
    read_only_keys: Vec<String>,
}

impl DynamicConfig {
    /// Create a new dynamic configuration with default values
    pub fn new() -> Self {
        let mut values = HashMap::new();

        // Default runtime configuration
        values.insert("timeout".to_string(), "0".to_string());
        values.insert("tcp-keepalive".to_string(), "300".to_string());
        values.insert("loglevel".to_string(), "notice".to_string());
        values.insert("maxclients".to_string(), "10000".to_string());
        values.insert("maxmemory".to_string(), "0".to_string());
        values.insert("maxmemory-policy".to_string(), "noeviction".to_string());
        values.insert("appendfsync".to_string(), "everysec".to_string());
        values.insert("slowlog-log-slower-than".to_string(), "10000".to_string());
        values.insert("slowlog-max-len".to_string(), "128".to_string());
        values.insert("requirepass".to_string(), "".to_string());

        // Read-only keys (cannot be changed at runtime)
        let read_only_keys = vec![
            "bind".to_string(),
            "port".to_string(),
            "daemonize".to_string(),
            "databases".to_string(),
            "dir".to_string(),
            "cluster-enabled".to_string(),
        ];

        Self {
            values: RwLock::new(values),
            read_only_keys,
        }
    }

    /// Create dynamic config from static config
    pub fn from_static(static_config: &StaticConfig) -> Self {
        let mut config = Self::new();

        // Copy all non-read-only values from static config
        for (key, value) in static_config.get_all() {
            if !config.read_only_keys.contains(key) {
                config.values.write().unwrap().insert(key.clone(), value.to_string());
            }
        }

        config
    }

    /// Get a configuration value
    pub fn get(&self, key: &str) -> Option<String> {
        self.values.read().unwrap().get(key).cloned()
    }

    /// Set a configuration value
    pub fn set(&self, key: String, value: String) -> Result<()> {
        // Check if the key is read-only
        if self.read_only_keys.contains(&key) {
            bail!("Configuration parameter '{}' cannot be changed at runtime", key);
        }

        // Validate the configuration value
        self.validate(&key, &value)?;

        // Set the value
        self.values.write().unwrap().insert(key, value);
        Ok(())
    }

    /// Get all configuration values
    pub fn get_all(&self) -> Vec<(String, String)> {
        self.values
            .read()
            .unwrap()
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect()
    }

    /// Reset a configuration value to its default
    pub fn reset(&self, key: &str) -> Result<()> {
        if self.read_only_keys.contains(&key.to_string()) {
            bail!("Configuration parameter '{}' cannot be changed at runtime", key);
        }

        // Get default value
        let default_config = Self::new();
        if let Some(default_value) = default_config.get(key) {
            self.values.write().unwrap().insert(key.to_string(), default_value);
            Ok(())
        } else {
            bail!("Unknown configuration parameter '{}'", key);
        }
    }

    /// Write configuration to a file
    pub fn write_to_file(&self, path: &str) -> Result<()> {
        let values = self.values.read().unwrap().clone();
        let formatted = format_config(&values);
        fs::write(path, formatted)?;
        Ok(())
    }

    /// Validate a configuration value
    fn validate(&self, key: &str, value: &str) -> Result<()> {
        match key {
            "timeout" => {
                let timeout: i64 = value.parse()
                    .map_err(|_| anyhow::anyhow!("Invalid timeout value"))?;
                if timeout < 0 {
                    bail!("Timeout must be non-negative");
                }
            }
            "tcp-keepalive" => {
                let keepalive: i64 = value.parse()
                    .map_err(|_| anyhow::anyhow!("Invalid tcp-keepalive value"))?;
                if keepalive < 0 {
                    bail!("tcp-keepalive must be non-negative");
                }
            }
            "maxclients" => {
                let max: i64 = value.parse()
                    .map_err(|_| anyhow::anyhow!("Invalid maxclients value"))?;
                if max < 1 {
                    bail!("maxclients must be at least 1");
                }
            }
            "maxmemory" => {
                let mem: i64 = value.parse()
                    .map_err(|_| anyhow::anyhow!("Invalid maxmemory value"))?;
                if mem < 0 {
                    bail!("maxmemory must be non-negative");
                }
            }
            "maxmemory-policy" => {
                let valid_policies = ["volatile-lru", "allkeys-lru", "volatile-lfu",
                                     "allkeys-lfu", "volatile-random", "allkeys-random",
                                     "volatile-ttl", "noeviction"];
                if !valid_policies.contains(&value) {
                    bail!("Invalid maxmemory-policy. Valid values: {}", valid_policies.join(", "));
                }
            }
            "loglevel" => {
                let valid_levels = ["debug", "verbose", "notice", "warning"];
                if !valid_levels.contains(&value) {
                    bail!("Invalid loglevel. Valid values: {}", valid_levels.join(", "));
                }
            }
            "appendfsync" => {
                let valid_values = ["always", "everysec", "no"];
                if !valid_values.contains(&value) {
                    bail!("Invalid appendfsync. Valid values: {}", valid_values.join(", "));
                }
            }
            "slowlog-log-slower-than" => {
                let _: i64 = value.parse()
                    .map_err(|_| anyhow::anyhow!("Invalid slowlog-log-slower-than value"))?;
            }
            "slowlog-max-len" => {
                let len: i64 = value.parse()
                    .map_err(|_| anyhow::anyhow!("Invalid slowlog-max-len value"))?;
                if len < 0 {
                    bail!("slowlog-max-len must be non-negative");
                }
            }
            _ => {
                // Allow unknown keys for forward compatibility
            }
        }

        Ok(())
    }

    /// Check if a key is read-only
    pub fn is_read_only(&self, key: &str) -> bool {
        self.read_only_keys.contains(&key.to_string())
    }
}

impl Default for DynamicConfig {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_set() {
        let config = DynamicConfig::new();
        assert_eq!(config.get("timeout"), Some("0".to_string()));

        config.set("timeout".to_string(), "300".to_string()).unwrap();
        assert_eq!(config.get("timeout"), Some("300".to_string()));
    }

    #[test]
    fn test_read_only_keys() {
        let config = DynamicConfig::new();
        let result = config.set("port".to_string(), "6380".to_string());
        assert!(result.is_err());
        assert!(config.is_read_only("port"));
    }

    #[test]
    fn test_validation() {
        let config = DynamicConfig::new();

        // Invalid timeout
        let result = config.set("timeout".to_string(), "-1".to_string());
        assert!(result.is_err());

        // Invalid maxmemory-policy
        let result = config.set("maxmemory-policy".to_string(), "invalid".to_string());
        assert!(result.is_err());

        // Valid value
        let result = config.set("maxmemory-policy".to_string(), "allkeys-lru".to_string());
        assert!(result.is_ok());
    }

    #[test]
    fn test_reset() {
        let config = DynamicConfig::new();
        config.set("timeout".to_string(), "999".to_string()).unwrap();
        assert_eq!(config.get("timeout"), Some("999".to_string()));

        config.reset("timeout").unwrap();
        assert_eq!(config.get("timeout"), Some("0".to_string()));
    }
}
