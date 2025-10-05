// Synchronization protocol handler (SYNC and PSYNC)

use crate::protocol::RespValue;
use crate::replication::backlog::ReplicationBacklog;
use crate::storage::db::Database;
use std::sync::Arc;

/// Replication offset type
pub type ReplicationOffset = u64;

/// Sync handler for master-replica synchronization
pub struct SyncHandler {
    backlog: Arc<ReplicationBacklog>,
}

impl SyncHandler {
    /// Create a new sync handler
    pub fn new(backlog: Arc<ReplicationBacklog>) -> Self {
        Self { backlog }
    }

    /// Handle PSYNC command from replica
    /// Returns: (needs_full_sync, offset, replication_id)
    pub fn handle_psync(
        &self,
        replica_repl_id: Option<String>,
        replica_offset: i64,
        master_repl_id: &str,
    ) -> (bool, u64, String) {
        // Check if replica has a replication ID
        match replica_repl_id {
            None => {
                // First time sync - need full sync
                (true, 0, master_repl_id.to_string())
            }
            Some(repl_id) => {
                // Check if replication ID matches
                if repl_id != master_repl_id {
                    // Replication ID mismatch - need full sync
                    (true, 0, master_repl_id.to_string())
                }
                // Check if we can do partial resync
                else if replica_offset < 0 {
                    // Invalid offset - need full sync
                    (true, 0, master_repl_id.to_string())
                } else {
                    let offset = replica_offset as u64;

                    // Check if offset is in backlog
                    if self.backlog.get_from_offset(offset).is_some() {
                        // Can do partial resync
                        (false, offset, master_repl_id.to_string())
                    } else {
                        // Offset too old - need full sync
                        (true, 0, master_repl_id.to_string())
                    }
                }
            }
        }
    }

    /// Generate PSYNC response for partial resync
    pub fn generate_continue_response(offset: u64, repl_id: &str) -> RespValue {
        RespValue::SimpleString(format!("CONTINUE {}", repl_id))
    }

    /// Generate PSYNC response for full resync
    pub fn generate_fullresync_response(repl_id: &str) -> RespValue {
        RespValue::SimpleString(format!("FULLRESYNC {} 0", repl_id))
    }

    /// Get commands from backlog for partial resync
    pub fn get_partial_sync_data(&self, offset: u64) -> Option<Vec<Vec<u8>>> {
        self.backlog.get_from_offset(offset)
    }
}

/// Parse PSYNC command arguments
pub fn parse_psync_args(args: &[Vec<u8>]) -> anyhow::Result<(Option<String>, i64)> {
    if args.len() != 2 {
        return Err(anyhow::anyhow!("ERR wrong number of arguments for PSYNC"));
    }

    // Parse replication ID
    let repl_id = std::str::from_utf8(&args[0])?;
    let repl_id = if repl_id == "?" {
        None
    } else {
        Some(repl_id.to_string())
    };

    // Parse offset
    let offset_str = std::str::from_utf8(&args[1])?;
    let offset: i64 = offset_str.parse()?;

    Ok((repl_id, offset))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_psync_args() {
        // First sync
        let args = vec![b"?".to_vec(), b"-1".to_vec()];
        let (repl_id, offset) = parse_psync_args(&args).unwrap();
        assert_eq!(repl_id, None);
        assert_eq!(offset, -1);

        // Partial resync
        let args = vec![
            b"8371b4fb1155b71f4a04d3e1bc3e18c4a990aeeb".to_vec(),
            b"1000".to_vec(),
        ];
        let (repl_id, offset) = parse_psync_args(&args).unwrap();
        assert_eq!(
            repl_id,
            Some("8371b4fb1155b71f4a04d3e1bc3e18c4a990aeeb".to_string())
        );
        assert_eq!(offset, 1000);
    }

    #[test]
    fn test_handle_psync_first_sync() {
        let backlog = Arc::new(ReplicationBacklog::new());
        let handler = SyncHandler::new(backlog);

        let (needs_full, offset, _) =
            handler.handle_psync(None, -1, "test-repl-id");

        assert!(needs_full);
        assert_eq!(offset, 0);
    }

    #[test]
    fn test_handle_psync_repl_id_mismatch() {
        let backlog = Arc::new(ReplicationBacklog::new());
        let handler = SyncHandler::new(backlog);

        let (needs_full, offset, _) = handler.handle_psync(
            Some("old-repl-id".to_string()),
            100,
            "new-repl-id",
        );

        assert!(needs_full);
        assert_eq!(offset, 0);
    }

    #[test]
    fn test_handle_psync_partial_sync() {
        let backlog = Arc::new(ReplicationBacklog::new());

        // Add some data to backlog
        backlog.add(0, b"SET key1 val1".to_vec());
        backlog.add(14, b"SET key2 val2".to_vec());

        let handler = SyncHandler::new(backlog);

        let (needs_full, offset, _) = handler.handle_psync(
            Some("test-repl-id".to_string()),
            0,
            "test-repl-id",
        );

        assert!(!needs_full);
        assert_eq!(offset, 0);
    }

    #[test]
    fn test_handle_psync_offset_too_old() {
        let backlog = Arc::new(ReplicationBacklog::with_size(50));

        // Add data that will evict old entries
        backlog.add(0, b"12345678901234567890".to_vec());
        backlog.add(20, b"12345678901234567890".to_vec());
        backlog.add(40, b"12345678901234567890".to_vec()); // Evicts first

        let handler = SyncHandler::new(backlog);

        // Try to sync from offset 0 (too old)
        let (needs_full, offset, _) = handler.handle_psync(
            Some("test-repl-id".to_string()),
            0,
            "test-repl-id",
        );

        assert!(needs_full);
        assert_eq!(offset, 0);
    }
}
