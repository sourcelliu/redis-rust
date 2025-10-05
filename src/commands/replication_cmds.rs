// Replication commands (REPLICAOF, ROLE, PSYNC, etc.)

use crate::protocol::RespValue;
use crate::replication::{ReplicationInfo, ReplicationRole, SyncHandler, CommandPropagator, ReplicaClient};
use crate::replication::backlog::ReplicationBacklog;
use crate::storage::db::Database;
use std::sync::Arc;
use tracing::info;

/// REPLICAOF command - Configure replication
pub async fn replicaof(
    repl_info: &Arc<ReplicationInfo>,
    backlog: &Arc<ReplicationBacklog>,
    db: &Arc<Database>,
    args: Vec<Vec<u8>>,
) -> RespValue {
    if args.len() != 2 {
        return RespValue::Error(
            "ERR wrong number of arguments for 'replicaof' command".to_string(),
        );
    }

    let host = match std::str::from_utf8(&args[0]) {
        Ok(s) => s,
        Err(_) => return RespValue::Error("ERR invalid host".to_string()),
    };

    let port_str = match std::str::from_utf8(&args[1]) {
        Ok(s) => s,
        Err(_) => return RespValue::Error("ERR invalid port".to_string()),
    };

    // Check for "NO ONE" to become master
    if host.to_uppercase() == "NO" && port_str.to_uppercase() == "ONE" {
        info!("Becoming master (REPLICAOF NO ONE)");
        repl_info.set_master();
        return RespValue::SimpleString("OK".to_string());
    }

    // Parse port
    let port: u16 = match port_str.parse() {
        Ok(p) => p,
        Err(_) => return RespValue::Error("ERR invalid port number".to_string()),
    };

    info!("Configuring as replica of {}:{}", host, port);
    repl_info.set_replica(host.to_string(), port);

    // Start replication connection in background
    let replica_client = ReplicaClient::new(
        host.to_string(),
        port,
        Arc::clone(db),
        Arc::clone(repl_info),
        Arc::clone(backlog),
    );

    tokio::spawn(async move {
        if let Err(e) = replica_client.start().await {
            tracing::error!("Replication connection failed: {}", e);
        }
    });

    RespValue::SimpleString("OK".to_string())
}

/// ROLE command - Get replication role information
pub async fn role(repl_info: &Arc<ReplicationInfo>) -> RespValue {
    match repl_info.role() {
        ReplicationRole::Master => {
            // Return: master, master_repl_offset, [replica info...]
            let offset = repl_info.master_offset();
            let replicas = repl_info.replicas();

            let mut replica_arrays = Vec::new();
            for replica in replicas {
                replica_arrays.push(RespValue::Array(Some(vec![
                    RespValue::BulkString(Some(replica.ip.into_bytes())),
                    RespValue::BulkString(Some(replica.port.to_string().into_bytes())),
                    RespValue::BulkString(Some(replica.offset.to_string().into_bytes())),
                ])));
            }

            RespValue::Array(Some(vec![
                RespValue::BulkString(Some(b"master".to_vec())),
                RespValue::Integer(offset as i64),
                RespValue::Array(Some(replica_arrays)),
            ]))
        }
        ReplicationRole::Replica { master_host, master_port, state } => {
            // Return: slave, master_ip, master_port, state, offset
            let offset = repl_info.master_offset();
            let state_str = format!("{:?}", state).to_lowercase();

            RespValue::Array(Some(vec![
                RespValue::BulkString(Some(b"slave".to_vec())),
                RespValue::BulkString(Some(master_host.into_bytes())),
                RespValue::Integer(master_port as i64),
                RespValue::BulkString(Some(state_str.into_bytes())),
                RespValue::Integer(offset as i64),
            ]))
        }
    }
}

/// PSYNC command - Partial resynchronization
pub async fn psync(
    repl_info: &Arc<ReplicationInfo>,
    backlog: &Arc<ReplicationBacklog>,
    args: Vec<Vec<u8>>,
) -> RespValue {
    // Only masters can handle PSYNC
    if !repl_info.is_master() {
        return RespValue::Error("ERR PSYNC can only be sent to a master".to_string());
    }

    // Parse arguments
    let (replica_repl_id, replica_offset) = match crate::replication::sync::parse_psync_args(&args) {
        Ok(parsed) => parsed,
        Err(e) => return RespValue::Error(format!("ERR {}", e)),
    };

    let master_repl_id = repl_info.replication_id();

    // Create sync handler
    let sync_handler = SyncHandler::new(Arc::clone(backlog));

    // Determine sync type
    let (needs_full_sync, offset, repl_id) = sync_handler.handle_psync(
        replica_repl_id,
        replica_offset,
        &master_repl_id,
    );

    if needs_full_sync {
        // Full resynchronization needed
        info!("PSYNC: Full resynchronization required");
        SyncHandler::generate_fullresync_response(&repl_id)
    } else {
        // Partial resynchronization possible
        info!("PSYNC: Partial resynchronization from offset {}", offset);
        SyncHandler::generate_continue_response(offset, &repl_id)
    }
}

/// REPLCONF command - Replication configuration
pub async fn replconf(
    propagator: &Arc<CommandPropagator>,
    args: Vec<Vec<u8>>,
) -> RespValue {
    if args.is_empty() {
        return RespValue::Error(
            "ERR wrong number of arguments for 'replconf' command".to_string(),
        );
    }

    let subcommand = match std::str::from_utf8(&args[0]) {
        Ok(s) => s.to_uppercase(),
        Err(_) => return RespValue::Error("ERR invalid subcommand".to_string()),
    };

    match subcommand.as_str() {
        "LISTENING-PORT" => {
            // Replica sends its listening port
            if args.len() != 2 {
                return RespValue::Error("ERR wrong number of arguments".to_string());
            }
            // Just acknowledge for now
            RespValue::SimpleString("OK".to_string())
        }
        "CAPA" => {
            // Replica sends its capabilities
            // Capabilities: eof, psync2, etc.
            RespValue::SimpleString("OK".to_string())
        }
        "GETACK" => {
            // Master requests acknowledgment
            // Replica should respond with REPLCONF ACK <offset>
            RespValue::SimpleString("OK".to_string())
        }
        "ACK" => {
            // Replica sends offset acknowledgment
            if args.len() != 2 {
                return RespValue::Error("ERR wrong number of arguments".to_string());
            }

            // Parse replica offset
            let offset_str = match std::str::from_utf8(&args[1]) {
                Ok(s) => s,
                Err(_) => return RespValue::Error("ERR invalid offset".to_string()),
            };

            let offset: u64 = match offset_str.parse() {
                Ok(o) => o,
                Err(_) => return RespValue::Error("ERR invalid offset".to_string()),
            };

            // TODO: Update replica offset in propagator
            // For now we don't have replica IP/port context here
            info!("Received ACK from replica with offset {}", offset);

            RespValue::SimpleString("OK".to_string())
        }
        _ => RespValue::Error(format!(
            "ERR Unknown REPLCONF subcommand '{}'",
            subcommand
        )),
    }
}

/// WAIT command - Wait for replicas to acknowledge offset
pub async fn wait(
    repl_info: &Arc<ReplicationInfo>,
    propagator: &Arc<CommandPropagator>,
    args: Vec<Vec<u8>>,
) -> RespValue {
    // Only masters can handle WAIT
    if !repl_info.is_master() {
        return RespValue::Integer(0);
    }

    if args.len() != 2 {
        return RespValue::Error(
            "ERR wrong number of arguments for 'wait' command".to_string(),
        );
    }

    // Parse min_replicas
    let min_replicas_str = match std::str::from_utf8(&args[0]) {
        Ok(s) => s,
        Err(_) => return RespValue::Error("ERR invalid numreplicas".to_string()),
    };

    let min_replicas: usize = match min_replicas_str.parse() {
        Ok(n) => n,
        Err(_) => return RespValue::Error("ERR invalid numreplicas".to_string()),
    };

    // Parse timeout
    let timeout_str = match std::str::from_utf8(&args[1]) {
        Ok(s) => s,
        Err(_) => return RespValue::Error("ERR invalid timeout".to_string()),
    };

    let timeout_ms: u64 = match timeout_str.parse() {
        Ok(n) => n,
        Err(_) => return RespValue::Error("ERR invalid timeout".to_string()),
    };

    // Get current master offset
    let target_offset = repl_info.master_offset();

    // Wait for replicas to acknowledge
    let acked = propagator
        .wait_for_replicas(min_replicas, timeout_ms, target_offset)
        .await;

    RespValue::Integer(acked as i64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_replicaof_no_one() {
        let repl_info = Arc::new(ReplicationInfo::new());
        let backlog = Arc::new(ReplicationBacklog::new());
        let db = Arc::new(Database::new(16));

        let result = replicaof(&repl_info, &backlog, &db, vec![b"NO".to_vec(), b"ONE".to_vec()]).await;

        assert_eq!(result, RespValue::SimpleString("OK".to_string()));
        assert!(repl_info.is_master());
    }

    #[tokio::test]
    async fn test_replicaof_set_master() {
        let repl_info = Arc::new(ReplicationInfo::new());
        let backlog = Arc::new(ReplicationBacklog::new());
        let db = Arc::new(Database::new(16));

        let result = replicaof(
            &repl_info,
            &backlog,
            &db,
            vec![b"127.0.0.1".to_vec(), b"6379".to_vec()],
        )
        .await;

        assert_eq!(result, RespValue::SimpleString("OK".to_string()));
        assert!(repl_info.is_replica());
    }

    #[tokio::test]
    async fn test_role_master() {
        let repl_info = Arc::new(ReplicationInfo::new());

        let result = role(&repl_info).await;

        match result {
            RespValue::Array(Some(arr)) => {
                assert_eq!(arr.len(), 3);
                assert_eq!(arr[0], RespValue::BulkString(Some(b"master".to_vec())));
            }
            _ => panic!("Expected Array"),
        }
    }

    #[tokio::test]
    async fn test_role_replica() {
        let repl_info = Arc::new(ReplicationInfo::new());
        repl_info.set_replica("127.0.0.1".to_string(), 6379);

        let result = role(&repl_info).await;

        match result {
            RespValue::Array(Some(arr)) => {
                assert_eq!(arr.len(), 5);
                assert_eq!(arr[0], RespValue::BulkString(Some(b"slave".to_vec())));
            }
            _ => panic!("Expected Array"),
        }
    }

    #[tokio::test]
    async fn test_replconf_listening_port() {
        let backlog = Arc::new(ReplicationBacklog::new());
        let propagator = Arc::new(CommandPropagator::new(backlog));
        let result = replconf(&propagator, vec![b"listening-port".to_vec(), b"6380".to_vec()]).await;
        assert_eq!(result, RespValue::SimpleString("OK".to_string()));
    }

    #[tokio::test]
    async fn test_replconf_capa() {
        let backlog = Arc::new(ReplicationBacklog::new());
        let propagator = Arc::new(CommandPropagator::new(backlog));
        let result = replconf(&propagator, vec![b"capa".to_vec(), b"eof".to_vec()]).await;
        assert_eq!(result, RespValue::SimpleString("OK".to_string()));
    }
}
