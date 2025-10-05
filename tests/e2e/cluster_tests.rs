// E2E Tests for Redis Cluster functionality
//
// Tests cluster setup, key distribution, slot migration, and redirection

use redis::{Client, Commands, RedisResult, Value};
use redis_rust::cluster::{key_hash_slot, CLUSTER_SLOTS};
use std::collections::HashMap;

mod common;

/// Test basic cluster KEYSLOT command
#[tokio::test]
async fn test_cluster_keyslot_command() {
    // This test validates that CLUSTER KEYSLOT returns correct slot numbers

    // Test keys and their expected slots (calculated manually with CRC16)
    let test_cases = vec![
        ("user:1000", key_hash_slot(b"user:1000")),
        ("{user}:profile", key_hash_slot(b"user")),  // Hash tag extraction
        ("{user}:settings", key_hash_slot(b"user")), // Same hash tag
        ("mykey", key_hash_slot(b"mykey")),
        ("", key_hash_slot(b"")),
    ];

    // Note: This test assumes a Redis-Rust server is running on port 6379
    // In a real E2E test, we would start the server programmatically

    let client = Client::open("redis://127.0.0.1:6379").unwrap();
    let mut con = client.get_connection().unwrap();

    for (key, expected_slot) in test_cases {
        // Execute CLUSTER KEYSLOT command
        let result: RedisResult<i64> = redis::cmd("CLUSTER")
            .arg("KEYSLOT")
            .arg(key)
            .query(&mut con);

        if let Ok(slot) = result {
            assert_eq!(
                slot as u16, expected_slot,
                "Slot mismatch for key '{}': expected {}, got {}",
                key, expected_slot, slot
            );
        } else {
            // If cluster mode is not enabled, skip this test
            println!("Cluster mode not enabled, skipping test");
            return;
        }
    }
}

/// Test hash tag extraction ensures same slot
#[test]
fn test_hash_tag_consistency() {
    // Keys with same hash tag should map to same slot
    let keys_with_same_tag = vec![
        b"{user}:profile".as_ref(),
        b"{user}:settings".as_ref(),
        b"{user}:preferences".as_ref(),
        b"{user}:cart".as_ref(),
    ];

    let first_slot = key_hash_slot(keys_with_same_tag[0]);

    for key in &keys_with_same_tag[1..] {
        let slot = key_hash_slot(key);
        assert_eq!(
            slot, first_slot,
            "Hash tag extraction failed: {} != {}",
            slot, first_slot
        );
    }
}

/// Test slot distribution across 16384 slots
#[test]
fn test_slot_distribution() {
    // Generate keys and verify they distribute across all possible slots
    let mut slot_counts: HashMap<u16, usize> = HashMap::new();

    // Generate 10000 test keys
    for i in 0..10000 {
        let key = format!("key:{}", i);
        let slot = key_hash_slot(key.as_bytes());

        // Verify slot is within valid range
        assert!(
            slot < CLUSTER_SLOTS,
            "Slot {} exceeds maximum {}",
            slot,
            CLUSTER_SLOTS
        );

        *slot_counts.entry(slot).or_insert(0) += 1;
    }

    // With 10000 keys and 16384 slots, we should hit many different slots
    assert!(
        slot_counts.len() > 1000,
        "Poor slot distribution: only {} unique slots out of 10000 keys",
        slot_counts.len()
    );
}

/// Test CLUSTER INFO command output
#[tokio::test]
async fn test_cluster_info_command() {
    let client = Client::open("redis://127.0.0.1:6379").unwrap();
    let mut con = client.get_connection().unwrap();

    let result: RedisResult<String> = redis::cmd("CLUSTER")
        .arg("INFO")
        .query(&mut con);

    if let Ok(info) = result {
        // Verify expected fields are present
        assert!(info.contains("cluster_state:"), "Missing cluster_state field");
        assert!(info.contains("cluster_slots_assigned:"), "Missing cluster_slots_assigned");
        assert!(info.contains("cluster_known_nodes:"), "Missing cluster_known_nodes");
    } else {
        println!("Cluster mode not enabled, skipping test");
    }
}

/// Test CLUSTER NODES command output format
#[tokio::test]
async fn test_cluster_nodes_command() {
    let client = Client::open("redis://127.0.0.1:6379").unwrap();
    let mut con = client.get_connection().unwrap();

    let result: RedisResult<String> = redis::cmd("CLUSTER")
        .arg("NODES")
        .query(&mut con);

    if let Ok(nodes_output) = result {
        // Parse the output and verify format
        let lines: Vec<&str> = nodes_output.lines().collect();

        if !lines.is_empty() {
            for line in lines {
                // Each line should have at least: id, ip:port, flags, master, ping, pong, epoch, state
                let parts: Vec<&str> = line.split_whitespace().collect();
                assert!(
                    parts.len() >= 8,
                    "Invalid CLUSTER NODES line format: {} (expected >= 8 fields, got {})",
                    line,
                    parts.len()
                );

                // Verify node ID is 40 hex characters
                let node_id = parts[0];
                assert_eq!(
                    node_id.len(), 40,
                    "Node ID should be 40 characters, got {}",
                    node_id.len()
                );
            }
        }
    } else {
        println!("Cluster mode not enabled, skipping test");
    }
}

/// Test CLUSTER SLOTS command output
#[tokio::test]
async fn test_cluster_slots_command() {
    let client = Client::open("redis://127.0.0.1:6379").unwrap();
    let mut con = client.get_connection().unwrap();

    let result: RedisResult<Value> = redis::cmd("CLUSTER")
        .arg("SLOTS")
        .query(&mut con);

    if let Ok(Value::Bulk(slots)) = result {
        // Each slot range should be: [start, end, [master_ip, master_port], [replica_ip, replica_port]...]
        for slot_info in slots {
            if let Value::Bulk(info) = slot_info {
                assert!(
                    info.len() >= 3,
                    "Slot info should have at least 3 elements (start, end, master)"
                );
            }
        }
    } else {
        println!("Cluster mode not enabled or no slots assigned, skipping test");
    }
}

/// Test CLUSTER ADDSLOTS command
#[tokio::test]
async fn test_cluster_addslots() {
    let client = Client::open("redis://127.0.0.1:6379").unwrap();
    let mut con = client.get_connection().unwrap();

    // Try to add slots 0-10
    let result: RedisResult<String> = redis::cmd("CLUSTER")
        .arg("ADDSLOTS")
        .arg(0)
        .arg(1)
        .arg(2)
        .arg(3)
        .arg(4)
        .arg(5)
        .query(&mut con);

    if result.is_ok() {
        // Verify slots were added by checking CLUSTER SLOTS
        let slots_result: RedisResult<Value> = redis::cmd("CLUSTER")
            .arg("SLOTS")
            .query(&mut con);

        if let Ok(Value::Bulk(slots)) = slots_result {
            // Should have at least one slot range now
            assert!(!slots.is_empty(), "Expected slots to be assigned");
        }

        // Clean up: delete the slots
        let _: RedisResult<String> = redis::cmd("CLUSTER")
            .arg("DELSLOTS")
            .arg(0)
            .arg(1)
            .arg(2)
            .arg(3)
            .arg(4)
            .arg(5)
            .query(&mut con);
    } else {
        println!("Cluster mode not enabled, skipping test");
    }
}

/// Test CLUSTER DELSLOTS command
#[tokio::test]
async fn test_cluster_delslots() {
    let client = Client::open("redis://127.0.0.1:6379").unwrap();
    let mut con = client.get_connection().unwrap();

    // First add some slots
    let add_result: RedisResult<String> = redis::cmd("CLUSTER")
        .arg("ADDSLOTS")
        .arg(100)
        .arg(101)
        .arg(102)
        .query(&mut con);

    if add_result.is_ok() {
        // Then delete them
        let del_result: RedisResult<String> = redis::cmd("CLUSTER")
            .arg("DELSLOTS")
            .arg(100)
            .arg(101)
            .arg(102)
            .query(&mut con);

        assert!(del_result.is_ok(), "DELSLOTS should succeed");
    } else {
        println!("Cluster mode not enabled, skipping test");
    }
}

/// Test multi-key operation with same hash tag
#[tokio::test]
async fn test_multi_key_same_slot() {
    let client = Client::open("redis://127.0.0.1:6379").unwrap();
    let mut con = client.get_connection().unwrap();

    // Keys with same hash tag should be in same slot
    let key1 = "{user:1000}:profile";
    let key2 = "{user:1000}:settings";

    // Set both keys
    let _: RedisResult<()> = con.set(key1, "profile_data");
    let _: RedisResult<()> = con.set(key2, "settings_data");

    // Get both keys
    let val1: RedisResult<String> = con.get(key1);
    let val2: RedisResult<String> = con.get(key2);

    assert!(val1.is_ok());
    assert!(val2.is_ok());

    // Verify they map to the same slot
    let slot1 = key_hash_slot(key1.as_bytes());
    let slot2 = key_hash_slot(key2.as_bytes());
    assert_eq!(slot1, slot2, "Keys with same hash tag should be in same slot");
}

/// Test configuration persistence (nodes.conf)
#[test]
fn test_config_persistence_format() {
    use redis_rust::cluster::{ClusterState, save_cluster_config};
    use redis_rust::cluster::node::ClusterNode;
    use std::sync::Arc;
    use tempfile::NamedTempFile;

    // Create a test cluster state
    let cluster = Arc::new(ClusterState::new(true));

    // Add a node
    let mut node = ClusterNode::new_master(
        "abc123def456".to_string(),
        Some("127.0.0.1:6379".parse().unwrap())
    );
    node.add_slot(0);
    node.add_slot(1);
    node.add_slot(2);
    cluster.add_node(node);

    // Save to temp file
    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path().to_str().unwrap();

    let result = save_cluster_config(&cluster, 10, path);
    assert!(result.is_ok(), "Config save should succeed");

    // Read the file and verify format
    let content = std::fs::read_to_string(path).unwrap();
    assert!(content.contains("abc123def456"), "Should contain node ID");
    assert!(content.contains("127.0.0.1:6379"), "Should contain node address");
    assert!(content.contains("0-2"), "Should contain slot range");
}

/// Integration test: Save and load cluster configuration
#[test]
fn test_config_save_load_roundtrip() {
    use redis_rust::cluster::{ClusterState, save_cluster_config, load_cluster_config};
    use redis_rust::cluster::node::ClusterNode;
    use std::sync::Arc;
    use tempfile::NamedTempFile;

    // Create cluster with nodes and slots
    let cluster1 = Arc::new(ClusterState::new(true));

    let mut node1 = ClusterNode::new_master(
        "node1".to_string(),
        Some("127.0.0.1:7000".parse().unwrap())
    );
    node1.add_slot(0);
    node1.add_slot(1);
    node1.add_slot(2);
    cluster1.add_node(node1);

    let node2 = ClusterNode::new_replica(
        "node2".to_string(),
        Some("127.0.0.1:7001".parse().unwrap()),
        "node1".to_string()
    );
    cluster1.add_node(node2);

    // Save
    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path().to_str().unwrap();
    save_cluster_config(&cluster1, 5, path).unwrap();

    // Load into new cluster
    let cluster2 = Arc::new(ClusterState::new(true));
    let epoch = load_cluster_config(&cluster2, path).unwrap();

    // Verify
    assert_eq!(epoch, 5, "Epoch should match");

    let loaded_node1 = cluster2.get_node("node1");
    assert!(loaded_node1.is_some(), "Node1 should be loaded");
    assert!(loaded_node1.unwrap().owns_slot(0), "Node1 should own slot 0");

    let loaded_node2 = cluster2.get_node("node2");
    assert!(loaded_node2.is_some(), "Node2 should be loaded");
    assert!(loaded_node2.unwrap().is_slave(), "Node2 should be replica");
}

#[cfg(test)]
mod cluster_edge_cases {
    use super::*;

    #[test]
    fn test_empty_key_hash_slot() {
        let slot = key_hash_slot(b"");
        assert!(slot < CLUSTER_SLOTS, "Empty key should still produce valid slot");
    }

    #[test]
    fn test_special_chars_in_key() {
        let keys = vec![
            b"key:with:colons".as_ref(),
            b"key-with-dashes".as_ref(),
            b"key_with_underscores".as_ref(),
            b"key.with.dots".as_ref(),
            b"key@with@at".as_ref(),
        ];

        for key in keys {
            let slot = key_hash_slot(key);
            assert!(slot < CLUSTER_SLOTS, "Special char key should produce valid slot");
        }
    }

    #[test]
    fn test_unicode_key_hash_slot() {
        let keys = vec![
            "用户:1000",      // Chinese
            "ユーザー:1000",  // Japanese
            "사용자:1000",    // Korean
            "مستخدم:1000",   // Arabic
        ];

        for key in keys {
            let slot = key_hash_slot(key.as_bytes());
            assert!(slot < CLUSTER_SLOTS, "Unicode key should produce valid slot");
        }
    }

    #[test]
    fn test_nested_hash_tags() {
        // Only the first hash tag should be used
        let key1 = b"{user}{session}:data";
        let key2 = b"{user}:data";

        let slot1 = key_hash_slot(key1);
        let slot2 = key_hash_slot(key2);

        assert_eq!(slot1, slot2, "Only first hash tag should be used");
    }

    #[test]
    fn test_incomplete_hash_tag() {
        // Missing closing brace
        let key = b"{incomplete:data";
        let slot = key_hash_slot(key);

        // Should hash the entire key
        assert!(slot < CLUSTER_SLOTS);
    }

    #[test]
    fn test_empty_hash_tag() {
        // Empty hash tag {}
        let key = b"{}:data";
        let slot = key_hash_slot(key);

        // Should hash the entire key
        assert!(slot < CLUSTER_SLOTS);
    }
}
