// INFO command implementation

use crate::protocol::RespValue;
use crate::replication::ReplicationInfo;
use crate::storage::db::Database;
use std::sync::Arc;

/// Generate server info string
pub async fn info(
    db: &Arc<Database>,
    repl_info: &Arc<ReplicationInfo>,
    args: Vec<Vec<u8>>,
) -> RespValue {
    // Parse optional section argument
    let section = if args.is_empty() {
        "all".to_string()
    } else {
        match std::str::from_utf8(&args[0]) {
            Ok(s) => s.to_lowercase(),
            Err(_) => "all".to_string(),
        }
    };

    let mut info_lines = Vec::new();

    // Server section
    if section == "all" || section == "server" {
        info_lines.push("# Server".to_string());
        info_lines.push("redis_version:7.0.0-rust".to_string());
        info_lines.push("redis_mode:standalone".to_string());
        info_lines.push("os:".to_string() + std::env::consts::OS);
        info_lines.push("arch_bits:64".to_string());
        info_lines.push("multiplexing_api:tokio".to_string());
        info_lines.push(format!("process_id:{}", std::process::id()));
        info_lines.push("".to_string());
    }

    // Stats section
    if section == "all" || section == "stats" {
        info_lines.push("# Stats".to_string());
        info_lines.push("total_connections_received:0".to_string());
        info_lines.push("total_commands_processed:0".to_string());
        info_lines.push("instantaneous_ops_per_sec:0".to_string());
        info_lines.push("".to_string());
    }

    // Replication section
    if section == "all" || section == "replication" {
        info_lines.push("# Replication".to_string());

        if repl_info.is_master() {
            info_lines.push("role:master".to_string());
            let replicas = repl_info.replicas();
            info_lines.push(format!("connected_slaves:{}", replicas.len()));

            for (i, replica) in replicas.iter().enumerate() {
                info_lines.push(format!(
                    "slave{}:ip={},port={},state=online,offset={}",
                    i, replica.ip, replica.port, replica.offset
                ));
            }

            info_lines.push(format!("master_repl_offset:{}", repl_info.master_offset()));
            info_lines.push(format!("master_replid:{}", repl_info.replication_id()));
        } else {
            info_lines.push("role:slave".to_string());

            if let crate::replication::ReplicationRole::Replica { master_host, master_port, state } = repl_info.role() {
                info_lines.push(format!("master_host:{}", master_host));
                info_lines.push(format!("master_port:{}", master_port));
                info_lines.push(format!("master_link_status:{:?}", state).to_lowercase());
                info_lines.push(format!("slave_repl_offset:{}", repl_info.master_offset()));
            }
        }
        info_lines.push("".to_string());
    }

    // Keyspace section
    if section == "all" || section == "keyspace" {
        info_lines.push("# Keyspace".to_string());

        for db_index in 0..16 {
            let keys = db.keys(db_index, "*").await;
            if !keys.is_empty() {
                info_lines.push(format!("db{}:keys={}", db_index, keys.len()));
            }
        }
        info_lines.push("".to_string());
    }

    // Memory section
    if section == "all" || section == "memory" {
        info_lines.push("# Memory".to_string());
        info_lines.push("used_memory:0".to_string());
        info_lines.push("used_memory_human:0B".to_string());
        info_lines.push("used_memory_rss:0".to_string());
        info_lines.push("mem_fragmentation_ratio:1.0".to_string());
        info_lines.push("".to_string());
    }

    // CPU section
    if section == "all" || section == "cpu" {
        info_lines.push("# CPU".to_string());
        info_lines.push("used_cpu_sys:0.0".to_string());
        info_lines.push("used_cpu_user:0.0".to_string());
        info_lines.push("".to_string());
    }

    let info_str = info_lines.join("\r\n");
    RespValue::BulkString(Some(info_str.into_bytes()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_info_all() {
        let db = Arc::new(Database::new(16));
        let repl_info = Arc::new(ReplicationInfo::new());

        let result = info(&db, &repl_info, vec![]).await;

        match result {
            RespValue::BulkString(Some(data)) => {
                let info_str = String::from_utf8(data).unwrap();
                assert!(info_str.contains("# Server"));
                assert!(info_str.contains("# Replication"));
                assert!(info_str.contains("role:master"));
            }
            _ => panic!("Expected BulkString"),
        }
    }

    #[tokio::test]
    async fn test_info_replication() {
        let db = Arc::new(Database::new(16));
        let repl_info = Arc::new(ReplicationInfo::new());

        let result = info(&db, &repl_info, vec![b"replication".to_vec()]).await;

        match result {
            RespValue::BulkString(Some(data)) => {
                let info_str = String::from_utf8(data).unwrap();
                assert!(info_str.contains("# Replication"));
                assert!(info_str.contains("role:"));
                assert!(!info_str.contains("# Server"));
            }
            _ => panic!("Expected BulkString"),
        }
    }
}
