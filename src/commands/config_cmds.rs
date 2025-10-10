// CONFIG command implementation
// Provides runtime configuration management similar to Redis CONFIG commands

use crate::config::ConfigManager;
use crate::protocol::RespValue;
use std::sync::Arc;

pub struct ConfigCommands {
    config: Arc<ConfigManager>,
}

impl ConfigCommands {
    pub fn new(config: Arc<ConfigManager>) -> Self {
        Self { config }
    }

    /// Execute a CONFIG command
    pub fn execute(&self, args: &[String]) -> Result<RespValue, String> {
        if args.is_empty() {
            return Err("ERR wrong number of arguments for 'config' command".to_string());
        }

        let subcommand = args[0].to_uppercase();

        match subcommand.as_str() {
            "GET" => self.config_get(&args[1..]),
            "SET" => self.config_set(&args[1..]),
            "RESETSTAT" => self.config_resetstat(),
            "REWRITE" => self.config_rewrite(),
            "HELP" => Ok(self.config_help()),
            _ => Err(format!("ERR unknown CONFIG subcommand '{}'", subcommand)),
        }
    }

    /// CONFIG GET pattern
    /// Returns configuration parameters matching the pattern
    fn config_get(&self, args: &[String]) -> Result<RespValue, String> {
        if args.is_empty() {
            return Err("ERR wrong number of arguments for 'config get' command".to_string());
        }

        let pattern = &args[0];
        let all_config = self.config.get_all();

        let mut result = Vec::new();

        for (key, value) in all_config {
            if Self::match_pattern(&key, pattern) {
                result.push(RespValue::BulkString(key));
                result.push(RespValue::BulkString(value));
            }
        }

        Ok(RespValue::Array(result))
    }

    /// CONFIG SET parameter value
    /// Set a configuration parameter to a value
    fn config_set(&self, args: &[String]) -> Result<RespValue, String> {
        if args.len() < 2 {
            return Err("ERR wrong number of arguments for 'config set' command".to_string());
        }

        let key = args[0].clone();
        let value = args[1].clone();

        self.config
            .set(key.clone(), value.clone())
            .map_err(|e| format!("ERR {}", e))?;

        Ok(RespValue::SimpleString("OK".to_string()))
    }

    /// CONFIG RESETSTAT
    /// Reset statistics (placeholder for now)
    fn config_resetstat(&self) -> Result<RespValue, String> {
        // In a full implementation, this would reset server statistics
        Ok(RespValue::SimpleString("OK".to_string()))
    }

    /// CONFIG REWRITE
    /// Rewrite the configuration file with current values
    fn config_rewrite(&self) -> Result<RespValue, String> {
        self.config
            .rewrite()
            .map_err(|e| format!("ERR Failed to rewrite config: {}", e))?;

        Ok(RespValue::SimpleString("OK".to_string()))
    }

    /// CONFIG HELP
    /// Show help for CONFIG command
    fn config_help(&self) -> RespValue {
        let help_messages = vec![
            "CONFIG <subcommand> [<arg> [value] [opt] ...]. Subcommands are:",
            "GET <pattern>",
            "    Return parameters matching the glob-like <pattern> and their values.",
            "SET <directive> <value>",
            "    Set the configuration directive to <value>.",
            "RESETSTAT",
            "    Reset statistics reported by the INFO command.",
            "REWRITE",
            "    Rewrite the configuration file with the current configuration.",
            "HELP",
            "    Print this help.",
        ];

        RespValue::Array(
            help_messages
                .iter()
                .map(|s| RespValue::BulkString(s.to_string()))
                .collect(),
        )
    }

    /// Simple glob-style pattern matching
    fn match_pattern(text: &str, pattern: &str) -> bool {
        if pattern == "*" {
            return true;
        }

        if !pattern.contains('*') {
            return text == pattern;
        }

        if pattern.ends_with('*') {
            let prefix = &pattern[..pattern.len() - 1];
            return text.starts_with(prefix);
        }

        if pattern.starts_with('*') {
            let suffix = &pattern[1..];
            return text.ends_with(suffix);
        }

        text == pattern
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::ConfigManager;

    #[test]
    fn test_config_get() {
        let config = Arc::new(ConfigManager::new());
        let cmd = ConfigCommands::new(config.clone());

        let result = cmd.config_get(&["port".to_string()]);
        assert!(result.is_ok());
    }

    #[test]
    fn test_config_set() {
        let config = Arc::new(ConfigManager::new());
        let cmd = ConfigCommands::new(config.clone());

        let result = cmd.config_set(&["timeout".to_string(), "300".to_string()]);
        assert!(result.is_ok());

        assert_eq!(config.get("timeout"), Some("300".to_string()));
    }

    #[test]
    fn test_pattern_matching() {
        assert!(ConfigCommands::match_pattern("port", "*"));
        assert!(ConfigCommands::match_pattern("port", "port"));
        assert!(ConfigCommands::match_pattern("port", "po*"));
        assert!(ConfigCommands::match_pattern("port", "*rt"));
        assert!(!ConfigCommands::match_pattern("port", "timeout"));
    }
}
