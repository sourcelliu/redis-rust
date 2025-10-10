// Configuration management system inspired by Redis
// Supports static configuration (from file) and dynamic configuration (runtime changes)

mod static_config;
mod dynamic_config;
mod parser;

pub use static_config::{StaticConfig, ConfigValue};
pub use dynamic_config::DynamicConfig;
pub use parser::ConfigParser;

use std::sync::Arc;
use anyhow::Result;

/// Unified configuration manager that combines static and dynamic configuration
pub struct ConfigManager {
    /// Static configuration loaded from file
    static_config: StaticConfig,
    /// Dynamic configuration that can be modified at runtime
    dynamic_config: Arc<DynamicConfig>,
    /// Path to the configuration file
    config_file: Option<String>,
}

impl ConfigManager {
    /// Create a new configuration manager with default settings
    pub fn new() -> Self {
        Self {
            static_config: StaticConfig::default(),
            dynamic_config: Arc::new(DynamicConfig::new()),
            config_file: None,
        }
    }

    /// Load configuration from a file
    pub fn from_file(path: &str) -> Result<Self> {
        let static_config = StaticConfig::from_file(path)?;
        let dynamic_config = Arc::new(DynamicConfig::from_static(&static_config));

        Ok(Self {
            static_config,
            dynamic_config,
            config_file: Some(path.to_string()),
        })
    }

    /// Get a configuration value (checks dynamic config first, then static)
    pub fn get(&self, key: &str) -> Option<String> {
        self.dynamic_config.get(key)
            .or_else(|| self.static_config.get(key).map(|v| v.to_string()))
    }

    /// Set a configuration value at runtime
    pub fn set(&self, key: String, value: String) -> Result<()> {
        self.dynamic_config.set(key, value)
    }

    /// Rewrite the configuration file with current dynamic values
    pub fn rewrite(&self) -> Result<()> {
        if let Some(path) = &self.config_file {
            self.dynamic_config.write_to_file(path)?;
        }
        Ok(())
    }

    /// Get all configuration as key-value pairs
    pub fn get_all(&self) -> Vec<(String, String)> {
        self.dynamic_config.get_all()
    }

    /// Get the dynamic config handle for direct access
    pub fn dynamic(&self) -> Arc<DynamicConfig> {
        Arc::clone(&self.dynamic_config)
    }

    /// Get a typed configuration value
    pub fn get_int(&self, key: &str) -> Option<i64> {
        self.get(key).and_then(|v| v.parse().ok())
    }

    /// Get a boolean configuration value
    pub fn get_bool(&self, key: &str) -> Option<bool> {
        self.get(key).and_then(|v| {
            match v.to_lowercase().as_str() {
                "yes" | "true" | "1" => Some(true),
                "no" | "false" | "0" => Some(false),
                _ => None,
            }
        })
    }

    /// Get the configuration file path
    pub fn config_file(&self) -> Option<&str> {
        self.config_file.as_deref()
    }
}

impl Default for ConfigManager {
    fn default() -> Self {
        Self::new()
    }
}

// Legacy Config type for backward compatibility
pub type Config = ConfigManager;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = ConfigManager::new();
        assert_eq!(config.get("port"), Some("6379".to_string()));
        assert_eq!(config.get("databases"), Some("16".to_string()));
    }

    #[test]
    fn test_dynamic_config() {
        let config = ConfigManager::new();
        config.set("maxmemory".to_string(), "1000000".to_string()).unwrap();
        assert_eq!(config.get("maxmemory"), Some("1000000".to_string()));
    }

    #[test]
    fn test_typed_getters() {
        let config = ConfigManager::new();
        config.set("timeout".to_string(), "300".to_string()).unwrap();
        assert_eq!(config.get_int("timeout"), Some(300));

        config.set("appendonly".to_string(), "yes".to_string()).unwrap();
        assert_eq!(config.get_bool("appendonly"), Some(true));
    }
}

