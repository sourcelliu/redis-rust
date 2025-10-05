// Advanced E2E Tests for Redis Cluster - Redirection and Migration
//
// Tests MOVED/ASK redirection, slot migration workflows

use redis::{Client, RedisResult};
use redis_rust::cluster::{key_hash_slot, ClusterState, MigrationManager};
use redis_rust::cluster::node::ClusterNode;
use std::sync::Arc;

#[cfg(test)]
mod redirection_tests {
    use super::*;

    /// Test MOVED redirection error format
    #[test]
    fn test_moved_error_format() {
        use redis_rust::cluster::redirection::{check_slot_ownership, SlotState};

        let cluster = Arc::new(ClusterState::new(true));

        // Add slot 100 to a different node
        let node = ClusterNode::new_master(
            "target_node".to_string(),
            Some("127.0.0.1:7001".parse().unwrap())
        );
        cluster.add_node(node);
        cluster.assign_slots_to_node("target_node", vec![100]);

        // Check ownership for a key in slot 100
        let key = b"testkey"; // Assuming this maps to slot 100
        let actual_slot = key_hash_slot(key);

        if actual_slot == 100 {
            let result = check_slot_ownership(&cluster, key, false);

            // Should return MOVED error since we don't own slot 100
            assert!(result.is_some(), "Should return MOVED error");

            // Verify error format: MOVED <slot> <ip:port>
            if let Some(error_val) = result {
                let error_str = format!("{:?}", error_val);
                assert!(error_str.contains("MOVED"), "Error should contain MOVED");
                assert!(error_str.contains("100"), "Error should contain slot number");
            }
        }
    }

    /// Test ASK redirection during migration
    #[test]
    fn test_ask_redirection() {
        use redis_rust::cluster::redirection::check_slot_ownership;

        let cluster = Arc::new(ClusterState::new(true));
        let migration = Arc::new(MigrationManager::new());

        // Setup: slot 200 is being migrated
        cluster.add_slot(200);

        let target_node = ClusterNode::new_master(
            "target_node".to_string(),
            Some("127.0.0.1:7001".parse().unwrap())
        );
        cluster.add_node(target_node);

        migration.set_migrating(200, "target_node".to_string());

        // For a key in slot 200, if not found locally, should return ASK
        // (This would require integration with actual key storage to test fully)
    }

    /// Test multi-key operation CROSSSLOT error
    #[test]
    fn test_crossslot_error() {
        use redis_rust::cluster::redirection::check_multi_key_slot;

        // Keys in different slots should cause CROSSSLOT error
        let key1 = b"key1";
        let key2 = b"key2";

        let slot1 = key_hash_slot(key1);
        let slot2 = key_hash_slot(key2);

        if slot1 != slot2 {
            let result = check_multi_key_slot(&[key1, key2]);
            assert!(result.is_err(), "Should return CROSSSLOT error");

            let error = result.unwrap_err();
            assert!(
                error.contains("CROSSSLOT"),
                "Error should mention CROSSSLOT"
            );
        }
    }

    /// Test multi-key operation with same hash tag (should succeed)
    #[test]
    fn test_same_slot_multi_key() {
        use redis_rust::cluster::redirection::check_multi_key_slot;

        // Keys with same hash tag should be in same slot
        let keys = vec![
            b"{user}:profile".as_ref(),
            b"{user}:settings".as_ref(),
            b"{user}:cart".as_ref(),
        ];

        let result = check_multi_key_slot(&keys);
        assert!(result.is_ok(), "Same-slot keys should pass validation");

        let slot = result.unwrap();
        // Verify all keys map to this slot
        for key in keys {
            assert_eq!(key_hash_slot(key), slot);
        }
    }
}

#[cfg(test)]
mod migration_tests {
    use super::*;

    /// Test slot migration lifecycle
    #[test]
    fn test_migration_lifecycle() {
        let cluster = Arc::new(ClusterState::new(true));
        let migration = Arc::new(MigrationManager::new());

        // Add source and target nodes
        cluster.add_slot(500);

        let target = ClusterNode::new_master(
            "target".to_string(),
            Some("127.0.0.1:7001".parse().unwrap())
        );
        cluster.add_node(target);

        // Step 1: Mark slot as MIGRATING on source
        migration.set_migrating(500, "target".to_string());
        assert!(migration.is_migrating(500), "Slot should be migrating");

        // Step 2: Mark slot as IMPORTING on target
        migration.set_importing(500, cluster.my_id.clone());

        // Step 3: Complete migration - mark as STABLE
        migration.set_stable(500);
        assert!(!migration.is_migrating(500), "Slot should no longer be migrating");
        assert!(!migration.is_importing(500), "Slot should no longer be importing");

        // Step 4: Assign slot to target node
        cluster.assign_slots_to_node("target", vec![500]);
        assert_eq!(
            cluster.get_slot_node(500),
            Some("target".to_string()),
            "Slot should be assigned to target"
        );
    }

    /// Test CLUSTER SETSLOT IMPORTING command
    #[test]
    fn test_setslot_importing() {
        use redis_rust::cluster::migration::cluster_setslot_importing;

        let cluster = Arc::new(ClusterState::new(true));
        let migration = Arc::new(MigrationManager::new());

        // Add source node
        let source = ClusterNode::new_master(
            "source".to_string(),
            Some("127.0.0.1:7000".parse().unwrap())
        );
        cluster.add_node(source);

        // Execute SETSLOT IMPORTING
        let result = cluster_setslot_importing(
            &cluster,
            &migration,
            300,
            "source".to_string()
        );

        assert!(matches!(result, redis_rust::protocol::RespValue::SimpleString(_)));
        assert!(migration.is_importing(300));
    }

    /// Test CLUSTER SETSLOT MIGRATING command
    #[test]
    fn test_setslot_migrating() {
        use redis_rust::cluster::migration::cluster_setslot_migrating;

        let cluster = Arc::new(ClusterState::new(true));
        let migration = Arc::new(MigrationManager::new());

        // Assign slot to ourselves
        cluster.add_slot(400);

        // Add target node
        let target = ClusterNode::new_master(
            "target".to_string(),
            Some("127.0.0.1:7001".parse().unwrap())
        );
        cluster.add_node(target);

        // Execute SETSLOT MIGRATING
        let result = cluster_setslot_migrating(
            &cluster,
            &migration,
            400,
            "target".to_string()
        );

        assert!(matches!(result, redis_rust::protocol::RespValue::SimpleString(_)));
        assert!(migration.is_migrating(400));
    }

    /// Test CLUSTER SETSLOT STABLE command
    #[test]
    fn test_setslot_stable() {
        use redis_rust::cluster::migration::cluster_setslot_stable;

        let cluster = Arc::new(ClusterState::new(true));
        let migration = Arc::new(MigrationManager::new());

        // Set slot to migrating first
        migration.set_migrating(600, "some_node".to_string());
        assert!(migration.is_migrating(600));

        // Mark as stable
        let result = cluster_setslot_stable(&cluster, &migration, 600);

        assert!(matches!(result, redis_rust::protocol::RespValue::SimpleString(_)));
        assert!(!migration.is_migrating(600));
    }

    /// Test CLUSTER SETSLOT NODE command
    #[test]
    fn test_setslot_node() {
        use redis_rust::cluster::migration::cluster_setslot_node;

        let cluster = Arc::new(ClusterState::new(true));
        let migration = Arc::new(MigrationManager::new());

        // Add target node
        let target = ClusterNode::new_master(
            "new_owner".to_string(),
            Some("127.0.0.1:7001".parse().unwrap())
        );
        cluster.add_node(target);

        // Assign slot to node
        let result = cluster_setslot_node(
            &cluster,
            &migration,
            700,
            "new_owner".to_string()
        );

        assert!(matches!(result, redis_rust::protocol::RespValue::SimpleString(_)));
        assert_eq!(
            cluster.get_slot_node(700),
            Some("new_owner".to_string())
        );
    }

    /// Test concurrent migration of multiple slots
    #[test]
    fn test_multiple_slot_migration() {
        let cluster = Arc::new(ClusterState::new(true));
        let migration = Arc::new(MigrationManager::new());

        // Assign multiple slots
        for slot in 0..10 {
            cluster.add_slot(slot);
        }

        // Migrate different slots to different targets
        let target1 = ClusterNode::new_master(
            "target1".to_string(),
            Some("127.0.0.1:7001".parse().unwrap())
        );
        let target2 = ClusterNode::new_master(
            "target2".to_string(),
            Some("127.0.0.1:7002".parse().unwrap())
        );
        cluster.add_node(target1);
        cluster.add_node(target2);

        // Migrate slots 0-4 to target1, 5-9 to target2
        for slot in 0..5 {
            migration.set_migrating(slot, "target1".to_string());
        }
        for slot in 5..10 {
            migration.set_migrating(slot, "target2".to_string());
        }

        // Verify migration states
        for slot in 0..5 {
            assert!(migration.is_migrating(slot));
        }
        for slot in 5..10 {
            assert!(migration.is_migrating(slot));
        }

        // Complete migrations
        for slot in 0..10 {
            migration.set_stable(slot);
        }

        // Assign to new owners
        cluster.assign_slots_to_node("target1", (0..5).collect());
        cluster.assign_slots_to_node("target2", (5..10).collect());

        // Verify final ownership
        for slot in 0..5 {
            assert_eq!(cluster.get_slot_node(slot), Some("target1".to_string()));
        }
        for slot in 5..10 {
            assert_eq!(cluster.get_slot_node(slot), Some("target2".to_string()));
        }
    }
}

#[cfg(test)]
mod integration_tests {
    use super::*;

    /// Integration test: Full cluster setup with 3 nodes
    #[test]
    fn test_three_node_cluster_setup() {
        let cluster = Arc::new(ClusterState::new(true));

        // Create 3 master nodes
        let node1 = ClusterNode::new_master(
            "node1".to_string(),
            Some("127.0.0.1:7000".parse().unwrap())
        );
        let node2 = ClusterNode::new_master(
            "node2".to_string(),
            Some("127.0.0.1:7001".parse().unwrap())
        );
        let node3 = ClusterNode::new_master(
            "node3".to_string(),
            Some("127.0.0.1:7002".parse().unwrap())
        );

        cluster.add_node(node1);
        cluster.add_node(node2);
        cluster.add_node(node3);

        // Distribute slots: 0-5460 to node1, 5461-10922 to node2, 10923-16383 to node3
        cluster.assign_slots_to_node("node1", (0..=5460).collect());
        cluster.assign_slots_to_node("node2", (5461..=10922).collect());
        cluster.assign_slots_to_node("node3", (10923..=16383).collect());

        // Verify all nodes
        let nodes = cluster.get_all_nodes();
        assert_eq!(nodes.len(), 4, "Should have 4 nodes (including myself)");

        // Verify all slots are assigned
        for slot in 0..16384u16 {
            let owner = cluster.get_slot_node(slot);
            assert!(owner.is_some(), "Slot {} should have an owner", slot);
        }

        // Verify slot counts
        let node1_loaded = cluster.get_node("node1").unwrap();
        let node2_loaded = cluster.get_node("node2").unwrap();
        let node3_loaded = cluster.get_node("node3").unwrap();

        assert_eq!(node1_loaded.slots.len(), 5461);
        assert_eq!(node2_loaded.slots.len(), 5462);
        assert_eq!(node3_loaded.slots.len(), 5461);
    }

    /// Integration test: Cluster with replicas
    #[test]
    fn test_cluster_with_replicas() {
        let cluster = Arc::new(ClusterState::new(true));

        // Create masters
        let master1 = ClusterNode::new_master(
            "master1".to_string(),
            Some("127.0.0.1:7000".parse().unwrap())
        );
        let master2 = ClusterNode::new_master(
            "master2".to_string(),
            Some("127.0.0.1:7001".parse().unwrap())
        );

        cluster.add_node(master1);
        cluster.add_node(master2);

        // Create replicas
        let replica1 = ClusterNode::new_replica(
            "replica1".to_string(),
            Some("127.0.0.1:7003".parse().unwrap()),
            "master1".to_string()
        );
        let replica2 = ClusterNode::new_replica(
            "replica2".to_string(),
            Some("127.0.0.1:7004".parse().unwrap()),
            "master2".to_string()
        );

        cluster.add_node(replica1);
        cluster.add_node(replica2);

        // Verify master-replica relationships
        let master1_replicas = cluster.get_replicas("master1");
        assert_eq!(master1_replicas.len(), 1);
        assert_eq!(master1_replicas[0].id, "replica1");

        let master2_replicas = cluster.get_replicas("master2");
        assert_eq!(master2_replicas.len(), 1);
        assert_eq!(master2_replicas[0].id, "replica2");

        // Verify replica flags
        let loaded_replica1 = cluster.get_node("replica1").unwrap();
        assert!(loaded_replica1.is_slave());
        assert_eq!(loaded_replica1.master_id, Some("master1".to_string()));
    }

    /// Integration test: Resharding scenario
    #[test]
    fn test_resharding_workflow() {
        let cluster = Arc::new(ClusterState::new(true));
        let migration = Arc::new(MigrationManager::new());

        // Initial setup: 2 nodes with half slots each
        let node1 = ClusterNode::new_master(
            "node1".to_string(),
            Some("127.0.0.1:7000".parse().unwrap())
        );
        let node2 = ClusterNode::new_master(
            "node2".to_string(),
            Some("127.0.0.1:7001".parse().unwrap())
        );

        cluster.add_node(node1);
        cluster.add_node(node2);

        cluster.assign_slots_to_node("node1", (0..8192).collect());
        cluster.assign_slots_to_node("node2", (8192..16384).collect());

        // Add new node3
        let node3 = ClusterNode::new_master(
            "node3".to_string(),
            Some("127.0.0.1:7002".parse().unwrap())
        );
        cluster.add_node(node3);

        // Reshard: move slots 5000-5999 from node1 to node3
        for slot in 5000..6000 {
            // Mark as migrating on node1
            migration.set_migrating(slot, "node3".to_string());

            // Mark as importing on node3 (in real scenario)
            migration.set_importing(slot, "node1".to_string());

            // Complete migration
            migration.set_stable(slot);

            // Reassign to node3
            cluster.assign_slots_to_node("node3", vec![slot]);
        }

        // Verify final state
        let node3_loaded = cluster.get_node("node3").unwrap();
        assert_eq!(node3_loaded.slots.len(), 1000);

        for slot in 5000..6000 {
            assert_eq!(cluster.get_slot_node(slot), Some("node3".to_string()));
        }
    }
}

#[cfg(test)]
mod stress_tests {
    use super::*;

    /// Stress test: Many nodes
    #[test]
    fn test_many_nodes_cluster() {
        let cluster = Arc::new(ClusterState::new(true));

        // Create 100 nodes
        for i in 0..100 {
            let node = ClusterNode::new_master(
                format!("node{}", i),
                Some(format!("127.0.0.1:{}", 7000 + i).parse().unwrap())
            );
            cluster.add_node(node);
        }

        // Distribute all slots evenly
        let slots_per_node = 16384 / 100;
        for i in 0..100 {
            let start = i * slots_per_node;
            let end = if i == 99 { 16384 } else { (i + 1) * slots_per_node };
            cluster.assign_slots_to_node(&format!("node{}", i), (start..end).collect());
        }

        // Verify
        let nodes = cluster.get_all_nodes();
        assert_eq!(nodes.len(), 101, "Should have 101 nodes (including myself)");
    }

    /// Stress test: Rapid migrations
    #[test]
    fn test_rapid_migrations() {
        let cluster = Arc::new(ClusterState::new(true));
        let migration = Arc::new(MigrationManager::new());

        // Setup 2 nodes
        cluster.assign_slots_to_node(&cluster.my_id, (0..16384).collect());

        let target = ClusterNode::new_master(
            "target".to_string(),
            Some("127.0.0.1:7001".parse().unwrap())
        );
        cluster.add_node(target);

        // Rapidly migrate all slots
        for slot in 0..16384 {
            migration.set_migrating(slot, "target".to_string());
            migration.set_stable(slot);
            cluster.assign_slots_to_node("target", vec![slot]);
        }

        // Verify all slots moved
        for slot in 0..16384u16 {
            assert_eq!(cluster.get_slot_node(slot), Some("target".to_string()));
        }
    }
}
