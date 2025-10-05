// Slow query log tracking

use dashmap::DashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// Global slow log entry ID counter
static SLOWLOG_ID_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Single slow query log entry
#[derive(Debug, Clone)]
pub struct SlowLogEntry {
    /// Unique entry ID
    pub id: u64,
    /// Unix timestamp when command started
    pub timestamp: u64,
    /// Execution time in microseconds
    pub duration_micros: u64,
    /// Command and arguments
    pub command: Vec<String>,
    /// Client address
    pub client_addr: String,
    /// Client name (if set)
    pub client_name: Option<String>,
}

impl SlowLogEntry {
    /// Create a new slow log entry
    pub fn new(
        duration_micros: u64,
        command: Vec<String>,
        client_addr: String,
        client_name: Option<String>,
    ) -> Self {
        let id = SLOWLOG_ID_COUNTER.fetch_add(1, Ordering::SeqCst);
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        Self {
            id,
            timestamp,
            duration_micros,
            command,
            client_addr,
            client_name,
        }
    }
}

/// Slow query log manager
#[derive(Clone)]
pub struct SlowLog {
    /// Slow log entries (ordered by ID)
    entries: Arc<DashMap<u64, SlowLogEntry>>,
    /// Maximum number of entries to keep
    max_len: usize,
    /// Minimum execution time in microseconds to log
    threshold_micros: u64,
    /// Next entry ID to evict (for circular buffer)
    next_evict_id: Arc<AtomicU64>,
}

impl SlowLog {
    /// Create a new slow log with default settings
    pub fn new() -> Self {
        Self::with_config(128, 10000) // 128 entries, 10ms threshold
    }

    /// Create a slow log with custom configuration
    pub fn with_config(max_len: usize, threshold_micros: u64) -> Self {
        Self {
            entries: Arc::new(DashMap::new()),
            max_len,
            threshold_micros,
            next_evict_id: Arc::new(AtomicU64::new(0)),
        }
    }

    /// Add an entry to the slow log if it exceeds the threshold
    pub fn add_if_slow(
        &self,
        duration: Duration,
        command: Vec<String>,
        client_addr: String,
        client_name: Option<String>,
    ) {
        let duration_micros = duration.as_micros() as u64;

        // Only log if it exceeds threshold
        if duration_micros < self.threshold_micros {
            return;
        }

        let entry = SlowLogEntry::new(duration_micros, command, client_addr, client_name);
        let entry_id = entry.id;

        // Add entry
        self.entries.insert(entry_id, entry);

        // Evict oldest if we exceed max_len
        if self.entries.len() > self.max_len {
            let evict_id = self.next_evict_id.fetch_add(1, Ordering::SeqCst);
            self.entries.remove(&evict_id);
        }
    }

    /// Get the N most recent slow log entries
    pub fn get(&self, count: usize) -> Vec<SlowLogEntry> {
        let mut entries: Vec<_> = self
            .entries
            .iter()
            .map(|entry| entry.value().clone())
            .collect();

        // Sort by ID descending (most recent first)
        entries.sort_by(|a, b| b.id.cmp(&a.id));

        // Take only requested count
        entries.truncate(count);
        entries
    }

    /// Get all slow log entries
    pub fn get_all(&self) -> Vec<SlowLogEntry> {
        let mut entries: Vec<_> = self
            .entries
            .iter()
            .map(|entry| entry.value().clone())
            .collect();

        // Sort by ID descending (most recent first)
        entries.sort_by(|a, b| b.id.cmp(&a.id));
        entries
    }

    /// Get the number of entries in the slow log
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Check if slow log is empty
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Clear all entries
    pub fn reset(&self) {
        self.entries.clear();
        self.next_evict_id.store(0, Ordering::SeqCst);
    }

    /// Get the current threshold in microseconds
    pub fn threshold_micros(&self) -> u64 {
        self.threshold_micros
    }

    /// Set the threshold in microseconds
    pub fn set_threshold_micros(&mut self, threshold: u64) {
        self.threshold_micros = threshold;
    }

    /// Get the maximum number of entries
    pub fn max_len(&self) -> usize {
        self.max_len
    }

    /// Set the maximum number of entries
    pub fn set_max_len(&mut self, max_len: usize) {
        self.max_len = max_len;
    }
}

impl Default for SlowLog {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_slowlog_entry_creation() {
        let entry = SlowLogEntry::new(
            50000,
            vec!["GET".to_string(), "mykey".to_string()],
            "127.0.0.1:54321".to_string(),
            Some("myclient".to_string()),
        );

        assert_eq!(entry.duration_micros, 50000);
        assert_eq!(entry.command, vec!["GET", "mykey"]);
        assert_eq!(entry.client_addr, "127.0.0.1:54321");
        assert_eq!(entry.client_name, Some("myclient".to_string()));
        assert!(entry.id > 0);
    }

    #[test]
    fn test_slowlog_threshold() {
        let slowlog = SlowLog::with_config(10, 10000); // 10ms threshold

        // Add a slow query (20ms)
        slowlog.add_if_slow(
            Duration::from_micros(20000),
            vec!["GET".to_string(), "key1".to_string()],
            "127.0.0.1:1111".to_string(),
            None,
        );

        // Add a fast query (5ms) - should not be logged
        slowlog.add_if_slow(
            Duration::from_micros(5000),
            vec!["GET".to_string(), "key2".to_string()],
            "127.0.0.1:2222".to_string(),
            None,
        );

        assert_eq!(slowlog.len(), 1);
        let entries = slowlog.get_all();
        assert_eq!(entries[0].command, vec!["GET", "key1"]);
    }

    #[test]
    fn test_slowlog_max_len() {
        let slowlog = SlowLog::with_config(3, 1000);

        // Add 5 slow queries
        for i in 0..5 {
            slowlog.add_if_slow(
                Duration::from_micros(10000),
                vec!["GET".to_string(), format!("key{}", i)],
                "127.0.0.1:1111".to_string(),
                None,
            );
        }

        // Should only keep 3 most recent
        assert_eq!(slowlog.len(), 3);
    }

    #[test]
    fn test_slowlog_get() {
        let slowlog = SlowLog::with_config(10, 1000);

        // Add 5 entries
        for i in 0..5 {
            slowlog.add_if_slow(
                Duration::from_micros(10000),
                vec!["GET".to_string(), format!("key{}", i)],
                "127.0.0.1:1111".to_string(),
                None,
            );
        }

        // Get 3 most recent
        let entries = slowlog.get(3);
        assert_eq!(entries.len(), 3);

        // Verify they are in descending order (most recent first)
        assert!(entries[0].id > entries[1].id);
        assert!(entries[1].id > entries[2].id);
    }

    #[test]
    fn test_slowlog_reset() {
        let slowlog = SlowLog::with_config(10, 1000);

        slowlog.add_if_slow(
            Duration::from_micros(10000),
            vec!["GET".to_string(), "key1".to_string()],
            "127.0.0.1:1111".to_string(),
            None,
        );

        assert_eq!(slowlog.len(), 1);

        slowlog.reset();

        assert_eq!(slowlog.len(), 0);
        assert!(slowlog.is_empty());
    }
}
