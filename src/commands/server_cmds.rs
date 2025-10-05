// Server commands (PING, ECHO, SELECT, etc.)

use crate::config::Config;
use crate::persistence::{aof::AofManager, RdbSerializer};
use crate::protocol::RespValue;
use crate::storage::db::Database;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

pub async fn ping(args: Vec<Vec<u8>>) -> RespValue {
    if args.is_empty() {
        RespValue::SimpleString("PONG".to_string())
    } else if args.len() == 1 {
        RespValue::BulkString(Some(args[0].clone()))
    } else {
        RespValue::Error("ERR wrong number of arguments for 'ping' command".to_string())
    }
}

pub async fn echo(args: Vec<Vec<u8>>) -> RespValue {
    if args.len() != 1 {
        return RespValue::Error("ERR wrong number of arguments for 'echo' command".to_string());
    }
    RespValue::BulkString(Some(args[0].clone()))
}

pub async fn select(db_index: &mut usize, args: Vec<Vec<u8>>) -> RespValue {
    if args.len() != 1 {
        return RespValue::Error("ERR wrong number of arguments for 'select' command".to_string());
    }

    let index_str = match std::str::from_utf8(&args[0]) {
        Ok(s) => s,
        Err(_) => return RespValue::Error("ERR invalid DB index".to_string()),
    };

    let index = match index_str.parse::<usize>() {
        Ok(i) => i,
        Err(_) => return RespValue::Error("ERR invalid DB index".to_string()),
    };

    if index >= 16 {
        return RespValue::Error("ERR DB index is out of range".to_string());
    }

    *db_index = index;
    RespValue::SimpleString("OK".to_string())
}

pub async fn flushdb(db: &Arc<Database>, db_index: usize) -> RespValue {
    db.flush_db(db_index).await;
    RespValue::SimpleString("OK".to_string())
}

pub async fn flushall(db: &Arc<Database>) -> RespValue {
    db.flush_all().await;
    RespValue::SimpleString("OK".to_string())
}

pub async fn dbsize(db: &Arc<Database>, db_index: usize) -> RespValue {
    let size = db.db_size(db_index).await;
    RespValue::Integer(size as i64)
}

pub async fn keys(db: &Arc<Database>, db_index: usize, args: Vec<Vec<u8>>) -> RespValue {
    if args.len() != 1 {
        return RespValue::Error("ERR wrong number of arguments for 'keys' command".to_string());
    }

    let pattern = match std::str::from_utf8(&args[0]) {
        Ok(s) => s,
        Err(_) => return RespValue::Error("ERR invalid pattern".to_string()),
    };

    let keys = db.keys(db_index, pattern).await;
    let resp_keys: Vec<RespValue> = keys
        .into_iter()
        .map(|k| RespValue::BulkString(Some(k.into_bytes())))
        .collect();

    RespValue::Array(Some(resp_keys))
}

/// SAVE - Synchronously save the database to disk
pub async fn save(db: &Arc<Database>) -> RespValue {
    let result = RdbSerializer::save(db, "dump.rdb").await;
    match result {
        Ok(_) => RespValue::SimpleString("OK".to_string()),
        Err(e) => RespValue::Error(format!("ERR save failed: {}", e)),
    }
}

/// BGSAVE - Asynchronously save the database to disk
pub async fn bgsave(db: &Arc<Database>) -> RespValue {
    let db_clone = Arc::clone(db);

    // Spawn background task
    tokio::spawn(async move {
        let _ = RdbSerializer::save(&db_clone, "dump.rdb").await;
    });

    RespValue::SimpleString("Background saving started".to_string())
}

/// BGREWRITEAOF - Asynchronously rewrite the AOF file
pub async fn bgrewriteaof(db: &Arc<Database>, aof: &Arc<AofManager>) -> RespValue {
    if !aof.is_enabled() {
        return RespValue::Error("ERR AOF is not enabled".to_string());
    }

    let db_clone = Arc::clone(db);
    let aof_clone = Arc::clone(aof);

    // Spawn background task
    tokio::spawn(async move {
        let temp_path = "temp-rewriteaof.aof";
        let final_path = "appendonly.aof";

        match aof_clone.rewrite(&db_clone, temp_path).await {
            Ok(_) => {
                // Replace old AOF with new one
                if let Err(e) = tokio::fs::rename(temp_path, final_path).await {
                    tracing::error!("Failed to replace AOF file: {}", e);
                } else {
                    tracing::info!("AOF rewrite completed successfully");
                }
            }
            Err(e) => {
                tracing::error!("Failed to rewrite AOF: {}", e);
            }
        }
    });

    RespValue::SimpleString("Background append only file rewriting started".to_string())
}

/// CONFIG GET - Get configuration parameter(s)
pub async fn config_get(config: &Arc<Config>, args: Vec<Vec<u8>>) -> RespValue {
    if args.is_empty() {
        return RespValue::Error("ERR wrong number of arguments for 'config|get' command".to_string());
    }

    let pattern = match std::str::from_utf8(&args[0]) {
        Ok(s) => s,
        Err(_) => return RespValue::Error("ERR invalid parameter".to_string()),
    };

    // Support * wildcard to get all configs
    if pattern == "*" {
        let all_settings = config.get_all();
        let mut result = Vec::new();
        for (key, value) in all_settings {
            result.push(RespValue::BulkString(Some(key.into_bytes())));
            result.push(RespValue::BulkString(Some(value.into_bytes())));
        }
        return RespValue::Array(Some(result));
    }

    // Get specific parameter
    match config.get(pattern) {
        Some(value) => {
            RespValue::Array(Some(vec![
                RespValue::BulkString(Some(pattern.as_bytes().to_vec())),
                RespValue::BulkString(Some(value.into_bytes())),
            ]))
        }
        None => RespValue::Array(Some(vec![])),
    }
}

/// CONFIG SET - Set configuration parameter
pub async fn config_set(config: &Arc<Config>, args: Vec<Vec<u8>>) -> RespValue {
    if args.len() != 2 {
        return RespValue::Error("ERR wrong number of arguments for 'config|set' command".to_string());
    }

    let key = match std::str::from_utf8(&args[0]) {
        Ok(s) => s.to_string(),
        Err(_) => return RespValue::Error("ERR invalid parameter".to_string()),
    };

    let value = match std::str::from_utf8(&args[1]) {
        Ok(s) => s.to_string(),
        Err(_) => return RespValue::Error("ERR invalid value".to_string()),
    };

    if config.set(key.clone(), value) {
        RespValue::SimpleString("OK".to_string())
    } else {
        RespValue::Error(format!("ERR Unsupported CONFIG parameter: {}", key))
    }
}

/// TIME - Return the current server time
pub async fn time() -> RespValue {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap();

    let seconds = now.as_secs();
    let microseconds = now.subsec_micros();

    RespValue::Array(Some(vec![
        RespValue::BulkString(Some(seconds.to_string().into_bytes())),
        RespValue::BulkString(Some(microseconds.to_string().into_bytes())),
    ]))
}

/// LASTSAVE - Get UNIX timestamp of last successful save
pub async fn lastsave() -> RespValue {
    // For now, return current time - in a real implementation,
    // this would track the actual last save timestamp
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap();

    RespValue::Integer(now.as_secs() as i64)
}

/// TYPE - Determine the type stored at key
pub async fn key_type(db: &Arc<Database>, db_index: usize, args: Vec<Vec<u8>>) -> RespValue {
    if args.len() != 1 {
        return RespValue::Error("ERR wrong number of arguments for 'type' command".to_string());
    }

    let key = match std::str::from_utf8(&args[0]) {
        Ok(s) => s,
        Err(_) => return RespValue::Error("ERR invalid key".to_string()),
    };

    let db_instance = match db.get_db(db_index) {
        Some(db) => db,
        None => return RespValue::Error("ERR invalid database index".to_string()),
    };

    match db_instance.get(key) {
        Some(value) => {
            let type_str = value.type_name();
            RespValue::SimpleString(type_str.to_string())
        }
        None => RespValue::SimpleString("none".to_string()),
    }
}

/// RANDOMKEY - Return a random key from the currently selected database
pub async fn randomkey(db: &Arc<Database>, db_index: usize) -> RespValue {
    let keys = db.keys(db_index, "*").await;

    if keys.is_empty() {
        return RespValue::Null;
    }

    // Simple random selection using system time as seed
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap();
    let index = (now.as_nanos() % keys.len() as u128) as usize;

    RespValue::BulkString(Some(keys[index].clone().into_bytes()))
}

/// SHUTDOWN - Synchronously save the dataset to disk and shutdown the server
pub async fn shutdown(db: &Arc<Database>) -> RespValue {
    // Save the database before shutdown
    let _ = RdbSerializer::save(db, "dump.rdb").await;

    // In a real implementation, this would trigger graceful shutdown
    // For now, just return OK
    RespValue::SimpleString("OK".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_ping() {
        let result = ping(vec![]).await;
        assert_eq!(result, RespValue::SimpleString("PONG".to_string()));

        let result = ping(vec![b"hello".to_vec()]).await;
        assert_eq!(result, RespValue::BulkString(Some(b"hello".to_vec())));
    }

    #[tokio::test]
    async fn test_echo() {
        let result = echo(vec![b"test".to_vec()]).await;
        assert_eq!(result, RespValue::BulkString(Some(b"test".to_vec())));
    }

    #[tokio::test]
    async fn test_select() {
        let mut db_index = 0;
        let result = select(&mut db_index, vec![b"5".to_vec()]).await;
        assert_eq!(result, RespValue::SimpleString("OK".to_string()));
        assert_eq!(db_index, 5);

        let result = select(&mut db_index, vec![b"20".to_vec()]).await;
        assert!(matches!(result, RespValue::Error(_)));
    }
}
