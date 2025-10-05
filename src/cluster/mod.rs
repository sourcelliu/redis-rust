// Cluster module - Redis Cluster implementation

pub mod node;
pub mod slots;
pub mod redirection;
pub mod migration;
pub mod config;

use dashmap::DashMap;
use node::ClusterNode;
use std::sync::Arc;
pub use slots::{key_hash_slot, CLUSTER_SLOTS};
pub use redirection::{check_slot_ownership, check_multi_key_slot, SlotState};
pub use migration::MigrationManager;
pub use config::{ConfigEpoch, save_cluster_config, load_cluster_config, auto_save_cluster_config};

/// Cluster state management
pub struct ClusterState {
    pub enabled: bool,
    pub my_id: String,
    pub slot_map: Arc<DashMap<u16, String>>, // slot -> node_id
    pub nodes: Arc<DashMap<String, ClusterNode>>, // node_id -> node
}

impl ClusterState {
    pub fn new(enabled: bool) -> Self {
        let my_id = Self::generate_node_id();
        let nodes = Arc::new(DashMap::new());

        // Add myself as a node
        if enabled {
            let mut myself = ClusterNode::new_master(my_id.clone(), None);
            myself.add_flag(node::NodeFlags::Myself);
            nodes.insert(my_id.clone(), myself);
        }

        Self {
            enabled,
            my_id,
            slot_map: Arc::new(DashMap::new()),
            nodes,
        }
    }

    /// Generate a unique node ID (40 hex chars)
    fn generate_node_id() -> String {
        use std::time::{SystemTime, UNIX_EPOCH};
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        // Use a simpler approach: hash the timestamp
        format!("{:040x}", timestamp)
    }

    /// Assign a slot to this node
    pub fn add_slot(&self, slot: u16) {
        self.slot_map.insert(slot, self.my_id.clone());
    }

    /// Remove slot assignment from this node
    pub fn del_slot(&self, slot: u16) {
        self.slot_map.remove(&slot);
    }

    /// Check if this node owns a slot
    pub fn owns_slot(&self, slot: u16) -> bool {
        self.slot_map
            .get(&slot)
            .map(|r| r.value() == &self.my_id)
            .unwrap_or(false)
    }

    /// Get the node ID responsible for a slot
    pub fn get_slot_node(&self, slot: u16) -> Option<String> {
        self.slot_map.get(&slot).map(|r| r.value().clone())
    }

    /// Get all slots owned by this node
    pub fn get_my_slots(&self) -> Vec<u16> {
        self.slot_map
            .iter()
            .filter(|r| r.value() == &self.my_id)
            .map(|r| *r.key())
            .collect()
    }

    /// Count how many slots this node owns
    pub fn count_my_slots(&self) -> usize {
        self.slot_map
            .iter()
            .filter(|r| r.value() == &self.my_id)
            .count()
    }

    /// Add a node to the cluster
    pub fn add_node(&self, node: ClusterNode) {
        self.nodes.insert(node.id.clone(), node);
    }

    /// Remove a node from the cluster
    pub fn remove_node(&self, node_id: &str) {
        self.nodes.remove(node_id);
    }

    /// Get a node by ID
    pub fn get_node(&self, node_id: &str) -> Option<ClusterNode> {
        self.nodes.get(node_id).map(|r| r.value().clone())
    }

    /// Get all nodes in the cluster
    pub fn get_all_nodes(&self) -> Vec<ClusterNode> {
        self.nodes.iter().map(|r| r.value().clone()).collect()
    }

    /// Get all master nodes
    pub fn get_master_nodes(&self) -> Vec<ClusterNode> {
        self.nodes
            .iter()
            .filter(|r| r.value().is_master())
            .map(|r| r.value().clone())
            .collect()
    }

    /// Get all replica nodes for a specific master
    pub fn get_replicas(&self, master_id: &str) -> Vec<ClusterNode> {
        self.nodes
            .iter()
            .filter(|r| {
                r.value().is_slave() && r.value().master_id.as_deref() == Some(master_id)
            })
            .map(|r| r.value().clone())
            .collect()
    }

    /// Update node slot assignment
    pub fn assign_slots_to_node(&self, node_id: &str, slots: Vec<u16>) {
        // Update slot_map
        for slot in &slots {
            self.slot_map.insert(*slot, node_id.to_string());
        }

        // Update node's slots if it exists
        if let Some(mut node_ref) = self.nodes.get_mut(node_id) {
            for slot in slots {
                node_ref.add_slot(slot);
            }
        }
    }

    /// Remove slots from a node
    pub fn remove_slots_from_node(&self, node_id: &str, slots: Vec<u16>) {
        // Remove from slot_map
        for slot in &slots {
            if let Some(owner) = self.slot_map.get(slot) {
                if owner.value() == node_id {
                    self.slot_map.remove(slot);
                }
            }
        }

        // Remove from node's slots if it exists
        if let Some(mut node_ref) = self.nodes.get_mut(node_id) {
            for slot in slots {
                node_ref.remove_slot(slot);
            }
        }
    }
}

impl Default for ClusterState {
    fn default() -> Self {
        Self::new(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cluster_state_creation() {
        let state = ClusterState::new(true);
        assert!(state.enabled);
        assert_eq!(state.my_id.len(), 40); // 40 hex chars
        assert_eq!(state.count_my_slots(), 0);
    }

    #[test]
    fn test_slot_assignment() {
        let state = ClusterState::new(true);

        // Initially no slots
        assert!(!state.owns_slot(0));
        assert!(!state.owns_slot(100));

        // Add slots
        state.add_slot(0);
        state.add_slot(100);
        state.add_slot(1000);

        assert!(state.owns_slot(0));
        assert!(state.owns_slot(100));
        assert!(state.owns_slot(1000));
        assert!(!state.owns_slot(1));

        assert_eq!(state.count_my_slots(), 3);

        // Remove slot
        state.del_slot(100);
        assert!(!state.owns_slot(100));
        assert_eq!(state.count_my_slots(), 2);
    }

    #[test]
    fn test_get_my_slots() {
        let state = ClusterState::new(true);

        state.add_slot(0);
        state.add_slot(100);
        state.add_slot(16383);

        let mut slots = state.get_my_slots();
        slots.sort();

        assert_eq!(slots, vec![0, 100, 16383]);
    }

    #[test]
    fn test_get_slot_node() {
        let state = ClusterState::new(true);

        assert!(state.get_slot_node(0).is_none());

        state.add_slot(0);
        let node_id = state.get_slot_node(0);
        assert!(node_id.is_some());
        assert_eq!(node_id.unwrap(), state.my_id);
    }

    #[test]
    fn test_node_management() {
        let state = ClusterState::new(true);

        // Initially only has myself
        let nodes = state.get_all_nodes();
        assert_eq!(nodes.len(), 1);
        assert!(nodes[0].flags.contains(&node::NodeFlags::Myself));

        // Add another master node
        let new_node = ClusterNode::new_master("node2".to_string(), None);
        state.add_node(new_node);

        assert_eq!(state.get_all_nodes().len(), 2);
        assert_eq!(state.get_master_nodes().len(), 2);

        // Remove the node
        state.remove_node("node2");
        assert_eq!(state.get_all_nodes().len(), 1);
    }

    #[test]
    fn test_replica_tracking() {
        let state = ClusterState::new(true);

        // Add a replica for myself
        let replica = ClusterNode::new_replica(
            "replica1".to_string(),
            None,
            state.my_id.clone()
        );
        state.add_node(replica);

        let replicas = state.get_replicas(&state.my_id);
        assert_eq!(replicas.len(), 1);
        assert_eq!(replicas[0].id, "replica1");
    }

    #[test]
    fn test_slot_assignment_to_nodes() {
        let state = ClusterState::new(true);

        // Add another node
        let node2 = ClusterNode::new_master("node2".to_string(), None);
        state.add_node(node2);

        // Assign slots to node2
        state.assign_slots_to_node("node2", vec![0, 1, 2, 100]);

        // Verify slot_map updated
        assert_eq!(state.get_slot_node(0), Some("node2".to_string()));
        assert_eq!(state.get_slot_node(1), Some("node2".to_string()));
        assert_eq!(state.get_slot_node(100), Some("node2".to_string()));

        // Verify node's slots updated
        let node2_updated = state.get_node("node2").unwrap();
        assert!(node2_updated.owns_slot(0));
        assert!(node2_updated.owns_slot(100));

        // Remove some slots
        state.remove_slots_from_node("node2", vec![0, 1]);
        assert!(state.get_slot_node(0).is_none());
        assert!(state.get_slot_node(1).is_none());
        assert_eq!(state.get_slot_node(100), Some("node2".to_string()));
    }
}
