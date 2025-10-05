// Slot migration management

use crate::cluster::{ClusterState, SlotState};
use crate::protocol::RespValue;
use crate::storage::db::Database;
use dashmap::DashMap;
use std::sync::Arc;

/// Migration state tracker for slots
pub struct MigrationManager {
    /// Slot migration states: slot -> state
    slot_states: Arc<DashMap<u16, SlotState>>,
}

impl MigrationManager {
    pub fn new() -> Self {
        Self {
            slot_states: Arc::new(DashMap::new()),
        }
    }

    /// Mark a slot as importing from another node
    pub fn set_importing(&self, slot: u16, from_node: String) {
        self.slot_states.insert(
            slot,
            SlotState::Importing { from_node },
        );
    }

    /// Mark a slot as migrating to another node
    pub fn set_migrating(&self, slot: u16, to_node: String) {
        self.slot_states.insert(
            slot,
            SlotState::Migrating { to_node },
        );
    }

    /// Mark a slot as stable (migration complete)
    pub fn set_stable(&self, slot: u16) {
        self.slot_states.remove(&slot);
    }

    /// Get the state of a slot
    pub fn get_state(&self, slot: u16) -> SlotState {
        self.slot_states
            .get(&slot)
            .map(|s| s.value().clone())
            .unwrap_or(SlotState::Stable)
    }

    /// Check if a slot is being migrated
    pub fn is_migrating(&self, slot: u16) -> bool {
        matches!(self.get_state(slot), SlotState::Migrating { .. })
    }

    /// Check if a slot is being imported
    pub fn is_importing(&self, slot: u16) -> bool {
        matches!(self.get_state(slot), SlotState::Importing { .. })
    }
}

impl Default for MigrationManager {
    fn default() -> Self {
        Self::new()
    }
}

/// CLUSTER SETSLOT <slot> IMPORTING <node-id>
pub fn cluster_setslot_importing(
    cluster: &Arc<ClusterState>,
    migration: &Arc<MigrationManager>,
    slot: u16,
    node_id: String,
) -> RespValue {
    if !cluster.enabled {
        return RespValue::Error("ERR This instance has cluster support disabled".to_string());
    }

    // Validate node exists
    if cluster.get_node(&node_id).is_none() {
        return RespValue::Error(format!("ERR Unknown node {}", node_id));
    }

    migration.set_importing(slot, node_id);
    RespValue::SimpleString("OK".to_string())
}

/// CLUSTER SETSLOT <slot> MIGRATING <node-id>
pub fn cluster_setslot_migrating(
    cluster: &Arc<ClusterState>,
    migration: &Arc<MigrationManager>,
    slot: u16,
    node_id: String,
) -> RespValue {
    if !cluster.enabled {
        return RespValue::Error("ERR This instance has cluster support disabled".to_string());
    }

    // Must own the slot to migrate it
    if !cluster.owns_slot(slot) {
        return RespValue::Error(format!(
            "ERR I'm not the owner of hash slot {}",
            slot
        ));
    }

    // Validate target node exists
    if cluster.get_node(&node_id).is_none() {
        return RespValue::Error(format!("ERR Unknown node {}", node_id));
    }

    migration.set_migrating(slot, node_id);
    RespValue::SimpleString("OK".to_string())
}

/// CLUSTER SETSLOT <slot> STABLE
pub fn cluster_setslot_stable(
    cluster: &Arc<ClusterState>,
    migration: &Arc<MigrationManager>,
    slot: u16,
) -> RespValue {
    if !cluster.enabled {
        return RespValue::Error("ERR This instance has cluster support disabled".to_string());
    }

    migration.set_stable(slot);
    RespValue::SimpleString("OK".to_string())
}

/// CLUSTER SETSLOT <slot> NODE <node-id>
pub fn cluster_setslot_node(
    cluster: &Arc<ClusterState>,
    migration: &Arc<MigrationManager>,
    slot: u16,
    node_id: String,
) -> RespValue {
    if !cluster.enabled {
        return RespValue::Error("ERR This instance has cluster support disabled".to_string());
    }

    // Validate node exists
    if cluster.get_node(&node_id).is_none() {
        return RespValue::Error(format!("ERR Unknown node {}", node_id));
    }

    // Assign the slot
    cluster.assign_slots_to_node(&node_id, vec![slot]);

    // Mark as stable
    migration.set_stable(slot);

    RespValue::SimpleString("OK".to_string())
}

/// CLUSTER GETKEYSINSLOT <slot> <count>
/// Get up to <count> keys in a specific slot
/// (Placeholder - requires database integration)
pub fn cluster_getkeysinslot(
    cluster: &Arc<ClusterState>,
    _db: &Arc<Database>,
    _db_index: usize,
    _slot: u16,
    count: i64,
) -> RespValue {
    if !cluster.enabled {
        return RespValue::Error("ERR This instance has cluster support disabled".to_string());
    }

    if count < 0 {
        return RespValue::Error("ERR count must be positive".to_string());
    }

    // TODO: Implement actual key scanning when database API is available
    // For now, return empty array
    RespValue::Array(Some(vec![]))
}

/// CLUSTER COUNTKEYSINSLOT <slot>
/// Count keys in a specific slot
/// (Placeholder - requires database integration)
pub fn cluster_countkeysinslot(
    cluster: &Arc<ClusterState>,
    _db: &Arc<Database>,
    _db_index: usize,
    _slot: u16,
) -> RespValue {
    if !cluster.enabled {
        return RespValue::Error("ERR This instance has cluster support disabled".to_string());
    }

    // TODO: Implement actual key counting when database API is available
    // For now, return 0
    RespValue::Integer(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_migration_manager_creation() {
        let mgr = MigrationManager::new();

        // Initially all slots are stable
        assert_eq!(mgr.get_state(0), SlotState::Stable);
        assert!(!mgr.is_migrating(0));
        assert!(!mgr.is_importing(0));
    }

    #[test]
    fn test_set_importing() {
        let mgr = MigrationManager::new();

        mgr.set_importing(100, "node123".to_string());

        assert!(mgr.is_importing(100));
        assert!(!mgr.is_migrating(100));

        match mgr.get_state(100) {
            SlotState::Importing { from_node } => {
                assert_eq!(from_node, "node123");
            }
            _ => panic!("Expected Importing state"),
        }
    }

    #[test]
    fn test_set_migrating() {
        let mgr = MigrationManager::new();

        mgr.set_migrating(200, "node456".to_string());

        assert!(mgr.is_migrating(200));
        assert!(!mgr.is_importing(200));

        match mgr.get_state(200) {
            SlotState::Migrating { to_node } => {
                assert_eq!(to_node, "node456");
            }
            _ => panic!("Expected Migrating state"),
        }
    }

    #[test]
    fn test_set_stable() {
        let mgr = MigrationManager::new();

        // Set to migrating first
        mgr.set_migrating(300, "node789".to_string());
        assert!(mgr.is_migrating(300));

        // Then mark as stable
        mgr.set_stable(300);
        assert_eq!(mgr.get_state(300), SlotState::Stable);
        assert!(!mgr.is_migrating(300));
    }

    #[test]
    fn test_cluster_setslot_importing() {
        let cluster = Arc::new(ClusterState::new(true));
        let migration = Arc::new(MigrationManager::new());

        // Add a node first
        let node = crate::cluster::node::ClusterNode::new_master(
            "source_node".to_string(),
            None
        );
        cluster.add_node(node);

        let result = cluster_setslot_importing(
            &cluster,
            &migration,
            100,
            "source_node".to_string()
        );

        assert_eq!(result, RespValue::SimpleString("OK".to_string()));
        assert!(migration.is_importing(100));
    }

    #[test]
    fn test_cluster_setslot_migrating() {
        let cluster = Arc::new(ClusterState::new(true));
        let migration = Arc::new(MigrationManager::new());

        // Assign slot to ourselves
        cluster.add_slot(200);

        // Add target node
        let node = crate::cluster::node::ClusterNode::new_master(
            "target_node".to_string(),
            None
        );
        cluster.add_node(node);

        let result = cluster_setslot_migrating(
            &cluster,
            &migration,
            200,
            "target_node".to_string()
        );

        assert_eq!(result, RespValue::SimpleString("OK".to_string()));
        assert!(migration.is_migrating(200));
    }

    #[test]
    fn test_cluster_setslot_stable() {
        let cluster = Arc::new(ClusterState::new(true));
        let migration = Arc::new(MigrationManager::new());

        // Set migrating first
        migration.set_migrating(300, "some_node".to_string());
        assert!(migration.is_migrating(300));

        // Mark as stable
        let result = cluster_setslot_stable(&cluster, &migration, 300);

        assert_eq!(result, RespValue::SimpleString("OK".to_string()));
        assert_eq!(migration.get_state(300), SlotState::Stable);
    }

    #[test]
    fn test_cluster_setslot_node() {
        let cluster = Arc::new(ClusterState::new(true));
        let migration = Arc::new(MigrationManager::new());

        // Add target node
        let node = crate::cluster::node::ClusterNode::new_master(
            "new_owner".to_string(),
            None
        );
        cluster.add_node(node);

        let result = cluster_setslot_node(
            &cluster,
            &migration,
            400,
            "new_owner".to_string()
        );

        assert_eq!(result, RespValue::SimpleString("OK".to_string()));
        assert_eq!(cluster.get_slot_node(400), Some("new_owner".to_string()));
        assert_eq!(migration.get_state(400), SlotState::Stable);
    }
}
