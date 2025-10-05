// Replication backlog for partial resynchronization

use std::collections::VecDeque;
use std::sync::{Arc, RwLock};

/// Replication backlog stores recent commands for partial resync
pub struct ReplicationBacklog {
    /// Circular buffer of commands
    buffer: Arc<RwLock<VecDeque<BacklogEntry>>>,
    /// Maximum size in bytes
    max_size: usize,
    /// Current size in bytes
    current_size: Arc<RwLock<usize>>,
    /// First offset in the backlog
    first_offset: Arc<RwLock<u64>>,
}

/// Entry in the replication backlog
#[derive(Clone)]
pub struct BacklogEntry {
    /// Replication offset when this command was written
    pub offset: u64,
    /// Command data (RESP-encoded)
    pub data: Vec<u8>,
}

impl ReplicationBacklog {
    /// Create a new replication backlog
    /// Default size: 1MB
    pub fn new() -> Self {
        Self::with_size(1024 * 1024)
    }

    /// Create a backlog with specific size
    pub fn with_size(max_size: usize) -> Self {
        Self {
            buffer: Arc::new(RwLock::new(VecDeque::new())),
            max_size,
            current_size: Arc::new(RwLock::new(0)),
            first_offset: Arc::new(RwLock::new(0)),
        }
    }

    /// Add a command to the backlog
    pub fn add(&self, offset: u64, data: Vec<u8>) {
        let data_len = data.len();
        let entry = BacklogEntry {
            offset,
            data,
        };

        let mut buffer = self.buffer.write().unwrap();
        let mut current_size = self.current_size.write().unwrap();

        // Add new entry
        buffer.push_back(entry);
        *current_size += data_len;

        // Evict old entries if over size limit
        while *current_size > self.max_size && !buffer.is_empty() {
            if let Some(old_entry) = buffer.pop_front() {
                *current_size -= old_entry.data.len();

                // Update first offset
                if let Some(next_entry) = buffer.front() {
                    let mut first_offset = self.first_offset.write().unwrap();
                    *first_offset = next_entry.offset;
                }
            }
        }
    }

    /// Get commands starting from a specific offset
    /// Returns None if the offset is too old (not in backlog)
    pub fn get_from_offset(&self, offset: u64) -> Option<Vec<Vec<u8>>> {
        let buffer = self.buffer.read().unwrap();
        let first_offset = *self.first_offset.read().unwrap();

        // Check if offset is in range
        if offset < first_offset {
            return None; // Offset too old, need full sync
        }

        // Collect all commands from offset
        let mut result = Vec::new();
        for entry in buffer.iter() {
            if entry.offset >= offset {
                result.push(entry.data.clone());
            }
        }

        Some(result)
    }

    /// Get the first offset in the backlog
    pub fn first_offset(&self) -> u64 {
        *self.first_offset.read().unwrap()
    }

    /// Get the current size of the backlog
    pub fn size(&self) -> usize {
        *self.current_size.read().unwrap()
    }

    /// Get the number of entries in the backlog
    pub fn len(&self) -> usize {
        self.buffer.read().unwrap().len()
    }

    /// Check if the backlog is empty
    pub fn is_empty(&self) -> bool {
        self.buffer.read().unwrap().is_empty()
    }

    /// Clear the backlog
    pub fn clear(&self) {
        let mut buffer = self.buffer.write().unwrap();
        let mut current_size = self.current_size.write().unwrap();
        let mut first_offset = self.first_offset.write().unwrap();

        buffer.clear();
        *current_size = 0;
        *first_offset = 0;
    }
}

impl Default for ReplicationBacklog {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_backlog_basic() {
        let backlog = ReplicationBacklog::new();

        backlog.add(0, b"SET key1 value1".to_vec());
        backlog.add(15, b"SET key2 value2".to_vec());

        assert_eq!(backlog.len(), 2);
        assert!(!backlog.is_empty());
    }

    #[test]
    fn test_backlog_get_from_offset() {
        let backlog = ReplicationBacklog::new();

        backlog.add(0, b"SET key1 value1".to_vec());
        backlog.add(15, b"SET key2 value2".to_vec());
        backlog.add(30, b"SET key3 value3".to_vec());

        // Get from offset 15 should return 2 commands
        let cmds = backlog.get_from_offset(15).unwrap();
        assert_eq!(cmds.len(), 2);

        // Get from offset 0 should return all commands
        let cmds = backlog.get_from_offset(0).unwrap();
        assert_eq!(cmds.len(), 3);

        // Get from offset 30 should return 1 command
        let cmds = backlog.get_from_offset(30).unwrap();
        assert_eq!(cmds.len(), 1);
    }

    #[test]
    fn test_backlog_eviction() {
        // Create small backlog (50 bytes)
        let backlog = ReplicationBacklog::with_size(50);

        backlog.add(0, b"12345678901234567890".to_vec());   // 20 bytes
        backlog.add(20, b"12345678901234567890".to_vec());  // 20 bytes
        backlog.add(40, b"12345678901234567890".to_vec());  // 20 bytes (should evict first)

        // First entry should be evicted
        assert!(backlog.len() <= 2);
        assert!(backlog.size() <= 50);

        // Offset 0 should be too old
        assert!(backlog.get_from_offset(0).is_none());

        // Offset 20 should still be available
        assert!(backlog.get_from_offset(20).is_some());
    }

    #[test]
    fn test_backlog_clear() {
        let backlog = ReplicationBacklog::new();

        backlog.add(0, b"SET key1 value1".to_vec());
        backlog.add(15, b"SET key2 value2".to_vec());

        assert_eq!(backlog.len(), 2);

        backlog.clear();

        assert_eq!(backlog.len(), 0);
        assert!(backlog.is_empty());
        assert_eq!(backlog.first_offset(), 0);
    }
}
