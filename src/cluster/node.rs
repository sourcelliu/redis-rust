// Cluster node management

use std::collections::HashSet;
use std::net::SocketAddr;
use std::time::{SystemTime, UNIX_EPOCH};

/// Node flags representing the state and role of a cluster node
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NodeFlags {
    /// This node is a master
    Master,
    /// This node is a slave/replica
    Slave,
    /// This is the node we're connected to (myself)
    Myself,
    /// The node is in a failed state
    Fail,
    /// The node might be failing (PFAIL)
    PFail,
    /// Handshake in progress
    Handshake,
    /// Node has no address yet
    NoAddr,
    /// Node address needs update
    NoFlags,
}

impl NodeFlags {
    pub fn to_string(&self) -> &'static str {
        match self {
            NodeFlags::Master => "master",
            NodeFlags::Slave => "slave",
            NodeFlags::Myself => "myself",
            NodeFlags::Fail => "fail",
            NodeFlags::PFail => "fail?",
            NodeFlags::Handshake => "handshake",
            NodeFlags::NoAddr => "noaddr",
            NodeFlags::NoFlags => "noflags",
        }
    }

    /// Parse flags from a comma-separated string
    pub fn parse_flags(flags_str: &str) -> Vec<NodeFlags> {
        flags_str
            .split(',')
            .filter_map(|s| match s.trim() {
                "master" => Some(NodeFlags::Master),
                "slave" => Some(NodeFlags::Slave),
                "myself" => Some(NodeFlags::Myself),
                "fail" => Some(NodeFlags::Fail),
                "fail?" => Some(NodeFlags::PFail),
                "handshake" => Some(NodeFlags::Handshake),
                "noaddr" => Some(NodeFlags::NoAddr),
                _ => None,
            })
            .collect()
    }
}

/// Represents a node in the Redis Cluster
#[derive(Debug, Clone)]
pub struct ClusterNode {
    /// Unique 40-character node ID
    pub id: String,

    /// IP address and port (e.g., "127.0.0.1:7000")
    pub addr: Option<SocketAddr>,

    /// Node flags (master, slave, myself, etc.)
    pub flags: Vec<NodeFlags>,

    /// Master node ID if this is a replica
    pub master_id: Option<String>,

    /// Ping sent timestamp (milliseconds)
    pub ping_sent: u64,

    /// Pong received timestamp (milliseconds)
    pub pong_recv: u64,

    /// Configuration epoch
    pub config_epoch: u64,

    /// Link state: "connected" or "disconnected"
    pub link_state: String,

    /// Hash slots assigned to this node (for masters)
    pub slots: HashSet<u16>,
}

impl ClusterNode {
    /// Create a new cluster node
    pub fn new(id: String, addr: Option<SocketAddr>) -> Self {
        Self {
            id,
            addr,
            flags: vec![],
            master_id: None,
            ping_sent: 0,
            pong_recv: Self::current_time_millis(),
            config_epoch: 0,
            link_state: "connected".to_string(),
            slots: HashSet::new(),
        }
    }

    /// Create a new master node
    pub fn new_master(id: String, addr: Option<SocketAddr>) -> Self {
        let mut node = Self::new(id, addr);
        node.flags.push(NodeFlags::Master);
        node
    }

    /// Create a new replica node
    pub fn new_replica(id: String, addr: Option<SocketAddr>, master_id: String) -> Self {
        let mut node = Self::new(id, addr);
        node.flags.push(NodeFlags::Slave);
        node.master_id = Some(master_id);
        node
    }

    /// Check if this is a master node
    pub fn is_master(&self) -> bool {
        self.flags.contains(&NodeFlags::Master)
    }

    /// Check if this is a replica/slave node
    pub fn is_slave(&self) -> bool {
        self.flags.contains(&NodeFlags::Slave)
    }

    /// Check if this node is in failed state
    pub fn is_failed(&self) -> bool {
        self.flags.contains(&NodeFlags::Fail)
    }

    /// Add a flag to this node
    pub fn add_flag(&mut self, flag: NodeFlags) {
        if !self.flags.contains(&flag) {
            self.flags.push(flag);
        }
    }

    /// Remove a flag from this node
    pub fn remove_flag(&mut self, flag: &NodeFlags) {
        self.flags.retain(|f| f != flag);
    }

    /// Convert flags to comma-separated string
    pub fn flags_to_string(&self) -> String {
        self.flags
            .iter()
            .map(|f| f.to_string())
            .collect::<Vec<_>>()
            .join(",")
    }

    /// Assign a slot to this node
    pub fn add_slot(&mut self, slot: u16) {
        self.slots.insert(slot);
    }

    /// Assign a range of slots to this node
    pub fn add_slot_range(&mut self, start: u16, end: u16) {
        for slot in start..=end {
            self.slots.insert(slot);
        }
    }

    /// Remove a slot from this node
    pub fn remove_slot(&mut self, slot: u16) {
        self.slots.remove(&slot);
    }

    /// Check if this node owns a specific slot
    pub fn owns_slot(&self, slot: u16) -> bool {
        self.slots.contains(&slot)
    }

    /// Get all slots owned by this node as a sorted vector
    pub fn get_slots(&self) -> Vec<u16> {
        let mut slots: Vec<u16> = self.slots.iter().copied().collect();
        slots.sort_unstable();
        slots
    }

    /// Get slot ranges as compressed format: [[0, 100], [200, 300]]
    pub fn get_slot_ranges(&self) -> Vec<(u16, u16)> {
        let slots = self.get_slots();
        if slots.is_empty() {
            return vec![];
        }

        let mut ranges = vec![];
        let mut start = slots[0];
        let mut end = slots[0];

        for &slot in &slots[1..] {
            if slot == end + 1 {
                end = slot;
            } else {
                ranges.push((start, end));
                start = slot;
                end = slot;
            }
        }
        ranges.push((start, end));
        ranges
    }

    /// Format node info for CLUSTER NODES command
    /// Format: <id> <ip:port> <flags> <master> <ping-sent> <pong-recv> <config-epoch> <link-state> <slot> <slot> ... <slot>
    pub fn to_cluster_nodes_line(&self) -> String {
        let id = &self.id;
        let addr = self.addr
            .map(|a| a.to_string())
            .unwrap_or_else(|| ":0".to_string());

        let flags = self.flags
            .iter()
            .map(|f| f.to_string())
            .collect::<Vec<_>>()
            .join(",");

        let master = self.master_id.as_deref()
            .unwrap_or("-");

        let slots_str = if self.slots.is_empty() {
            String::new()
        } else {
            let ranges = self.get_slot_ranges();
            let mut parts = vec![];
            for (start, end) in ranges {
                if start == end {
                    parts.push(format!("{}", start));
                } else {
                    parts.push(format!("{}-{}", start, end));
                }
            }
            format!(" {}", parts.join(" "))
        };

        format!(
            "{} {} {} {} {} {} {} {}{}",
            id,
            addr,
            flags,
            master,
            self.ping_sent,
            self.pong_recv,
            self.config_epoch,
            self.link_state,
            slots_str
        )
    }

    /// Get current time in milliseconds
    fn current_time_millis() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn test_node_creation() {
        let addr = SocketAddr::from_str("127.0.0.1:7000").ok();
        let node = ClusterNode::new("abc123".to_string(), addr);

        assert_eq!(node.id, "abc123");
        assert_eq!(node.addr, addr);
        assert_eq!(node.link_state, "connected");
        assert!(node.slots.is_empty());
    }

    #[test]
    fn test_master_node() {
        let addr = SocketAddr::from_str("127.0.0.1:7000").ok();
        let node = ClusterNode::new_master("master1".to_string(), addr);

        assert!(node.is_master());
        assert!(!node.is_slave());
        assert_eq!(node.flags.len(), 1);
    }

    #[test]
    fn test_replica_node() {
        let addr = SocketAddr::from_str("127.0.0.1:7001").ok();
        let node = ClusterNode::new_replica(
            "replica1".to_string(),
            addr,
            "master1".to_string()
        );

        assert!(node.is_slave());
        assert!(!node.is_master());
        assert_eq!(node.master_id, Some("master1".to_string()));
    }

    #[test]
    fn test_slot_assignment() {
        let mut node = ClusterNode::new_master("node1".to_string(), None);

        // Add individual slots
        node.add_slot(0);
        node.add_slot(100);
        node.add_slot(1000);

        assert!(node.owns_slot(0));
        assert!(node.owns_slot(100));
        assert!(node.owns_slot(1000));
        assert!(!node.owns_slot(500));

        assert_eq!(node.slots.len(), 3);
    }

    #[test]
    fn test_slot_range_assignment() {
        let mut node = ClusterNode::new_master("node1".to_string(), None);

        // Assign range 0-1000
        node.add_slot_range(0, 1000);

        assert_eq!(node.slots.len(), 1001);
        assert!(node.owns_slot(0));
        assert!(node.owns_slot(500));
        assert!(node.owns_slot(1000));
        assert!(!node.owns_slot(1001));
    }

    #[test]
    fn test_slot_ranges_compression() {
        let mut node = ClusterNode::new_master("node1".to_string(), None);

        // Add continuous range
        node.add_slot_range(0, 100);
        // Add another continuous range
        node.add_slot_range(200, 300);
        // Add single slot
        node.add_slot(500);

        let ranges = node.get_slot_ranges();
        assert_eq!(ranges.len(), 3);
        assert_eq!(ranges[0], (0, 100));
        assert_eq!(ranges[1], (200, 300));
        assert_eq!(ranges[2], (500, 500));
    }

    #[test]
    fn test_cluster_nodes_output() {
        let addr = SocketAddr::from_str("127.0.0.1:7000").ok();
        let mut node = ClusterNode::new_master("abc123".to_string(), addr);
        node.add_flag(NodeFlags::Myself);
        node.add_slot_range(0, 100);
        node.add_slot_range(200, 300);

        let output = node.to_cluster_nodes_line();

        assert!(output.contains("abc123"));
        assert!(output.contains("127.0.0.1:7000"));
        assert!(output.contains("master"));
        assert!(output.contains("myself"));
        assert!(output.contains("0-100"));
        assert!(output.contains("200-300"));
    }

    #[test]
    fn test_flag_management() {
        let mut node = ClusterNode::new("node1".to_string(), None);

        node.add_flag(NodeFlags::Master);
        assert!(node.is_master());

        node.add_flag(NodeFlags::Fail);
        assert!(node.is_failed());

        node.remove_flag(&NodeFlags::Fail);
        assert!(!node.is_failed());
    }

    #[test]
    fn test_flag_parsing() {
        let flags = NodeFlags::parse_flags("master,myself");
        assert_eq!(flags.len(), 2);
        assert!(flags.contains(&NodeFlags::Master));
        assert!(flags.contains(&NodeFlags::Myself));
    }
}
