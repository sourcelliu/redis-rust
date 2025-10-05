// Cluster configuration persistence (nodes.conf)

use crate::cluster::{ClusterState, node::ClusterNode};
use std::fs::{File, OpenOptions};
use std::io::{self, BufRead, BufReader, Write};
use std::path::Path;
use std::sync::Arc;

/// Configuration epoch for cluster state versioning
#[derive(Debug, Clone)]
pub struct ConfigEpoch {
    pub epoch: u64,
}

impl ConfigEpoch {
    pub fn new() -> Self {
        Self { epoch: 0 }
    }

    pub fn increment(&mut self) -> u64 {
        self.epoch += 1;
        self.epoch
    }

    pub fn get(&self) -> u64 {
        self.epoch
    }

    pub fn set(&mut self, epoch: u64) {
        self.epoch = epoch;
    }
}

impl Default for ConfigEpoch {
    fn default() -> Self {
        Self::new()
    }
}

/// Save cluster configuration to nodes.conf
///
/// Format (one line per node):
/// <id> <ip:port@cport> <flags> <master> <ping-sent> <pong-recv> <config-epoch> <link-state> <slot> <slot> ... <slot>
///
/// Example:
/// 07c37dfeb235213a872192d90877d0cd55635b91 127.0.0.1:6379@16379 myself,master - 0 0 1 connected 0-5460
pub fn save_cluster_config(
    cluster: &Arc<ClusterState>,
    config_epoch: u64,
    file_path: &str,
) -> io::Result<()> {
    if !cluster.enabled {
        return Ok(()); // Don't save if cluster mode disabled
    }

    let mut file = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(file_path)?;

    let nodes = cluster.get_all_nodes();

    for node in nodes {
        let line = format_node_config_line(&node, config_epoch);
        writeln!(file, "{}", line)?;
    }

    file.sync_all()?;
    Ok(())
}

/// Format a single node's configuration line
fn format_node_config_line(node: &ClusterNode, config_epoch: u64) -> String {
    // Node ID
    let mut line = node.id.clone();

    // IP:port@cport
    let addr = node.addr
        .map(|a| format!(" {}@{}", a, a.port() + 10000))
        .unwrap_or_else(|| " :0@0".to_string());
    line.push_str(&addr);

    // Flags
    let flags = node.flags_to_string();
    line.push_str(&format!(" {}", flags));

    // Master ID (or -)
    let master_id = node.master_id.as_deref().unwrap_or("-");
    line.push_str(&format!(" {}", master_id));

    // Ping sent, pong received (0 for now)
    line.push_str(" 0 0");

    // Config epoch
    line.push_str(&format!(" {}", config_epoch));

    // Link state (connected/disconnected)
    line.push_str(" connected");

    // Slots (as ranges)
    if !node.slots.is_empty() {
        let ranges = node.get_slot_ranges();
        for (start, end) in ranges {
            if start == end {
                line.push_str(&format!(" {}", start));
            } else {
                line.push_str(&format!(" {}-{}", start, end));
            }
        }
    }

    line
}

/// Load cluster configuration from nodes.conf
pub fn load_cluster_config(
    cluster: &Arc<ClusterState>,
    file_path: &str,
) -> io::Result<u64> {
    if !cluster.enabled {
        return Ok(0);
    }

    if !Path::new(file_path).exists() {
        return Ok(0); // No config file yet
    }

    let file = File::open(file_path)?;
    let reader = BufReader::new(file);

    let mut max_epoch = 0u64;

    for line in reader.lines() {
        let line = line?;
        if line.trim().is_empty() || line.starts_with('#') {
            continue;
        }

        if let Some((node, epoch)) = parse_node_config_line(&line) {
            cluster.add_node(node.clone());

            // Update slot assignments
            if !node.slots.is_empty() {
                let slots: Vec<u16> = node.slots.iter().copied().collect();
                cluster.assign_slots_to_node(&node.id, slots);
            }

            max_epoch = max_epoch.max(epoch);
        }
    }

    Ok(max_epoch)
}

/// Parse a single node configuration line
/// Returns (ClusterNode, config_epoch)
fn parse_node_config_line(line: &str) -> Option<(ClusterNode, u64)> {
    let parts: Vec<&str> = line.split_whitespace().collect();
    if parts.len() < 8 {
        return None;
    }

    // Parse node ID
    let id = parts[0].to_string();

    // Parse address (ip:port@cport)
    let addr = if parts[1].contains(':') {
        let addr_parts: Vec<&str> = parts[1].split('@').collect();
        if !addr_parts.is_empty() && addr_parts[0] != ":0" {
            addr_parts[0].parse().ok()
        } else {
            None
        }
    } else {
        None
    };

    // Parse flags
    let flags_str = parts[2];
    let is_master = flags_str.contains("master");
    let is_myself = flags_str.contains("myself");

    // Parse master ID
    let master_id = if parts[3] == "-" {
        None
    } else {
        Some(parts[3].to_string())
    };

    // Parse config epoch
    let config_epoch = parts[6].parse::<u64>().unwrap_or(0);

    // Create node
    let mut node = if is_master {
        ClusterNode::new_master(id, addr)
    } else if let Some(master) = master_id.clone() {
        ClusterNode::new_replica(id, addr, master)
    } else {
        ClusterNode::new_master(id, addr)
    };

    // Add myself flag if needed
    if is_myself {
        node.add_flag(crate::cluster::node::NodeFlags::Myself);
    }

    // Parse slots (parts[8] onwards)
    for i in 8..parts.len() {
        let slot_spec = parts[i];

        if slot_spec.contains('-') {
            // Range: "0-100"
            let range: Vec<&str> = slot_spec.split('-').collect();
            if range.len() == 2 {
                if let (Ok(start), Ok(end)) = (range[0].parse::<u16>(), range[1].parse::<u16>()) {
                    for slot in start..=end {
                        node.add_slot(slot);
                    }
                }
            }
        } else {
            // Single slot
            if let Ok(slot) = slot_spec.parse::<u16>() {
                node.add_slot(slot);
            }
        }
    }

    Some((node, config_epoch))
}

/// Auto-save cluster configuration with epoch increment
pub fn auto_save_cluster_config(
    cluster: &Arc<ClusterState>,
    config_epoch: &mut ConfigEpoch,
    file_path: &str,
) -> io::Result<()> {
    let epoch = config_epoch.increment();
    save_cluster_config(cluster, epoch, file_path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cluster::node::ClusterNode;
    use std::net::SocketAddr;
    use tempfile::NamedTempFile;

    #[test]
    fn test_config_epoch_creation() {
        let epoch = ConfigEpoch::new();
        assert_eq!(epoch.get(), 0);
    }

    #[test]
    fn test_config_epoch_increment() {
        let mut epoch = ConfigEpoch::new();
        assert_eq!(epoch.increment(), 1);
        assert_eq!(epoch.increment(), 2);
        assert_eq!(epoch.get(), 2);
    }

    #[test]
    fn test_config_epoch_set() {
        let mut epoch = ConfigEpoch::new();
        epoch.set(100);
        assert_eq!(epoch.get(), 100);
    }

    #[test]
    fn test_format_node_config_line() {
        let mut node = ClusterNode::new_master(
            "abc123".to_string(),
            Some("127.0.0.1:6379".parse().unwrap())
        );
        node.add_flag(crate::cluster::node::NodeFlags::Myself);
        node.add_slot(0);
        node.add_slot(1);
        node.add_slot(2);
        node.add_slot(100);

        let line = format_node_config_line(&node, 5);

        assert!(line.contains("abc123"));
        assert!(line.contains("127.0.0.1:6379"));
        assert!(line.contains("myself,master"));
        assert!(line.contains(" 5 ")); // config epoch
        assert!(line.contains("0-2"));
        assert!(line.contains("100"));
    }

    #[test]
    fn test_parse_node_config_line_master() {
        let line = "abc123 127.0.0.1:6379@16379 myself,master - 0 0 5 connected 0-100 200";

        let result = parse_node_config_line(line);
        assert!(result.is_some());

        let (node, epoch) = result.unwrap();
        assert_eq!(node.id, "abc123");
        assert_eq!(epoch, 5);
        assert!(node.is_master());
        assert!(node.owns_slot(0));
        assert!(node.owns_slot(50));
        assert!(node.owns_slot(100));
        assert!(node.owns_slot(200));
        assert!(!node.owns_slot(150));
    }

    #[test]
    fn test_parse_node_config_line_replica() {
        let line = "replica1 127.0.0.1:6380@16380 slave master123 0 0 3 connected";

        let result = parse_node_config_line(line);
        assert!(result.is_some());

        let (node, epoch) = result.unwrap();
        assert_eq!(node.id, "replica1");
        assert_eq!(epoch, 3);
        assert!(node.is_slave());
        assert_eq!(node.master_id, Some("master123".to_string()));
    }

    #[test]
    fn test_save_and_load_cluster_config() {
        let cluster = Arc::new(ClusterState::new(true));

        // Add some nodes and slots
        let mut node1 = ClusterNode::new_master(
            "node1".to_string(),
            Some("127.0.0.1:6379".parse().unwrap())
        );
        node1.add_slot(0);
        node1.add_slot(1);
        node1.add_slot(2);
        cluster.add_node(node1);

        let node2 = ClusterNode::new_replica(
            "node2".to_string(),
            Some("127.0.0.1:6380".parse().unwrap()),
            "node1".to_string()
        );
        cluster.add_node(node2);

        // Save to temp file
        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path().to_str().unwrap();

        let result = save_cluster_config(&cluster, 10, path);
        assert!(result.is_ok());

        // Load into new cluster
        let cluster2 = Arc::new(ClusterState::new(true));
        let epoch = load_cluster_config(&cluster2, path);

        assert!(epoch.is_ok());
        assert_eq!(epoch.unwrap(), 10);

        // Verify nodes loaded
        let loaded_node1 = cluster2.get_node("node1");
        assert!(loaded_node1.is_some());
        assert!(loaded_node1.unwrap().owns_slot(0));

        let loaded_node2 = cluster2.get_node("node2");
        assert!(loaded_node2.is_some());
        assert!(loaded_node2.unwrap().is_slave());
    }

    #[test]
    fn test_auto_save_cluster_config() {
        let cluster = Arc::new(ClusterState::new(true));
        let mut epoch = ConfigEpoch::new();

        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path().to_str().unwrap();

        // First save
        let result = auto_save_cluster_config(&cluster, &mut epoch, path);
        assert!(result.is_ok());
        assert_eq!(epoch.get(), 1);

        // Second save
        let result = auto_save_cluster_config(&cluster, &mut epoch, path);
        assert!(result.is_ok());
        assert_eq!(epoch.get(), 2);
    }

    #[test]
    fn test_load_nonexistent_config() {
        let cluster = Arc::new(ClusterState::new(true));
        let result = load_cluster_config(&cluster, "/nonexistent/path/nodes.conf");

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 0);
    }
}
