// Transaction support for Redis commands
// Implements MULTI/EXEC/DISCARD/WATCH for atomic command execution

use crate::protocol::RespValue;
use crate::storage::db::Database;
use dashmap::DashMap;
use std::sync::Arc;

/// Transaction state for a connection
#[derive(Debug, Clone)]
pub struct Transaction {
    /// Queued commands waiting for EXEC
    pub commands: Vec<Vec<Vec<u8>>>,
    /// Keys being watched for optimistic locking
    pub watched_keys: Vec<String>,
    /// Whether we're in MULTI mode
    pub in_multi: bool,
}

impl Transaction {
    pub fn new() -> Self {
        Self {
            commands: Vec::new(),
            watched_keys: Vec::new(),
            in_multi: false,
        }
    }

    /// Start a transaction
    pub fn start_multi(&mut self) {
        self.in_multi = true;
        self.commands.clear();
    }

    /// Queue a command for execution
    pub fn queue_command(&mut self, args: Vec<Vec<u8>>) {
        self.commands.push(args);
    }

    /// Discard the transaction
    pub fn discard(&mut self) {
        self.in_multi = false;
        self.commands.clear();
    }

    /// Execute all queued commands
    pub fn exec(&mut self) -> Vec<Vec<Vec<u8>>> {
        self.in_multi = false;
        std::mem::take(&mut self.commands)
    }

    /// Add a key to watch list
    pub fn watch_key(&mut self, key: String) {
        if !self.watched_keys.contains(&key) {
            self.watched_keys.push(key);
        }
    }

    /// Clear all watched keys
    pub fn unwatch(&mut self) {
        self.watched_keys.clear();
    }

    /// Check if any watched keys were modified
    pub fn check_watched_keys(&self, modified_keys: &WatchedKeysRegistry) -> bool {
        for key in &self.watched_keys {
            if modified_keys.was_modified(key) {
                return true;
            }
        }
        false
    }
}

impl Default for Transaction {
    fn default() -> Self {
        Self::new()
    }
}

/// Global registry to track which keys have been modified
/// Used for WATCH command to detect changes
pub struct WatchedKeysRegistry {
    /// Maps key -> version (incremented on each modification)
    versions: DashMap<String, u64>,
}

impl WatchedKeysRegistry {
    pub fn new() -> Self {
        Self {
            versions: DashMap::new(),
        }
    }

    /// Mark a key as modified
    pub fn mark_modified(&self, key: &str) {
        self.versions
            .entry(key.to_string())
            .and_modify(|v| *v += 1)
            .or_insert(1);
    }

    /// Check if a key was modified (version changed)
    pub fn was_modified(&self, key: &str) -> bool {
        self.versions.get(key).is_some()
    }

    /// Get current version of a key
    pub fn get_version(&self, key: &str) -> u64 {
        self.versions.get(key).map(|v| *v).unwrap_or(0)
    }

    /// Clear all tracked versions (for testing)
    #[allow(dead_code)]
    pub fn clear(&self) {
        self.versions.clear();
    }
}

impl Default for WatchedKeysRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// MULTI command - Start a transaction
pub async fn multi(tx: &mut Transaction) -> RespValue {
    if tx.in_multi {
        return RespValue::Error("ERR MULTI calls can not be nested".to_string());
    }
    tx.start_multi();
    RespValue::SimpleString("OK".to_string())
}

/// EXEC command - Execute all queued commands
pub async fn exec(
    tx: &mut Transaction,
    db: &Arc<Database>,
    db_index: usize,
    registry: &Arc<WatchedKeysRegistry>,
    executor: impl Fn(Vec<Vec<u8>>) -> std::pin::Pin<Box<dyn std::future::Future<Output = RespValue> + Send>>,
) -> RespValue {
    if !tx.in_multi {
        return RespValue::Error("ERR EXEC without MULTI".to_string());
    }

    // Check if any watched keys were modified
    if tx.check_watched_keys(registry) {
        tx.discard();
        tx.unwatch();
        return RespValue::BulkString(None); // Transaction aborted
    }

    // Execute all queued commands
    let commands = tx.exec();
    let mut results = Vec::new();

    for cmd_args in commands {
        let result = executor(cmd_args).await;
        results.push(result);
    }

    tx.unwatch();
    RespValue::Array(Some(results))
}

/// DISCARD command - Abort transaction
pub async fn discard(tx: &mut Transaction) -> RespValue {
    if !tx.in_multi {
        return RespValue::Error("ERR DISCARD without MULTI".to_string());
    }
    tx.discard();
    tx.unwatch();
    RespValue::SimpleString("OK".to_string())
}

/// WATCH command - Watch keys for changes
pub async fn watch(
    tx: &mut Transaction,
    db: &Arc<Database>,
    db_index: usize,
    args: Vec<Vec<u8>>,
) -> RespValue {
    if args.is_empty() {
        return RespValue::Error("ERR wrong number of arguments for 'watch' command".to_string());
    }

    if tx.in_multi {
        return RespValue::Error("ERR WATCH inside MULTI is not allowed".to_string());
    }

    for key_bytes in args {
        let key = match std::str::from_utf8(&key_bytes) {
            Ok(s) => s.to_string(),
            Err(_) => return RespValue::Error("ERR invalid key".to_string()),
        };
        tx.watch_key(key);
    }

    RespValue::SimpleString("OK".to_string())
}

/// UNWATCH command - Clear all watched keys
pub async fn unwatch(tx: &mut Transaction) -> RespValue {
    tx.unwatch();
    RespValue::SimpleString("OK".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_transaction_lifecycle() {
        let mut tx = Transaction::new();

        assert!(!tx.in_multi);

        // Start transaction
        tx.start_multi();
        assert!(tx.in_multi);

        // Queue commands
        tx.queue_command(vec![b"SET".to_vec(), b"key1".to_vec(), b"value1".to_vec()]);
        tx.queue_command(vec![b"GET".to_vec(), b"key1".to_vec()]);
        assert_eq!(tx.commands.len(), 2);

        // Execute
        let commands = tx.exec();
        assert_eq!(commands.len(), 2);
        assert!(!tx.in_multi);
    }

    #[test]
    fn test_transaction_discard() {
        let mut tx = Transaction::new();

        tx.start_multi();
        tx.queue_command(vec![b"SET".to_vec(), b"key1".to_vec(), b"value1".to_vec()]);
        assert_eq!(tx.commands.len(), 1);

        tx.discard();
        assert!(!tx.in_multi);
        assert_eq!(tx.commands.len(), 0);
    }

    #[test]
    fn test_watch_keys() {
        let mut tx = Transaction::new();

        tx.watch_key("key1".to_string());
        tx.watch_key("key2".to_string());
        tx.watch_key("key1".to_string()); // Duplicate

        assert_eq!(tx.watched_keys.len(), 2);

        tx.unwatch();
        assert_eq!(tx.watched_keys.len(), 0);
    }

    #[test]
    fn test_watched_keys_registry() {
        let registry = WatchedKeysRegistry::new();

        assert!(!registry.was_modified("key1"));
        assert_eq!(registry.get_version("key1"), 0);

        registry.mark_modified("key1");
        assert!(registry.was_modified("key1"));
        assert_eq!(registry.get_version("key1"), 1);

        registry.mark_modified("key1");
        assert_eq!(registry.get_version("key1"), 2);
    }

    #[tokio::test]
    async fn test_multi_command() {
        let mut tx = Transaction::new();

        let result = multi(&mut tx).await;
        assert_eq!(result, RespValue::SimpleString("OK".to_string()));
        assert!(tx.in_multi);

        // Nested MULTI should fail
        let result = multi(&mut tx).await;
        assert!(matches!(result, RespValue::Error(_)));
    }

    #[tokio::test]
    async fn test_discard_command() {
        let mut tx = Transaction::new();

        // DISCARD without MULTI should fail
        let result = discard(&mut tx).await;
        assert!(matches!(result, RespValue::Error(_)));

        // Start transaction and discard
        tx.start_multi();
        let result = discard(&mut tx).await;
        assert_eq!(result, RespValue::SimpleString("OK".to_string()));
        assert!(!tx.in_multi);
    }
}
