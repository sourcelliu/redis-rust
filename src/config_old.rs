// Server configuration management

use std::collections::HashMap;
use std::sync::RwLock;

pub struct Config {
    settings: RwLock<HashMap<String, String>>,
}

impl Config {
    pub fn new() -> Self {
        let mut settings = HashMap::new();
        
        // Default configuration values
        settings.insert("databases".to_string(), "16".to_string());
        settings.insert("port".to_string(), "6379".to_string());
        settings.insert("timeout".to_string(), "0".to_string());
        settings.insert("maxclients".to_string(), "10000".to_string());
        settings.insert("save".to_string(), "900 1 300 10 60 10000".to_string());
        settings.insert("appendonly".to_string(), "no".to_string());
        settings.insert("appendfsync".to_string(), "everysec".to_string());
        settings.insert("slowlog-log-slower-than".to_string(), "10000".to_string());
        settings.insert("slowlog-max-len".to_string(), "128".to_string());
        
        Self {
            settings: RwLock::new(settings),
        }
    }
    
    pub fn get(&self, key: &str) -> Option<String> {
        self.settings.read().unwrap().get(key).cloned()
    }
    
    pub fn set(&self, key: String, value: String) -> bool {
        // Some keys are read-only
        match key.as_str() {
            "databases" | "port" => false, // Read-only
            _ => {
                self.settings.write().unwrap().insert(key, value);
                true
            }
        }
    }
    
    pub fn get_all(&self) -> Vec<(String, String)> {
        self.settings
            .read()
            .unwrap()
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect()
    }
}

impl Default for Config {
    fn default() -> Self {
        Self::new()
    }
}
