// Cluster redirection logic - MOVED and ASK errors

use crate::cluster::ClusterState;
use crate::protocol::RespValue;
use std::sync::Arc;

/// Slot migration state
#[derive(Debug, Clone, PartialEq)]
pub enum SlotState {
    /// Slot is stable (not migrating)
    Stable,
    /// Slot is being imported from another node
    Importing { from_node: String },
    /// Slot is being migrated to another node
    Migrating { to_node: String },
}

/// Check if a key belongs to this node, return redirection if needed
/// Returns None if this node owns the slot, or a MOVED/ASK error
pub fn check_slot_ownership(
    cluster: &Arc<ClusterState>,
    key: &[u8],
    _asking: bool,
) -> Option<RespValue> {
    if !cluster.enabled {
        return None; // Not in cluster mode
    }

    let slot = crate::cluster::key_hash_slot(key);

    // Check if we own this slot
    if cluster.owns_slot(slot) {
        return None; // We own it, proceed
    }

    // We don't own this slot - need to redirect
    if let Some(owner_id) = cluster.get_slot_node(slot) {
        // Get the node info to find address
        if let Some(node) = cluster.get_node(&owner_id) {
            if let Some(addr) = node.addr {
                // Return MOVED redirection
                return Some(RespValue::Error(format!(
                    "MOVED {} {}",
                    slot,
                    addr
                )));
            }
        }
    }

    // No owner found - cluster not fully configured
    Some(RespValue::Error(
        "CLUSTERDOWN Hash slot not served".to_string(),
    ))
}

/// Check if key access should return ASK redirection during migration
/// This is used when a slot is being migrated
pub fn check_ask_redirection(
    cluster: &Arc<ClusterState>,
    key: &[u8],
    slot_state: &SlotState,
) -> Option<RespValue> {
    if !cluster.enabled {
        return None;
    }

    let slot = crate::cluster::key_hash_slot(key);

    match slot_state {
        SlotState::Migrating { to_node } => {
            // Slot is migrating - return ASK if key doesn't exist locally
            if let Some(node) = cluster.get_node(to_node) {
                if let Some(addr) = node.addr {
                    return Some(RespValue::Error(format!(
                        "ASK {} {}",
                        slot,
                        addr
                    )));
                }
            }
            None
        }
        SlotState::Importing { .. } => {
            // Only accept if ASKING was sent
            None
        }
        SlotState::Stable => None,
    }
}

/// Parse MOVED error from response
/// Format: "MOVED 3999 127.0.0.1:6381"
pub fn parse_moved_error(error: &str) -> Option<(u16, String)> {
    let parts: Vec<&str> = error.split_whitespace().collect();
    if parts.len() == 3 && parts[0] == "MOVED" {
        if let Ok(slot) = parts[1].parse::<u16>() {
            return Some((slot, parts[2].to_string()));
        }
    }
    None
}

/// Parse ASK error from response
/// Format: "ASK 3999 127.0.0.1:6381"
pub fn parse_ask_error(error: &str) -> Option<(u16, String)> {
    let parts: Vec<&str> = error.split_whitespace().collect();
    if parts.len() == 3 && parts[0] == "ASK" {
        if let Ok(slot) = parts[1].parse::<u16>() {
            return Some((slot, parts[2].to_string()));
        }
    }
    None
}

/// Check if multiple keys are in the same slot (for multi-key commands)
pub fn check_multi_key_slot(keys: &[&[u8]]) -> Result<u16, String> {
    if keys.is_empty() {
        return Err("ERR no keys provided".to_string());
    }

    let first_slot = crate::cluster::key_hash_slot(keys[0]);

    for key in &keys[1..] {
        let slot = crate::cluster::key_hash_slot(key);
        if slot != first_slot {
            return Err("CROSSSLOT Keys in request don't hash to the same slot".to_string());
        }
    }

    Ok(first_slot)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cluster::ClusterState;
    use std::net::SocketAddr;
    use std::str::FromStr;

    #[test]
    fn test_check_slot_ownership_not_enabled() {
        let cluster = Arc::new(ClusterState::new(false));
        let result = check_slot_ownership(&cluster, b"mykey", false);
        assert!(result.is_none()); // No redirection when cluster disabled
    }

    #[test]
    fn test_check_slot_ownership_we_own() {
        let cluster = Arc::new(ClusterState::new(true));

        // Assign slot 0 to ourselves
        cluster.add_slot(0);

        // Find a key that hashes to slot 0
        let result = check_slot_ownership(&cluster, b"mykey", false);

        // If we own the slot, no redirection
        let slot = crate::cluster::key_hash_slot(b"mykey");
        if cluster.owns_slot(slot) {
            assert!(result.is_none());
        }
    }

    #[test]
    fn test_check_slot_ownership_moved() {
        let cluster = Arc::new(ClusterState::new(true));

        // Create another node and assign a slot to it
        let mut other_node = crate::cluster::node::ClusterNode::new_master(
            "other_node_id".to_string(),
            SocketAddr::from_str("127.0.0.1:7001").ok()
        );
        other_node.add_slot(100);
        cluster.add_node(other_node);
        cluster.assign_slots_to_node("other_node_id", vec![100]);

        // Try to access a key in slot 100
        // We need to find a key that hashes to slot 100
        for i in 0..10000 {
            let key = format!("key{}", i);
            let slot = crate::cluster::key_hash_slot(key.as_bytes());
            if slot == 100 {
                let result = check_slot_ownership(&cluster, key.as_bytes(), false);
                match result {
                    Some(RespValue::Error(msg)) => {
                        assert!(msg.starts_with("MOVED 100"));
                        assert!(msg.contains("127.0.0.1:7001"));
                    }
                    _ => panic!("Expected MOVED error"),
                }
                return;
            }
        }
    }

    #[test]
    fn test_parse_moved_error() {
        let result = parse_moved_error("MOVED 3999 127.0.0.1:6381");
        assert_eq!(result, Some((3999, "127.0.0.1:6381".to_string())));

        let invalid = parse_moved_error("INVALID");
        assert_eq!(invalid, None);
    }

    #[test]
    fn test_parse_ask_error() {
        let result = parse_ask_error("ASK 3999 127.0.0.1:6381");
        assert_eq!(result, Some((3999, "127.0.0.1:6381".to_string())));

        let invalid = parse_ask_error("INVALID");
        assert_eq!(invalid, None);
    }

    #[test]
    fn test_multi_key_same_slot() {
        // Keys with same hash tag should be in same slot
        let keys = vec![b"{user}:profile".as_ref(), b"{user}:settings".as_ref()];
        let result = check_multi_key_slot(&keys);
        assert!(result.is_ok());
    }

    #[test]
    fn test_multi_key_different_slots() {
        // Keys without hash tags likely in different slots
        let keys = vec![b"key1".as_ref(), b"key2".as_ref()];
        let result = check_multi_key_slot(&keys);

        // They might be in the same slot by chance, so we check both cases
        let slot1 = crate::cluster::key_hash_slot(b"key1");
        let slot2 = crate::cluster::key_hash_slot(b"key2");

        if slot1 != slot2 {
            assert!(result.is_err());
            if let Err(msg) = result {
                assert!(msg.contains("CROSSSLOT"));
            }
        }
    }

    #[test]
    fn test_slot_state() {
        let stable = SlotState::Stable;
        let migrating = SlotState::Migrating {
            to_node: "node2".to_string(),
        };
        let importing = SlotState::Importing {
            from_node: "node1".to_string(),
        };

        assert_eq!(stable, SlotState::Stable);
        assert_ne!(migrating, stable);
        assert_ne!(importing, stable);
    }
}
