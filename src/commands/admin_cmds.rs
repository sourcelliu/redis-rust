// CLIENT command implementation

use crate::protocol::RespValue;
use crate::server::client_info::ClientRegistry;
use crate::server::slowlog::SlowLog;
use std::sync::Arc;

/// CLIENT command - Manage client connections
pub async fn client(
    client_registry: &Arc<ClientRegistry>,
    client_id: u64,
    args: Vec<Vec<u8>>,
) -> RespValue {
    if args.is_empty() {
        return RespValue::Error(
            "ERR wrong number of arguments for 'client' command".to_string(),
        );
    }

    let subcommand = match std::str::from_utf8(&args[0]) {
        Ok(s) => s.to_uppercase(),
        Err(_) => return RespValue::Error("ERR invalid subcommand".to_string()),
    };

    match subcommand.as_str() {
        "SETNAME" => {
            // Set client name
            if args.len() != 2 {
                return RespValue::Error("ERR wrong number of arguments".to_string());
            }
            let name = match std::str::from_utf8(&args[1]) {
                Ok(s) => s.to_string(),
                Err(_) => return RespValue::Error("ERR invalid name".to_string()),
            };
            client_registry.set_name(client_id, name);
            RespValue::SimpleString("OK".to_string())
        }
        "GETNAME" => {
            // Get client name
            match client_registry.get_name(client_id) {
                Some(name) => RespValue::BulkString(Some(name.into_bytes())),
                None => RespValue::BulkString(None),
            }
        }
        "LIST" => {
            // List all client connections
            let client_list = client_registry.list();
            RespValue::BulkString(Some(client_list.into_bytes()))
        }
        "PAUSE" => {
            // Pause client processing
            if args.len() != 2 {
                return RespValue::Error("ERR wrong number of arguments".to_string());
            }
            // TODO: Implement client pause
            RespValue::SimpleString("OK".to_string())
        }
        "UNPAUSE" => {
            // Unpause client processing
            // TODO: Implement client unpause
            RespValue::SimpleString("OK".to_string())
        }
        "KILL" => {
            // Kill client connection
            if args.len() < 2 {
                return RespValue::Error("ERR wrong number of arguments".to_string());
            }
            // TODO: Implement client kill
            RespValue::SimpleString("OK".to_string())
        }
        "ID" => {
            // Get client ID
            RespValue::Integer(client_id as i64)
        }
        "REPLY" => {
            // Control reply mode (ON/OFF/SKIP)
            if args.len() != 2 {
                return RespValue::Error("ERR wrong number of arguments".to_string());
            }
            // TODO: Implement reply control
            RespValue::SimpleString("OK".to_string())
        }
        _ => RespValue::Error(format!(
            "ERR Unknown subcommand '{}'. Try CLIENT HELP.",
            subcommand
        )),
    }
}

/// SLOWLOG command - Manage slow query log
pub async fn slowlog(slowlog: &Arc<SlowLog>, args: Vec<Vec<u8>>) -> RespValue {
    if args.is_empty() {
        return RespValue::Error(
            "ERR wrong number of arguments for 'slowlog' command".to_string(),
        );
    }

    let subcommand = match std::str::from_utf8(&args[0]) {
        Ok(s) => s.to_uppercase(),
        Err(_) => return RespValue::Error("ERR invalid subcommand".to_string()),
    };

    match subcommand.as_str() {
        "GET" => {
            // Get slow log entries
            let count = if args.len() > 1 {
                match std::str::from_utf8(&args[1]) {
                    Ok(s) => s.parse::<usize>().unwrap_or(10),
                    Err(_) => 10,
                }
            } else {
                10
            };

            let entries = slowlog.get(count);

            // Convert entries to RESP array
            let result: Vec<RespValue> = entries
                .into_iter()
                .map(|entry| {
                    // Each entry is an array with 6 elements:
                    // [id, timestamp, duration_micros, command, client_addr, client_name]
                    let command_arr: Vec<RespValue> = entry
                        .command
                        .into_iter()
                        .map(|s| RespValue::BulkString(Some(s.into_bytes())))
                        .collect();

                    RespValue::Array(Some(vec![
                        RespValue::Integer(entry.id as i64),
                        RespValue::Integer(entry.timestamp as i64),
                        RespValue::Integer(entry.duration_micros as i64),
                        RespValue::Array(Some(command_arr)),
                        RespValue::BulkString(Some(entry.client_addr.into_bytes())),
                        match entry.client_name {
                            Some(name) => RespValue::BulkString(Some(name.into_bytes())),
                            None => RespValue::BulkString(None),
                        },
                    ]))
                })
                .collect();

            RespValue::Array(Some(result))
        }
        "LEN" => {
            // Get slow log length
            RespValue::Integer(slowlog.len() as i64)
        }
        "RESET" => {
            // Reset slow log
            slowlog.reset();
            RespValue::SimpleString("OK".to_string())
        }
        _ => RespValue::Error(format!(
            "ERR Unknown SLOWLOG subcommand '{}'",
            subcommand
        )),
    }
}

/// COMMAND command - Get command information
pub async fn command(args: Vec<Vec<u8>>) -> RespValue {
    if args.is_empty() {
        // Return all commands info
        // Simplified response
        RespValue::Array(Some(vec![
            // Example command info
            RespValue::Array(Some(vec![
                RespValue::BulkString(Some(b"get".to_vec())),
                RespValue::Integer(2),
                RespValue::Array(Some(vec![
                    RespValue::BulkString(Some(b"readonly".to_vec())),
                    RespValue::BulkString(Some(b"fast".to_vec())),
                ])),
                RespValue::Integer(1),
                RespValue::Integer(1),
                RespValue::Integer(1),
            ])),
        ]))
    } else {
        let subcommand = match std::str::from_utf8(&args[0]) {
            Ok(s) => s.to_uppercase(),
            Err(_) => return RespValue::Error("ERR invalid subcommand".to_string()),
        };

        match subcommand.as_str() {
            "COUNT" => {
                // Return number of commands
                RespValue::Integer(87) // Current command count
            }
            "INFO" => {
                // Get info for specific commands
                if args.len() < 2 {
                    return RespValue::Error("ERR wrong number of arguments".to_string());
                }
                // TODO: Return actual command info
                RespValue::Array(Some(vec![]))
            }
            "DOCS" => {
                // Get command documentation
                RespValue::Array(Some(vec![]))
            }
            "GETKEYS" => {
                // Extract keys from command
                if args.len() < 2 {
                    return RespValue::Error("ERR wrong number of arguments".to_string());
                }
                // TODO: Implement key extraction
                RespValue::Array(Some(vec![]))
            }
            "LIST" => {
                // List all command names
                RespValue::Array(Some(vec![
                    RespValue::BulkString(Some(b"get".to_vec())),
                    RespValue::BulkString(Some(b"set".to_vec())),
                    RespValue::BulkString(Some(b"del".to_vec())),
                    // ... more commands
                ]))
            }
            _ => RespValue::Error(format!(
                "ERR Unknown COMMAND subcommand '{}'",
                subcommand
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::server::client_info::ClientRegistry;
    use crate::server::slowlog::SlowLog;

    #[tokio::test]
    async fn test_client_setname() {
        let registry = Arc::new(ClientRegistry::new());
        let client_id = registry.register("127.0.0.1:54321".to_string(), 8);

        let result = client(&registry, client_id, vec![b"SETNAME".to_vec(), b"myapp".to_vec()]).await;
        assert_eq!(result, RespValue::SimpleString("OK".to_string()));

        assert_eq!(registry.get_name(client_id), Some("myapp".to_string()));
    }

    #[tokio::test]
    async fn test_client_getname() {
        let registry = Arc::new(ClientRegistry::new());
        let client_id = registry.register("127.0.0.1:54321".to_string(), 8);

        let result = client(&registry, client_id, vec![b"GETNAME".to_vec()]).await;
        assert_eq!(result, RespValue::BulkString(None));
    }

    #[tokio::test]
    async fn test_client_id() {
        let registry = Arc::new(ClientRegistry::new());
        let client_id = registry.register("127.0.0.1:54321".to_string(), 8);

        let result = client(&registry, client_id, vec![b"ID".to_vec()]).await;
        match result {
            RespValue::Integer(id) => assert_eq!(id as u64, client_id),
            _ => panic!("Expected Integer"),
        }
    }

    #[tokio::test]
    async fn test_slowlog_get() {
        let slowlog = Arc::new(SlowLog::new());
        let result = slowlog(&slowlog, vec![b"GET".to_vec()]).await;
        match result {
            RespValue::Array(Some(arr)) => assert_eq!(arr.len(), 0),
            _ => panic!("Expected Array"),
        }
    }

    #[tokio::test]
    async fn test_command_count() {
        let result = command(vec![b"COUNT".to_vec()]).await;
        assert_eq!(result, RespValue::Integer(87));
    }
}
