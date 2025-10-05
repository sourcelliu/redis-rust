// Database implementation

use super::types::RedisValue;
use dashmap::DashMap;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

/// Get current timestamp in milliseconds
pub fn current_timestamp_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64
}

/// A single database instance
pub struct DbInstance {
    /// Main key-value storage
    data: DashMap<String, RedisValue>,
    /// Expiration timestamps in milliseconds (key -> expiration_time_ms)
    expires: DashMap<String, u64>,
}

impl DbInstance {
    pub fn new() -> Self {
        Self {
            data: DashMap::new(),
            expires: DashMap::new(),
        }
    }

    /// Check if key is expired and remove it if so
    fn check_expired(&self, key: &str) -> bool {
        if let Some(expire_entry) = self.expires.get(key) {
            let expire_time = *expire_entry.value();
            if current_timestamp_ms() >= expire_time {
                // Key has expired, remove it
                drop(expire_entry); // Drop the reference before removal
                self.data.remove(key);
                self.expires.remove(key);
                return true;
            }
        }
        false
    }

    pub fn get(&self, key: &str) -> Option<RedisValue> {
        if self.check_expired(key) {
            return None;
        }
        self.data.get(key).map(|v| v.value().clone())
    }

    pub fn set(&self, key: String, value: RedisValue) {
        self.data.insert(key, value);
    }

    /// Set key with expiration time in milliseconds
    pub fn set_with_expiry(&self, key: String, value: RedisValue, expire_at_ms: u64) {
        self.data.insert(key.clone(), value);
        self.expires.insert(key, expire_at_ms);
    }

    /// Set expiration for an existing key (returns true if key exists)
    pub fn set_expiry(&self, key: &str, expire_at_ms: u64) -> bool {
        if self.data.contains_key(key) {
            self.expires.insert(key.to_string(), expire_at_ms);
            true
        } else {
            false
        }
    }

    /// Get TTL in milliseconds (returns -2 if key doesn't exist, -1 if no expiry)
    pub fn get_ttl_ms(&self, key: &str) -> i64 {
        if self.check_expired(key) {
            return -2;
        }

        if !self.data.contains_key(key) {
            return -2;
        }

        if let Some(expire_entry) = self.expires.get(key) {
            let expire_time = *expire_entry.value();
            let now = current_timestamp_ms();
            if now >= expire_time {
                return -2; // Already expired
            }
            (expire_time - now) as i64
        } else {
            -1 // No expiration set
        }
    }

    /// Remove expiration from key (returns true if expiration was removed)
    pub fn persist(&self, key: &str) -> bool {
        self.expires.remove(key).is_some()
    }

    pub fn delete(&self, key: &str) -> bool {
        self.expires.remove(key);
        self.data.remove(key).is_some()
    }

    pub fn exists(&self, key: &str) -> bool {
        if self.check_expired(key) {
            return false;
        }
        self.data.contains_key(key)
    }

    pub fn len(&self) -> usize {
        self.data.len()
    }

    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    pub fn clear(&self) {
        self.data.clear();
        self.expires.clear();
    }

    pub fn keys(&self, pattern: &str) -> Vec<String> {
        if pattern == "*" {
            // Return all keys (excluding expired ones)
            self.data
                .iter()
                .filter(|entry| !self.check_expired(entry.key()))
                .map(|entry| entry.key().clone())
                .collect()
        } else {
            // Simple pattern matching (only supports * wildcard)
            self.data
                .iter()
                .filter(|entry| {
                    !self.check_expired(entry.key()) && Self::match_pattern(entry.key(), pattern)
                })
                .map(|entry| entry.key().clone())
                .collect()
        }
    }

    fn match_pattern(key: &str, pattern: &str) -> bool {
        // Simple glob pattern matching
        // Only supports * (match any sequence) for now
        if pattern == "*" {
            return true;
        }

        if !pattern.contains('*') {
            return key == pattern;
        }

        let parts: Vec<&str> = pattern.split('*').collect();
        let mut key_pos = 0;

        for (i, part) in parts.iter().enumerate() {
            if part.is_empty() {
                continue;
            }

            if i == 0 {
                // First part must match start
                if !key.starts_with(part) {
                    return false;
                }
                key_pos = part.len();
            } else if i == parts.len() - 1 {
                // Last part must match end
                if !key.ends_with(part) {
                    return false;
                }
            } else {
                // Middle parts
                if let Some(pos) = key[key_pos..].find(part) {
                    key_pos += pos + part.len();
                } else {
                    return false;
                }
            }
        }

        true
    }
}

impl Default for DbInstance {
    fn default() -> Self {
        Self::new()
    }
}

/// Main database with multiple instances (typically 16)
pub struct Database {
    databases: Vec<Arc<DbInstance>>,
}

impl Database {
    pub fn new(num_dbs: usize) -> Self {
        let mut databases = Vec::with_capacity(num_dbs);
        for _ in 0..num_dbs {
            databases.push(Arc::new(DbInstance::new()));
        }
        Self { databases }
    }

    pub fn get_db(&self, index: usize) -> Option<&Arc<DbInstance>> {
        self.databases.get(index)
    }

    pub async fn flush_db(&self, index: usize) {
        if let Some(db) = self.get_db(index) {
            db.clear();
        }
    }

    pub async fn flush_all(&self) {
        for db in &self.databases {
            db.clear();
        }
    }

    pub async fn db_size(&self, index: usize) -> usize {
        self.get_db(index).map_or(0, |db| db.len())
    }

    pub async fn keys(&self, index: usize, pattern: &str) -> Vec<String> {
        self.get_db(index)
            .map_or(vec![], |db| db.keys(pattern))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bytes::Bytes;

    #[test]
    fn test_db_instance() {
        let db = DbInstance::new();

        // Test set and get
        db.set(
            "key1".to_string(),
            RedisValue::String(Bytes::from("value1")),
        );
        assert!(db.exists("key1"));

        let value = db.get("key1").unwrap();
        assert_eq!(value.as_string().unwrap(), &Bytes::from("value1"));

        // Test delete
        assert!(db.delete("key1"));
        assert!(!db.exists("key1"));
        assert!(!db.delete("key1")); // Already deleted
    }

    #[test]
    fn test_pattern_matching() {
        assert!(DbInstance::match_pattern("hello", "*"));
        assert!(DbInstance::match_pattern("hello", "hello"));
        assert!(DbInstance::match_pattern("hello", "hel*"));
        assert!(DbInstance::match_pattern("hello", "*llo"));
        assert!(DbInstance::match_pattern("hello", "h*o"));
        assert!(!DbInstance::match_pattern("hello", "hi*"));
    }

    #[test]
    fn test_keys_pattern() {
        let db = DbInstance::new();
        db.set("user:1".to_string(), RedisValue::String(Bytes::from("a")));
        db.set("user:2".to_string(), RedisValue::String(Bytes::from("b")));
        db.set("post:1".to_string(), RedisValue::String(Bytes::from("c")));

        let keys = db.keys("user:*");
        assert_eq!(keys.len(), 2);
        assert!(keys.contains(&"user:1".to_string()));
        assert!(keys.contains(&"user:2".to_string()));

        let all_keys = db.keys("*");
        assert_eq!(all_keys.len(), 3);
    }

    #[tokio::test]
    async fn test_database() {
        let db = Database::new(16);

        // Test db selection
        let db0 = db.get_db(0).unwrap();
        db0.set("key".to_string(), RedisValue::String(Bytes::from("value")));

        assert_eq!(db.db_size(0).await, 1);
        assert_eq!(db.db_size(1).await, 0);

        // Test flush
        db.flush_db(0).await;
        assert_eq!(db.db_size(0).await, 0);
    }
}
