// Stream commands for Redis-Rust
// Stream is an append-only log data structure for message queues

use crate::protocol::RespValue;
use crate::storage::db::Database;
use crate::storage::types::{RedisValue, Stream, StreamEntry, StreamId};
use bytes::Bytes;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

/// Get current timestamp in milliseconds
fn current_timestamp_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64
}

/// Generate next stream ID
fn generate_stream_id(stream: &Stream, explicit_ts: Option<u64>) -> Result<StreamId, String> {
    let timestamp = explicit_ts.unwrap_or_else(current_timestamp_ms);

    // If timestamp is same as last, increment sequence
    // If timestamp is greater, reset sequence to 0
    // If timestamp is less, error
    if timestamp < stream.last_id.timestamp {
        return Err("ERR The ID specified in XADD is equal or smaller than the target stream top item".to_string());
    }

    let sequence = if timestamp == stream.last_id.timestamp {
        stream.last_id.sequence + 1
    } else {
        0
    };

    Ok(StreamId::new(timestamp, sequence))
}

/// XADD key ID field value [field value ...]
/// Add entry to stream
pub async fn xadd(db: &Arc<Database>, db_index: usize, args: Vec<Vec<u8>>) -> RespValue {
    if args.len() < 4 || (args.len() - 2) % 2 != 0 {
        return RespValue::Error("ERR wrong number of arguments for 'xadd' command".to_string());
    }

    let key = match std::str::from_utf8(&args[0]) {
        Ok(s) => s.to_string(),
        Err(_) => return RespValue::Error("ERR invalid key".to_string()),
    };

    let id_str = match std::str::from_utf8(&args[1]) {
        Ok(s) => s,
        Err(_) => return RespValue::Error("ERR invalid ID".to_string()),
    };

    let db_instance = match db.get_db(db_index) {
        Some(d) => d,
        None => return RespValue::Error("ERR invalid database".to_string()),
    };

    // Get or create stream
    let mut stream = match db_instance.get(&key) {
        Some(RedisValue::Stream(s)) => s,
        Some(_) => {
            return RespValue::Error(
                "WRONGTYPE Operation against a key holding the wrong kind of value".to_string(),
            )
        }
        None => Stream::new(),
    };

    // Parse or generate ID
    let id = if id_str == "*" {
        // Auto-generate ID
        match generate_stream_id(&stream, None) {
            Ok(id) => id,
            Err(e) => return RespValue::Error(e),
        }
    } else if id_str.ends_with("-*") {
        // Partial auto-generation: timestamp provided, generate sequence
        let ts_str = &id_str[..id_str.len() - 2];
        let timestamp: u64 = match ts_str.parse() {
            Ok(t) => t,
            Err(_) => return RespValue::Error("ERR invalid ID".to_string()),
        };
        match generate_stream_id(&stream, Some(timestamp)) {
            Ok(id) => id,
            Err(e) => return RespValue::Error(e),
        }
    } else {
        // Explicit ID
        match StreamId::from_string(id_str) {
            Some(id) => {
                // Validate ID is greater than last
                if id <= stream.last_id {
                    return RespValue::Error(
                        "ERR The ID specified in XADD is equal or smaller than the target stream top item".to_string(),
                    );
                }
                id
            }
            None => return RespValue::Error("ERR invalid ID".to_string()),
        }
    };

    // Parse field-value pairs
    let mut fields = HashMap::new();
    let mut i = 2;
    while i < args.len() {
        let field = Bytes::from(args[i].clone());
        let value = Bytes::from(args[i + 1].clone());
        fields.insert(field, value);
        i += 2;
    }

    // Create entry
    let entry = StreamEntry {
        id: id.clone(),
        fields,
    };

    // Add to stream
    stream.entries.insert(id.clone(), entry);
    stream.last_id = id.clone();

    // Store stream
    db_instance.set(key, RedisValue::Stream(stream));

    RespValue::BulkString(Some(id.to_string().into_bytes()))
}

/// XLEN key
/// Get stream length
pub async fn xlen(db: &Arc<Database>, db_index: usize, args: Vec<Vec<u8>>) -> RespValue {
    if args.len() != 1 {
        return RespValue::Error("ERR wrong number of arguments for 'xlen' command".to_string());
    }

    let key = match std::str::from_utf8(&args[0]) {
        Ok(s) => s,
        Err(_) => return RespValue::Error("ERR invalid key".to_string()),
    };

    let db_instance = match db.get_db(db_index) {
        Some(d) => d,
        None => return RespValue::Error("ERR invalid database".to_string()),
    };

    match db_instance.get(key) {
        Some(RedisValue::Stream(stream)) => RespValue::Integer(stream.len() as i64),
        Some(_) => RespValue::Error(
            "WRONGTYPE Operation against a key holding the wrong kind of value".to_string(),
        ),
        None => RespValue::Integer(0),
    }
}

/// XRANGE key start end [COUNT count]
/// Get entries in ID range
pub async fn xrange(db: &Arc<Database>, db_index: usize, args: Vec<Vec<u8>>) -> RespValue {
    if args.len() < 3 {
        return RespValue::Error("ERR wrong number of arguments for 'xrange' command".to_string());
    }

    let key = match std::str::from_utf8(&args[0]) {
        Ok(s) => s,
        Err(_) => return RespValue::Error("ERR invalid key".to_string()),
    };

    let start_str = match std::str::from_utf8(&args[1]) {
        Ok(s) => s,
        Err(_) => return RespValue::Error("ERR invalid start ID".to_string()),
    };

    let end_str = match std::str::from_utf8(&args[2]) {
        Ok(s) => s,
        Err(_) => return RespValue::Error("ERR invalid end ID".to_string()),
    };

    let db_instance = match db.get_db(db_index) {
        Some(d) => d,
        None => return RespValue::Error("ERR invalid database".to_string()),
    };

    let stream = match db_instance.get(key) {
        Some(RedisValue::Stream(s)) => s,
        Some(_) => {
            return RespValue::Error(
                "WRONGTYPE Operation against a key holding the wrong kind of value".to_string(),
            )
        }
        None => return RespValue::Array(Some(vec![])),
    };

    // Parse start ID
    let start_id = if start_str == "-" {
        StreamId::new(0, 0)
    } else {
        match StreamId::from_string(start_str) {
            Some(id) => id,
            None => return RespValue::Error("ERR invalid start ID".to_string()),
        }
    };

    // Parse end ID
    let end_id = if end_str == "+" {
        StreamId::new(u64::MAX, u64::MAX)
    } else {
        match StreamId::from_string(end_str) {
            Some(id) => id,
            None => return RespValue::Error("ERR invalid end ID".to_string()),
        }
    };

    // Collect entries in range
    let mut result = Vec::new();
    for (id, entry) in stream.entries.range(start_id..=end_id) {
        let mut entry_array = Vec::new();

        // Entry ID
        entry_array.push(RespValue::BulkString(Some(id.to_string().into_bytes())));

        // Field-value pairs
        let mut fields_array = Vec::new();
        for (field, value) in &entry.fields {
            fields_array.push(RespValue::BulkString(Some(field.to_vec())));
            fields_array.push(RespValue::BulkString(Some(value.to_vec())));
        }
        entry_array.push(RespValue::Array(Some(fields_array)));

        result.push(RespValue::Array(Some(entry_array)));
    }

    RespValue::Array(Some(result))
}

/// XDEL key ID [ID ...]
/// Delete entries from stream
pub async fn xdel(db: &Arc<Database>, db_index: usize, args: Vec<Vec<u8>>) -> RespValue {
    if args.len() < 2 {
        return RespValue::Error("ERR wrong number of arguments for 'xdel' command".to_string());
    }

    let key = match std::str::from_utf8(&args[0]) {
        Ok(s) => s.to_string(),
        Err(_) => return RespValue::Error("ERR invalid key".to_string()),
    };

    let db_instance = match db.get_db(db_index) {
        Some(d) => d,
        None => return RespValue::Error("ERR invalid database".to_string()),
    };

    let mut stream = match db_instance.get(&key) {
        Some(RedisValue::Stream(s)) => s,
        Some(_) => {
            return RespValue::Error(
                "WRONGTYPE Operation against a key holding the wrong kind of value".to_string(),
            )
        }
        None => return RespValue::Integer(0),
    };

    let mut deleted = 0;

    for id_bytes in &args[1..] {
        let id_str = match std::str::from_utf8(id_bytes) {
            Ok(s) => s,
            Err(_) => continue,
        };

        if let Some(id) = StreamId::from_string(id_str) {
            if stream.entries.remove(&id).is_some() {
                deleted += 1;
            }
        }
    }

    // Store back
    if stream.is_empty() {
        db_instance.delete(&key);
    } else {
        db_instance.set(key, RedisValue::Stream(stream));
    }

    RespValue::Integer(deleted)
}

/// XREAD [COUNT count] [BLOCK milliseconds] STREAMS key [key ...] ID [ID ...]
/// Read entries from streams
pub async fn xread(db: &Arc<Database>, db_index: usize, args: Vec<Vec<u8>>) -> RespValue {
    if args.is_empty() {
        return RespValue::Error("ERR wrong number of arguments for 'xread' command".to_string());
    }

    let mut count: Option<usize> = None;
    let mut block_ms: Option<u64> = None;
    let mut i = 0;

    // Parse options
    while i < args.len() {
        let arg = match std::str::from_utf8(&args[i]) {
            Ok(s) => s.to_uppercase(),
            Err(_) => break,
        };

        match arg.as_str() {
            "COUNT" => {
                i += 1;
                if i >= args.len() {
                    return RespValue::Error("ERR syntax error".to_string());
                }
                count = match std::str::from_utf8(&args[i]) {
                    Ok(s) => s.parse().ok(),
                    Err(_) => return RespValue::Error("ERR invalid count".to_string()),
                };
                i += 1;
            }
            "BLOCK" => {
                i += 1;
                if i >= args.len() {
                    return RespValue::Error("ERR syntax error".to_string());
                }
                block_ms = match std::str::from_utf8(&args[i]) {
                    Ok(s) => s.parse().ok(),
                    Err(_) => return RespValue::Error("ERR invalid block time".to_string()),
                };
                i += 1;
            }
            "STREAMS" => {
                i += 1;
                break;
            }
            _ => break,
        }
    }

    // Check for STREAMS keyword
    if i >= args.len() {
        return RespValue::Error("ERR syntax error".to_string());
    }

    // Parse stream keys and IDs
    let remaining = &args[i..];
    if remaining.len() % 2 != 0 {
        return RespValue::Error("ERR syntax error".to_string());
    }

    let num_streams = remaining.len() / 2;
    let keys = &remaining[..num_streams];
    let ids = &remaining[num_streams..];

    let db_instance = match db.get_db(db_index) {
        Some(d) => d,
        None => return RespValue::Error("ERR invalid database".to_string()),
    };

    // Read from streams (simplified - no actual blocking implementation)
    let mut result = Vec::new();

    for (key_bytes, id_bytes) in keys.iter().zip(ids.iter()) {
        let key = match std::str::from_utf8(key_bytes) {
            Ok(s) => s,
            Err(_) => continue,
        };

        let id_str = match std::str::from_utf8(id_bytes) {
            Ok(s) => s,
            Err(_) => continue,
        };

        let stream = match db_instance.get(key) {
            Some(RedisValue::Stream(s)) => s,
            _ => continue,
        };

        // Parse start ID
        let start_id = if id_str == "$" {
            // $ means read from latest
            stream.last_id.clone()
        } else {
            match StreamId::from_string(id_str) {
                Some(id) => id,
                None => continue,
            }
        };

        // Collect entries after start_id
        let mut entries = Vec::new();
        let mut collected = 0;

        for (id, entry) in stream.entries.range((std::ops::Bound::Excluded(&start_id), std::ops::Bound::Unbounded)) {
            if let Some(max) = count {
                if collected >= max {
                    break;
                }
            }

            let mut entry_array = Vec::new();
            entry_array.push(RespValue::BulkString(Some(id.to_string().into_bytes())));

            let mut fields_array = Vec::new();
            for (field, value) in &entry.fields {
                fields_array.push(RespValue::BulkString(Some(field.to_vec())));
                fields_array.push(RespValue::BulkString(Some(value.to_vec())));
            }
            entry_array.push(RespValue::Array(Some(fields_array)));

            entries.push(RespValue::Array(Some(entry_array)));
            collected += 1;
        }

        if !entries.is_empty() {
            result.push(RespValue::Array(Some(vec![
                RespValue::BulkString(Some(key.as_bytes().to_vec())),
                RespValue::Array(Some(entries)),
            ])));
        }
    }

    if result.is_empty() && block_ms.is_some() {
        // Simplified: return null for blocking timeout
        return RespValue::Null;
    }

    RespValue::Array(Some(result))
}

/// XREVRANGE key end start [COUNT count]
/// Query stream entries in reverse order
pub async fn xrevrange(db: &Arc<Database>, db_index: usize, args: Vec<Vec<u8>>) -> RespValue {
    if args.len() < 3 {
        return RespValue::Error(
            "ERR wrong number of arguments for 'xrevrange' command".to_string(),
        );
    }

    let key = match std::str::from_utf8(&args[0]) {
        Ok(s) => s.to_string(),
        Err(_) => return RespValue::Error("ERR invalid key".to_string()),
    };

    let end_str = match std::str::from_utf8(&args[1]) {
        Ok(s) => s,
        Err(_) => return RespValue::Error("ERR invalid end ID".to_string()),
    };

    let start_str = match std::str::from_utf8(&args[2]) {
        Ok(s) => s,
        Err(_) => return RespValue::Error("ERR invalid start ID".to_string()),
    };

    // Parse optional COUNT
    let count = if args.len() >= 5 {
        if let Ok(s) = std::str::from_utf8(&args[3]) {
            if s.to_uppercase() == "COUNT" {
                if let Ok(c_str) = std::str::from_utf8(&args[4]) {
                    c_str.parse::<usize>().ok()
                } else {
                    None
                }
            } else {
                None
            }
        } else {
            None
        }
    } else {
        None
    };

    let db_instance = match db.get_db(db_index) {
        Some(d) => d,
        None => return RespValue::Error("ERR invalid database".to_string()),
    };

    match db_instance.get(&key) {
        Some(RedisValue::Stream(stream)) => {
            // Collect entries in reverse order
            let mut entries: Vec<_> = stream
                .entries
                .iter()
                .rev() // Reverse iteration
                .filter(|(id, _)| {
                    let id_str = id.to_string();
                    let after_start = if start_str == "-" {
                        true
                    } else {
                        id_str.as_str() >= start_str
                    };
                    let before_end = if end_str == "+" {
                        true
                    } else {
                        id_str.as_str() <= end_str
                    };
                    after_start && before_end
                })
                .take(count.unwrap_or(usize::MAX))
                .collect();

            let mut result = Vec::new();
            for (id, entry) in entries {
                let mut entry_array = Vec::new();
                entry_array.push(RespValue::BulkString(Some(id.to_string().into_bytes())));

                let mut fields_array = Vec::new();
                for (field, value) in &entry.fields {
                    fields_array.push(RespValue::BulkString(Some(field.to_vec())));
                    fields_array.push(RespValue::BulkString(Some(value.to_vec())));
                }
                entry_array.push(RespValue::Array(Some(fields_array)));

                result.push(RespValue::Array(Some(entry_array)));
            }

            RespValue::Array(Some(result))
        }
        Some(_) => RespValue::Error(
            "WRONGTYPE Operation against a key holding the wrong kind of value".to_string(),
        ),
        None => RespValue::Array(Some(vec![])),
    }
}

/// XTRIM key MAXLEN [~] count
/// Trim stream to approximate maxlen
pub async fn xtrim(db: &Arc<Database>, db_index: usize, args: Vec<Vec<u8>>) -> RespValue {
    if args.len() < 3 {
        return RespValue::Error("ERR wrong number of arguments for 'xtrim' command".to_string());
    }

    let key = match std::str::from_utf8(&args[0]) {
        Ok(s) => s.to_string(),
        Err(_) => return RespValue::Error("ERR invalid key".to_string()),
    };

    // Check for MAXLEN strategy
    let strategy = match std::str::from_utf8(&args[1]) {
        Ok(s) => s.to_uppercase(),
        Err(_) => return RespValue::Error("ERR invalid strategy".to_string()),
    };

    if strategy != "MAXLEN" {
        return RespValue::Error("ERR unsupported trim strategy".to_string());
    }

    // Check for approximate trim (~)
    let mut arg_idx = 2;
    let _approximate = if args.len() > 3 {
        if let Ok(s) = std::str::from_utf8(&args[2]) {
            if s == "~" {
                arg_idx = 3;
                true
            } else {
                false
            }
        } else {
            false
        }
    } else {
        false
    };

    if arg_idx >= args.len() {
        return RespValue::Error("ERR syntax error".to_string());
    }

    let maxlen = match std::str::from_utf8(&args[arg_idx]) {
        Ok(s) => match s.parse::<usize>() {
            Ok(n) => n,
            Err(_) => return RespValue::Error("ERR invalid maxlen value".to_string()),
        },
        Err(_) => return RespValue::Error("ERR invalid maxlen value".to_string()),
    };

    let db_instance = match db.get_db(db_index) {
        Some(d) => d,
        None => return RespValue::Error("ERR invalid database".to_string()),
    };

    match db_instance.get(&key) {
        Some(RedisValue::Stream(mut stream)) => {
            let original_len = stream.entries.len();

            // If stream is already smaller than maxlen, no trimming needed
            if original_len <= maxlen {
                return RespValue::Integer(0);
            }

            // Keep only the last maxlen entries
            let to_remove = original_len - maxlen;
            let ids_to_remove: Vec<_> = stream.entries.keys().take(to_remove).cloned().collect();

            for id in ids_to_remove {
                stream.entries.remove(&id);
            }

            db_instance.set(key, RedisValue::Stream(stream));
            RespValue::Integer(to_remove as i64)
        }
        Some(_) => RespValue::Error(
            "WRONGTYPE Operation against a key holding the wrong kind of value".to_string(),
        ),
        None => RespValue::Integer(0),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_xadd_xlen() {
        let db = Arc::new(Database::new(16));

        // Add entries
        let result = xadd(
            &db,
            0,
            vec![
                b"mystream".to_vec(),
                b"*".to_vec(),
                b"field1".to_vec(),
                b"value1".to_vec(),
            ],
        )
        .await;

        // Should return an ID
        assert!(matches!(result, RespValue::BulkString(Some(_))));

        // Check length
        let result = xlen(&db, 0, vec![b"mystream".to_vec()]).await;
        assert_eq!(result, RespValue::Integer(1));
    }

    #[tokio::test]
    async fn test_xrange() {
        let db = Arc::new(Database::new(16));

        // Add multiple entries
        xadd(
            &db,
            0,
            vec![
                b"mystream".to_vec(),
                b"1-0".to_vec(),
                b"field1".to_vec(),
                b"value1".to_vec(),
            ],
        )
        .await;

        xadd(
            &db,
            0,
            vec![
                b"mystream".to_vec(),
                b"2-0".to_vec(),
                b"field2".to_vec(),
                b"value2".to_vec(),
            ],
        )
        .await;

        // Range query
        let result = xrange(
            &db,
            0,
            vec![b"mystream".to_vec(), b"-".to_vec(), b"+".to_vec()],
        )
        .await;

        if let RespValue::Array(Some(arr)) = result {
            assert_eq!(arr.len(), 2);
        } else {
            panic!("Expected array result");
        }
    }

    #[tokio::test]
    async fn test_xdel() {
        let db = Arc::new(Database::new(16));

        xadd(
            &db,
            0,
            vec![
                b"mystream".to_vec(),
                b"1-0".to_vec(),
                b"field1".to_vec(),
                b"value1".to_vec(),
            ],
        )
        .await;

        // Delete entry
        let result = xdel(&db, 0, vec![b"mystream".to_vec(), b"1-0".to_vec()]).await;
        assert_eq!(result, RespValue::Integer(1));

        // Check length
        let result = xlen(&db, 0, vec![b"mystream".to_vec()]).await;
        assert_eq!(result, RespValue::Integer(0));
    }
}
