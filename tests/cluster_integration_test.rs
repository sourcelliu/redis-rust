// Cluster Integration Test
//
// Verifies that cluster functionality is integrated into the server

use redis_rust::cluster::ClusterState;
use redis_rust::server::ServerConfig;
use std::sync::Arc;

#[test]
fn test_cluster_state_initialization() {
    // Test that ClusterState can be created in enabled and disabled modes
    let cluster_enabled = Arc::new(ClusterState::new(true));
    assert!(cluster_enabled.enabled);

    let cluster_disabled = Arc::new(ClusterState::new(false));
    assert!(!cluster_disabled.enabled);
}

#[test]
fn test_server_config_cluster_options() {
    // Test that ServerConfig has cluster configuration options
    let config = ServerConfig::default();
    assert!(!config.cluster_enabled, "Cluster should be disabled by default");
    assert_eq!(config.cluster_config_file, "nodes.conf");

    // Test builder methods
    let config_with_cluster = ServerConfig::default()
        .with_cluster_enabled(true)
        .with_cluster_config_file("custom.conf".to_string());

    assert!(config_with_cluster.cluster_enabled);
    assert_eq!(config_with_cluster.cluster_config_file, "custom.conf");
}

#[test]
fn test_cluster_state_operations() {
    let cluster = Arc::new(ClusterState::new(true));

    // Test slot assignment
    cluster.add_slot(100);
    assert!(cluster.owns_slot(100));
    assert!(!cluster.owns_slot(101));

    // Test node ID generation
    let node_id = cluster.my_id.clone();
    assert_eq!(node_id.len(), 40, "Node ID should be 40 characters");

    // Verify it's hexadecimal
    assert!(node_id.chars().all(|c| c.is_ascii_hexdigit()));
}

#[test]
fn test_cluster_disabled_mode() {
    let cluster = Arc::new(ClusterState::new(false));

    // When cluster is disabled, all slots should be considered "owned"
    // or cluster operations should be no-ops
    assert!(!cluster.enabled);
}

#[test]
fn test_cluster_with_migration() {
    use redis_rust::cluster::MigrationManager;

    let cluster = Arc::new(ClusterState::new(true));
    let migration = Arc::new(MigrationManager::new());

    // Assign a slot
    cluster.add_slot(500);

    // Test migration state
    migration.set_migrating(500, "target_node".to_string());
    assert!(migration.is_migrating(500));

    migration.set_stable(500);
    assert!(!migration.is_migrating(500));
}

#[test]
fn test_cluster_configuration_persistence_format() {
    use redis_rust::cluster::{save_cluster_config, load_cluster_config};
    use redis_rust::cluster::node::ClusterNode;
    use tempfile::NamedTempFile;

    let cluster = Arc::new(ClusterState::new(true));

    // Add a node with some slots
    let mut node = ClusterNode::new_master(
        "test_node_12345".to_string(),
        Some("127.0.0.1:7000".parse().unwrap())
    );
    node.add_slot(0);
    node.add_slot(1);
    node.add_slot(2);
    cluster.add_node(node);

    // Save to temp file
    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path().to_str().unwrap();

    let save_result = save_cluster_config(&cluster, 42, path);
    assert!(save_result.is_ok(), "Config save should succeed");

    // Load from temp file
    let cluster2 = Arc::new(ClusterState::new(true));
    let load_result = load_cluster_config(&cluster2, path);
    assert!(load_result.is_ok(), "Config load should succeed");

    let epoch = load_result.unwrap();
    assert_eq!(epoch, 42, "Epoch should match");

    // Verify node was loaded
    let loaded_node = cluster2.get_node("test_node_12345");
    assert!(loaded_node.is_some(), "Node should be loaded");
    assert!(loaded_node.unwrap().owns_slot(0), "Slot 0 should be owned");
}
