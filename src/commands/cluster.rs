// CLUSTER command implementation

use crate::cluster::{key_hash_slot, ClusterState};
use crate::protocol::RespValue;
use std::sync::Arc;

/// CLUSTER NODES - List all cluster nodes
/// Format: <id> <ip:port> <flags> <master> <ping-sent> <pong-recv> <config-epoch> <link-state> <slot>...
pub fn cluster_nodes(cluster: &Arc<ClusterState>) -> RespValue {
    if !cluster.enabled {
        return RespValue::Error("ERR This instance has cluster support disabled".to_string());
    }

    let nodes = cluster.get_all_nodes();
    let mut lines = Vec::new();

    for node in nodes {
        lines.push(node.to_cluster_nodes_line());
    }

    let output = lines.join("\n");
    RespValue::BulkString(Some(output.into_bytes()))
}

/// CLUSTER SLOTS - Get cluster slot mapping
/// Returns array of [start_slot, end_slot, [master_ip, master_port], [replica_ip, replica_port], ...]
pub fn cluster_slots(cluster: &Arc<ClusterState>) -> RespValue {
    if !cluster.enabled {
        return RespValue::Error("ERR This instance has cluster support disabled".to_string());
    }

    let mut result = Vec::new();
    let masters = cluster.get_master_nodes();

    for master in masters {
        let ranges = master.get_slot_ranges();
        if ranges.is_empty() {
            continue;
        }

        for (start, end) in ranges {
            let mut slot_info = vec![
                RespValue::Integer(start as i64),
                RespValue::Integer(end as i64),
            ];

            // Add master node info
            if let Some(addr) = master.addr {
                let master_info = vec![
                    RespValue::BulkString(Some(addr.ip().to_string().into_bytes())),
                    RespValue::Integer(addr.port() as i64),
                    RespValue::BulkString(Some(master.id.as_bytes().to_vec())),
                ];
                slot_info.push(RespValue::Array(Some(master_info)));
            }

            // Add replica nodes info
            let replicas = cluster.get_replicas(&master.id);
            for replica in replicas {
                if let Some(addr) = replica.addr {
                    let replica_info = vec![
                        RespValue::BulkString(Some(addr.ip().to_string().into_bytes())),
                        RespValue::Integer(addr.port() as i64),
                        RespValue::BulkString(Some(replica.id.as_bytes().to_vec())),
                    ];
                    slot_info.push(RespValue::Array(Some(replica_info)));
                }
            }

            result.push(RespValue::Array(Some(slot_info)));
        }
    }

    RespValue::Array(Some(result))
}

/// CLUSTER ADDSLOTS slot [slot ...]
/// Assign slots to this node
pub fn cluster_addslots(cluster: &Arc<ClusterState>, slots: Vec<u16>) -> RespValue {
    if !cluster.enabled {
        return RespValue::Error("ERR This instance has cluster support disabled".to_string());
    }

    // Check if any slot is already assigned
    for &slot in &slots {
        if let Some(owner) = cluster.get_slot_node(slot) {
            return RespValue::Error(format!(
                "ERR Slot {} is already assigned to node {}",
                slot, owner
            ));
        }
    }

    // Assign all slots to this node
    for slot in slots {
        cluster.add_slot(slot);
    }

    RespValue::SimpleString("OK".to_string())
}

/// CLUSTER DELSLOTS slot [slot ...]
/// Remove slot assignments from this node
pub fn cluster_delslots(cluster: &Arc<ClusterState>, slots: Vec<u16>) -> RespValue {
    if !cluster.enabled {
        return RespValue::Error("ERR This instance has cluster support disabled".to_string());
    }

    // Remove all specified slots
    for slot in slots {
        cluster.del_slot(slot);
    }

    RespValue::SimpleString("OK".to_string())
}

/// CLUSTER MEET ip port
/// Add a node to the cluster (placeholder - full implementation needs gossip protocol)
pub fn cluster_meet(
    cluster: &Arc<ClusterState>,
    _ip: String,
    _port: u16,
) -> RespValue {
    if !cluster.enabled {
        return RespValue::Error("ERR This instance has cluster support disabled".to_string());
    }

    // TODO: Implement actual handshake and node addition
    // For now, just return OK
    RespValue::SimpleString("OK".to_string())
}

/// CLUSTER FORGET node-id
/// Remove a node from the cluster
pub fn cluster_forget(cluster: &Arc<ClusterState>, node_id: String) -> RespValue {
    if !cluster.enabled {
        return RespValue::Error("ERR This instance has cluster support disabled".to_string());
    }

    // Don't allow forgetting myself
    if node_id == cluster.my_id {
        return RespValue::Error("ERR I tried hard but I can't forget myself...".to_string());
    }

    cluster.remove_node(&node_id);
    RespValue::SimpleString("OK".to_string())
}

/// CLUSTER REPLICATE node-id
/// Make this node a replica of the specified master
pub fn cluster_replicate(
    cluster: &Arc<ClusterState>,
    _master_id: String,
) -> RespValue {
    if !cluster.enabled {
        return RespValue::Error("ERR This instance has cluster support disabled".to_string());
    }

    // TODO: Implement actual replication setup
    // For now, just return OK
    RespValue::SimpleString("OK".to_string())
}

/// CLUSTER INFO - Get cluster state information
pub fn cluster_info(cluster: &Arc<ClusterState>) -> RespValue {
    if !cluster.enabled {
        return RespValue::Error("ERR This instance has cluster support disabled".to_string());
    }

    let state = if cluster.enabled { "ok" } else { "fail" };
    let slots_assigned = cluster.slot_map.len();
    let slots_ok = slots_assigned;
    let known_nodes = cluster.nodes.len();
    let size = cluster.get_master_nodes().len();

    let info = format!(
        "cluster_state:{}\n\
         cluster_slots_assigned:{}\n\
         cluster_slots_ok:{}\n\
         cluster_slots_pfail:0\n\
         cluster_slots_fail:0\n\
         cluster_known_nodes:{}\n\
         cluster_size:{}\n\
         cluster_current_epoch:0\n\
         cluster_my_epoch:0\n\
         cluster_stats_messages_sent:0\n\
         cluster_stats_messages_received:0\n",
        state, slots_assigned, slots_ok, known_nodes, size
    );

    RespValue::BulkString(Some(info.into_bytes()))
}

/// CLUSTER MYID - Get this node's ID
pub fn cluster_myid(cluster: &Arc<ClusterState>) -> RespValue {
    if !cluster.enabled {
        return RespValue::Error("ERR This instance has cluster support disabled".to_string());
    }

    RespValue::BulkString(Some(cluster.my_id.as_bytes().to_vec()))
}

/// CLUSTER KEYSLOT key - Get hash slot for a key
pub fn cluster_keyslot(key: &[u8]) -> RespValue {
    let slot = key_hash_slot(key);
    RespValue::Integer(slot as i64)
}

/// CLUSTER COUNTKEYSINSLOT slot - Count keys in a slot
/// (Placeholder - requires integration with database)
pub fn cluster_countkeysinslot(_cluster: &Arc<ClusterState>, _slot: u16) -> RespValue {
    // TODO: Integrate with database to count keys
    RespValue::Integer(0)
}

/// CLUSTER GETKEYSINSLOT slot count - Get keys in a slot
/// (Placeholder - requires integration with database)
pub fn cluster_getkeysinslot(
    _cluster: &Arc<ClusterState>,
    _slot: u16,
    _count: i64,
) -> RespValue {
    // TODO: Integrate with database to retrieve keys
    RespValue::Array(Some(vec![]))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::SocketAddr;
    use std::str::FromStr;

    #[test]
    fn test_cluster_info() {
        let cluster = Arc::new(ClusterState::new(true));
        let result = cluster_info(&cluster);

        match result {
            RespValue::BulkString(Some(data)) => {
                let info = String::from_utf8(data).unwrap();
                assert!(info.contains("cluster_state:ok"));
                assert!(info.contains("cluster_known_nodes:"));
            }
            _ => panic!("Expected BulkString"),
        }
    }

    #[test]
    fn test_cluster_myid() {
        let cluster = Arc::new(ClusterState::new(true));
        let result = cluster_myid(&cluster);

        match result {
            RespValue::BulkString(Some(data)) => {
                let id = String::from_utf8(data).unwrap();
                assert_eq!(id.len(), 40); // 40 hex chars
                assert_eq!(id, cluster.my_id);
            }
            _ => panic!("Expected BulkString"),
        }
    }

    #[test]
    fn test_cluster_keyslot() {
        let result = cluster_keyslot(b"mykey");
        match result {
            RespValue::Integer(slot) => {
                assert!(slot >= 0 && slot < 16384);
            }
            _ => panic!("Expected Integer"),
        }

        // Keys with same hash tag should have same slot
        let slot1 = cluster_keyslot(b"{user}:profile");
        let slot2 = cluster_keyslot(b"{user}:settings");
        assert_eq!(slot1, slot2);
    }

    #[test]
    fn test_cluster_addslots() {
        let cluster = Arc::new(ClusterState::new(true));
        let result = cluster_addslots(&cluster, vec![0, 1, 2, 100]);

        assert_eq!(result, RespValue::SimpleString("OK".to_string()));
        assert!(cluster.owns_slot(0));
        assert!(cluster.owns_slot(100));

        // Try to add already assigned slot
        let result = cluster_addslots(&cluster, vec![0]);
        match result {
            RespValue::Error(msg) => {
                assert!(msg.contains("already assigned"));
            }
            _ => panic!("Expected Error"),
        }
    }

    #[test]
    fn test_cluster_delslots() {
        let cluster = Arc::new(ClusterState::new(true));
        cluster.add_slot(0);
        cluster.add_slot(100);

        let result = cluster_delslots(&cluster, vec![0, 100]);
        assert_eq!(result, RespValue::SimpleString("OK".to_string()));
        assert!(!cluster.owns_slot(0));
        assert!(!cluster.owns_slot(100));
    }

    #[test]
    fn test_cluster_nodes() {
        let cluster = Arc::new(ClusterState::new(true));
        let result = cluster_nodes(&cluster);

        match result {
            RespValue::BulkString(Some(data)) => {
                let output = String::from_utf8(data).unwrap();
                assert!(output.contains(&cluster.my_id));
                assert!(output.contains("myself"));
            }
            _ => panic!("Expected BulkString"),
        }
    }

    #[test]
    fn test_cluster_slots() {
        let cluster = Arc::new(ClusterState::new(true));

        // Assign some slots
        cluster.add_slot(0);
        cluster.add_slot(1);
        cluster.add_slot(2);

        // Set address for the node
        let addr = SocketAddr::from_str("127.0.0.1:7000").ok();
        if let Some(mut node) = cluster.nodes.get_mut(&cluster.my_id) {
            node.addr = addr;
        }

        let result = cluster_slots(&cluster);

        match result {
            RespValue::Array(Some(slots)) => {
                assert!(!slots.is_empty());
                // Each slot range should have at least start, end, and master info
                if let RespValue::Array(Some(slot_info)) = &slots[0] {
                    assert!(slot_info.len() >= 3);
                }
            }
            _ => panic!("Expected Array"),
        }
    }

    #[test]
    fn test_cluster_disabled() {
        let cluster = Arc::new(ClusterState::new(false));

        let result = cluster_info(&cluster);
        match result {
            RespValue::Error(msg) => {
                assert!(msg.contains("cluster support disabled"));
            }
            _ => panic!("Expected Error"),
        }
    }
}
