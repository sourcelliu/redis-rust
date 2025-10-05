// Script cache for storing compiled Lua scripts

use dashmap::DashMap;
use sha1::{Digest, Sha1};
use std::sync::Arc;

/// Compute SHA1 hash of script content
pub fn compute_sha1(script: &str) -> String {
    let mut hasher = Sha1::new();
    hasher.update(script.as_bytes());
    format!("{:x}", hasher.finalize())
}

/// Cache for compiled Lua scripts
/// Maps SHA1 hash -> script source code
pub struct ScriptCache {
    scripts: Arc<DashMap<String, String>>,
}

impl ScriptCache {
    /// Create a new script cache
    pub fn new() -> Self {
        Self {
            scripts: Arc::new(DashMap::new()),
        }
    }

    /// Load a script into the cache and return its SHA1 hash
    pub fn load(&self, script: String) -> String {
        let sha1 = compute_sha1(&script);
        self.scripts.insert(sha1.clone(), script);
        sha1
    }

    /// Get a script by its SHA1 hash
    pub fn get(&self, sha1: &str) -> Option<String> {
        self.scripts.get(sha1).map(|entry| entry.value().clone())
    }

    /// Check if a script exists in the cache
    pub fn exists(&self, sha1: &str) -> bool {
        self.scripts.contains_key(sha1)
    }

    /// Check existence of multiple scripts
    pub fn exists_multi(&self, sha1s: &[String]) -> Vec<bool> {
        sha1s.iter().map(|sha1| self.exists(sha1)).collect()
    }

    /// Flush all scripts from the cache
    pub fn flush(&self) {
        self.scripts.clear();
    }

    /// Get the number of cached scripts
    pub fn len(&self) -> usize {
        self.scripts.len()
    }

    /// Check if the cache is empty
    pub fn is_empty(&self) -> bool {
        self.scripts.is_empty()
    }
}

impl Default for ScriptCache {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sha1_computation() {
        let script = "return 'hello'";
        let sha1 = compute_sha1(script);
        assert_eq!(sha1.len(), 40); // SHA1 is 40 hex characters
    }

    #[test]
    fn test_script_cache() {
        let cache = ScriptCache::new();
        let script = "return KEYS[1]".to_string();

        // Load script
        let sha1 = cache.load(script.clone());
        assert_eq!(sha1.len(), 40);

        // Check existence
        assert!(cache.exists(&sha1));

        // Get script
        let retrieved = cache.get(&sha1);
        assert_eq!(retrieved, Some(script));

        // Flush
        cache.flush();
        assert!(!cache.exists(&sha1));
    }

    #[test]
    fn test_exists_multi() {
        let cache = ScriptCache::new();

        let script1 = "return 1".to_string();
        let script2 = "return 2".to_string();

        let sha1_1 = cache.load(script1);
        let sha1_2 = cache.load(script2);
        let sha1_3 = "nonexistent".to_string();

        let results = cache.exists_multi(&[sha1_1, sha1_2, sha1_3]);
        assert_eq!(results, vec![true, true, false]);
    }
}
