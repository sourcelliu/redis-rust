// Key management commands for Redis-Rust
// Commands: RENAME, RENAMENX, COPY, MOVE, DUMP, RESTORE, SCAN, TOUCH, UNLINK, OBJECT

use crate::protocol::RespValue;
use crate::storage::db::Database;
use crate::storage::types::RedisValue;
use bytes::Bytes;
use std::sync::Arc;

/// RENAME key newkey
/// Rename a key atomically
pub async fn rename(db: &Arc<Database>, db_index: usize, args: Vec<Vec<u8>>) -> RespValue {
    if args.len() != 2 {
        return RespValue::Error("ERR wrong number of arguments for 'rename' command".to_string());
    }

    let key = match std::str::from_utf8(&args[0]) {
        Ok(s) => s.to_string(),
        Err(_) => return RespValue::Error("ERR invalid key".to_string()),
    };

    let newkey = match std::str::from_utf8(&args[1]) {
        Ok(s) => s.to_string(),
        Err(_) => return RespValue::Error("ERR invalid new key".to_string()),
    };

    let db_instance = match db.get_db(db_index) {
        Some(d) => d,
        None => return RespValue::Error("ERR invalid database".to_string()),
    };

    // Check if source key exists
    let value = match db_instance.get(&key) {
        Some(v) => v,
        None => return RespValue::Error("ERR no such key".to_string()),
    };

    // Get TTL if exists
    let ttl_ms = db_instance.get_ttl_ms(&key);

    // Delete old key
    db_instance.delete(&key);

    // Set new key with same value and TTL
    if ttl_ms > 0 {
        let expire_at_ms = crate::storage::db::current_timestamp_ms() + ttl_ms as u64;
        db_instance.set_with_expiry(newkey, value, expire_at_ms);
    } else {
        db_instance.set(newkey, value);
    }

    RespValue::SimpleString("OK".to_string())
}

/// RENAMENX key newkey
/// Rename key only if newkey doesn't exist
pub async fn renamenx(db: &Arc<Database>, db_index: usize, args: Vec<Vec<u8>>) -> RespValue {
    if args.len() != 2 {
        return RespValue::Error("ERR wrong number of arguments for 'renamenx' command".to_string());
    }

    let key = match std::str::from_utf8(&args[0]) {
        Ok(s) => s.to_string(),
        Err(_) => return RespValue::Error("ERR invalid key".to_string()),
    };

    let newkey = match std::str::from_utf8(&args[1]) {
        Ok(s) => s.to_string(),
        Err(_) => return RespValue::Error("ERR invalid new key".to_string()),
    };

    let db_instance = match db.get_db(db_index) {
        Some(d) => d,
        None => return RespValue::Error("ERR invalid database".to_string()),
    };

    // Check if newkey already exists
    if db_instance.exists(&newkey) {
        return RespValue::Integer(0);
    }

    // Check if source key exists
    let value = match db_instance.get(&key) {
        Some(v) => v,
        None => return RespValue::Error("ERR no such key".to_string()),
    };

    // Get TTL if exists
    let ttl_ms = db_instance.get_ttl_ms(&key);

    // Delete old key
    db_instance.delete(&key);

    // Set new key with same value and TTL
    if ttl_ms > 0 {
        let expire_at_ms = crate::storage::db::current_timestamp_ms() + ttl_ms as u64;
        db_instance.set_with_expiry(newkey, value, expire_at_ms);
    } else {
        db_instance.set(newkey, value);
    }

    RespValue::Integer(1)
}

/// COPY source dest [DB db] [REPLACE]
/// Copy a key to another key
pub async fn copy(db: &Arc<Database>, db_index: usize, args: Vec<Vec<u8>>) -> RespValue {
    if args.len() < 2 {
        return RespValue::Error("ERR wrong number of arguments for 'copy' command".to_string());
    }

    let source = match std::str::from_utf8(&args[0]) {
        Ok(s) => s.to_string(),
        Err(_) => return RespValue::Error("ERR invalid source key".to_string()),
    };

    let dest = match std::str::from_utf8(&args[1]) {
        Ok(s) => s.to_string(),
        Err(_) => return RespValue::Error("ERR invalid destination key".to_string()),
    };

    let mut target_db_index = db_index;
    let mut replace = false;

    // Parse optional arguments
    let mut i = 2;
    while i < args.len() {
        let arg = match std::str::from_utf8(&args[i]) {
            Ok(s) => s.to_uppercase(),
            Err(_) => return RespValue::Error("ERR invalid argument".to_string()),
        };

        match arg.as_str() {
            "DB" => {
                i += 1;
                if i >= args.len() {
                    return RespValue::Error("ERR syntax error".to_string());
                }
                target_db_index = match std::str::from_utf8(&args[i]) {
                    Ok(s) => match s.parse() {
                        Ok(idx) => idx,
                        Err(_) => return RespValue::Error("ERR invalid DB index".to_string()),
                    },
                    Err(_) => return RespValue::Error("ERR invalid DB index".to_string()),
                };
            }
            "REPLACE" => {
                replace = true;
            }
            _ => return RespValue::Error(format!("ERR syntax error near '{}'", arg)),
        }
        i += 1;
    }

    let source_db = match db.get_db(db_index) {
        Some(d) => d,
        None => return RespValue::Error("ERR invalid database".to_string()),
    };

    let target_db = match db.get_db(target_db_index) {
        Some(d) => d,
        None => return RespValue::Error("ERR invalid target database".to_string()),
    };

    // Check if source exists
    let value = match source_db.get(&source) {
        Some(v) => v,
        None => return RespValue::Integer(0),
    };

    // Check if dest exists and REPLACE not specified
    if !replace && target_db.exists(&dest) {
        return RespValue::Integer(0);
    }

    // Get TTL if exists
    let ttl_ms = source_db.get_ttl_ms(&source);

    // Copy to destination
    if ttl_ms > 0 {
        let expire_at_ms = crate::storage::db::current_timestamp_ms() + ttl_ms as u64;
        target_db.set_with_expiry(dest, value, expire_at_ms);
    } else {
        target_db.set(dest, value);
    }

    RespValue::Integer(1)
}

/// MOVE key db
/// Move key to different database
pub async fn move_key(db: &Arc<Database>, db_index: usize, args: Vec<Vec<u8>>) -> RespValue {
    if args.len() != 2 {
        return RespValue::Error("ERR wrong number of arguments for 'move' command".to_string());
    }

    let key = match std::str::from_utf8(&args[0]) {
        Ok(s) => s.to_string(),
        Err(_) => return RespValue::Error("ERR invalid key".to_string()),
    };

    let target_db_index: usize = match std::str::from_utf8(&args[1]) {
        Ok(s) => match s.parse() {
            Ok(idx) => idx,
            Err(_) => return RespValue::Error("ERR invalid DB index".to_string()),
        },
        Err(_) => return RespValue::Error("ERR invalid DB index".to_string()),
    };

    if target_db_index == db_index {
        return RespValue::Error("ERR source and destination objects are the same".to_string());
    }

    let source_db = match db.get_db(db_index) {
        Some(d) => d,
        None => return RespValue::Error("ERR invalid database".to_string()),
    };

    let target_db = match db.get_db(target_db_index) {
        Some(d) => d,
        None => return RespValue::Error("ERR invalid target database".to_string()),
    };

    // Check if key exists in target DB
    if target_db.exists(&key) {
        return RespValue::Integer(0);
    }

    // Check if source key exists
    let value = match source_db.get(&key) {
        Some(v) => v,
        None => return RespValue::Integer(0),
    };

    // Get TTL if exists
    let ttl_ms = source_db.get_ttl_ms(&key);

    // Move to target
    if ttl_ms > 0 {
        let expire_at_ms = crate::storage::db::current_timestamp_ms() + ttl_ms as u64;
        target_db.set_with_expiry(key.clone(), value, expire_at_ms);
    } else {
        target_db.set(key.clone(), value);
    }

    // Delete from source
    source_db.delete(&key);

    RespValue::Integer(1)
}

/// TOUCH key [key ...]
/// Update last access time for keys
pub async fn touch(db: &Arc<Database>, db_index: usize, args: Vec<Vec<u8>>) -> RespValue {
    if args.is_empty() {
        return RespValue::Error("ERR wrong number of arguments for 'touch' command".to_string());
    }

    let db_instance = match db.get_db(db_index) {
        Some(d) => d,
        None => return RespValue::Error("ERR invalid database".to_string()),
    };

    let mut count = 0;
    for key_bytes in args {
        let key = match std::str::from_utf8(&key_bytes) {
            Ok(s) => s,
            Err(_) => continue,
        };

        // Just check if key exists - in a full implementation, we'd update LRU
        if db_instance.exists(key) {
            count += 1;
        }
    }

    RespValue::Integer(count)
}

/// UNLINK key [key ...]
/// Asynchronous delete (in our simple impl, same as DEL)
pub async fn unlink(db: &Arc<Database>, db_index: usize, args: Vec<Vec<u8>>) -> RespValue {
    if args.is_empty() {
        return RespValue::Error("ERR wrong number of arguments for 'unlink' command".to_string());
    }

    let db_instance = match db.get_db(db_index) {
        Some(d) => d,
        None => return RespValue::Error("ERR invalid database".to_string()),
    };

    let mut count = 0;
    for key_bytes in args {
        let key = match std::str::from_utf8(&key_bytes) {
            Ok(s) => s,
            Err(_) => continue,
        };

        if db_instance.delete(key) {
            count += 1;
        }
    }

    RespValue::Integer(count)
}

/// DUMP key
/// Serialize key value to binary format
pub async fn dump(db: &Arc<Database>, db_index: usize, args: Vec<Vec<u8>>) -> RespValue {
    if args.len() != 1 {
        return RespValue::Error("ERR wrong number of arguments for 'dump' command".to_string());
    }

    let key = match std::str::from_utf8(&args[0]) {
        Ok(s) => s,
        Err(_) => return RespValue::Error("ERR invalid key".to_string()),
    };

    let db_instance = match db.get_db(db_index) {
        Some(d) => d,
        None => return RespValue::Error("ERR invalid database".to_string()),
    };

    let value = match db_instance.get(key) {
        Some(v) => v,
        None => return RespValue::Null,
    };

    // Simple serialization format:
    // [type_byte][data...]
    // For simplicity, we'll use a basic format
    let serialized = serialize_value(&value);

    RespValue::BulkString(Some(serialized))
}

/// RESTORE key ttl serialized-value [REPLACE] [ABSTTL] [IDLETIME seconds] [FREQ frequency]
/// Deserialize binary data to create a key
pub async fn restore(db: &Arc<Database>, db_index: usize, args: Vec<Vec<u8>>) -> RespValue {
    if args.len() < 3 {
        return RespValue::Error("ERR wrong number of arguments for 'restore' command".to_string());
    }

    let key = match std::str::from_utf8(&args[0]) {
        Ok(s) => s.to_string(),
        Err(_) => return RespValue::Error("ERR invalid key".to_string()),
    };

    let ttl_ms: i64 = match std::str::from_utf8(&args[1]) {
        Ok(s) => match s.parse() {
            Ok(t) => t,
            Err(_) => return RespValue::Error("ERR invalid TTL".to_string()),
        },
        Err(_) => return RespValue::Error("ERR invalid TTL".to_string()),
    };

    let serialized = &args[2];

    let mut replace = false;

    // Parse optional arguments
    for i in 3..args.len() {
        let arg = match std::str::from_utf8(&args[i]) {
            Ok(s) => s.to_uppercase(),
            Err(_) => continue,
        };

        if arg == "REPLACE" {
            replace = true;
        }
    }

    let db_instance = match db.get_db(db_index) {
        Some(d) => d,
        None => return RespValue::Error("ERR invalid database".to_string()),
    };

    // Check if key exists and REPLACE not specified
    if !replace && db_instance.exists(&key) {
        return RespValue::Error("BUSYKEY Target key name already exists.".to_string());
    }

    // Deserialize value
    let value = match deserialize_value(serialized) {
        Ok(v) => v,
        Err(e) => return RespValue::Error(format!("ERR {}", e)),
    };

    // Set key with optional TTL
    if ttl_ms > 0 {
        let expire_at_ms = crate::storage::db::current_timestamp_ms() + ttl_ms as u64;
        db_instance.set_with_expiry(key, value, expire_at_ms);
    } else {
        db_instance.set(key, value);
    }

    RespValue::SimpleString("OK".to_string())
}

/// SCAN cursor [MATCH pattern] [COUNT count] [TYPE type]
/// Iterate keys with cursor-based pagination
pub async fn scan(db: &Arc<Database>, db_index: usize, args: Vec<Vec<u8>>) -> RespValue {
    if args.is_empty() {
        return RespValue::Error("ERR wrong number of arguments for 'scan' command".to_string());
    }

    let cursor: usize = match std::str::from_utf8(&args[0]) {
        Ok(s) => match s.parse() {
            Ok(c) => c,
            Err(_) => return RespValue::Error("ERR invalid cursor".to_string()),
        },
        Err(_) => return RespValue::Error("ERR invalid cursor".to_string()),
    };

    let mut pattern = "*";
    let mut count = 10;

    // Parse options
    let mut i = 1;
    while i < args.len() {
        let arg = match std::str::from_utf8(&args[i]) {
            Ok(s) => s.to_uppercase(),
            Err(_) => {
                i += 1;
                continue;
            }
        };

        match arg.as_str() {
            "MATCH" => {
                i += 1;
                if i < args.len() {
                    pattern = match std::str::from_utf8(&args[i]) {
                        Ok(s) => s,
                        Err(_) => "*",
                    };
                }
            }
            "COUNT" => {
                i += 1;
                if i < args.len() {
                    count = match std::str::from_utf8(&args[i]) {
                        Ok(s) => s.parse().unwrap_or(10),
                        Err(_) => 10,
                    };
                }
            }
            _ => {}
        }
        i += 1;
    }

    let db_instance = match db.get_db(db_index) {
        Some(d) => d,
        None => return RespValue::Error("ERR invalid database".to_string()),
    };

    // Get all keys matching pattern
    let all_keys = db_instance.keys(pattern);

    // Pagination: skip to cursor position
    let start = cursor;
    let end = std::cmp::min(start + count, all_keys.len());

    let keys: Vec<RespValue> = all_keys[start..end]
        .iter()
        .map(|k| RespValue::BulkString(Some(k.as_bytes().to_vec())))
        .collect();

    // Calculate next cursor (0 means iteration complete)
    let next_cursor = if end < all_keys.len() {
        end.to_string()
    } else {
        "0".to_string()
    };

    RespValue::Array(Some(vec![
        RespValue::BulkString(Some(next_cursor.into_bytes())),
        RespValue::Array(Some(keys)),
    ]))
}

/// OBJECT subcommand [arguments]
/// Inspect Redis objects
pub async fn object(db: &Arc<Database>, db_index: usize, args: Vec<Vec<u8>>) -> RespValue {
    if args.is_empty() {
        return RespValue::Error("ERR wrong number of arguments for 'object' command".to_string());
    }

    let subcommand = match std::str::from_utf8(&args[0]) {
        Ok(s) => s.to_uppercase(),
        Err(_) => return RespValue::Error("ERR invalid subcommand".to_string()),
    };

    match subcommand.as_str() {
        "REFCOUNT" => {
            if args.len() != 2 {
                return RespValue::Error("ERR wrong number of arguments for 'object|refcount' command".to_string());
            }
            let key = match std::str::from_utf8(&args[1]) {
                Ok(s) => s,
                Err(_) => return RespValue::Error("ERR invalid key".to_string()),
            };

            let db_instance = match db.get_db(db_index) {
                Some(d) => d,
                None => return RespValue::Error("ERR invalid database".to_string()),
            };

            if db_instance.exists(key) {
                // In Rust with Arc, refcount is always effectively 1 from Redis perspective
                RespValue::Integer(1)
            } else {
                RespValue::Null
            }
        }
        "ENCODING" => {
            if args.len() != 2 {
                return RespValue::Error("ERR wrong number of arguments for 'object|encoding' command".to_string());
            }
            let key = match std::str::from_utf8(&args[1]) {
                Ok(s) => s,
                Err(_) => return RespValue::Error("ERR invalid key".to_string()),
            };

            let db_instance = match db.get_db(db_index) {
                Some(d) => d,
                None => return RespValue::Error("ERR invalid database".to_string()),
            };

            match db_instance.get(key) {
                Some(value) => {
                    let encoding = match value {
                        RedisValue::String(_) => "raw",
                        RedisValue::List(_) => "linkedlist",
                        RedisValue::Set(_) => "hashtable",
                        RedisValue::Hash(_) => "hashtable",
                        RedisValue::ZSet(_) => "skiplist",
                        RedisValue::Stream(_) => "stream",
                    };
                    RespValue::SimpleString(encoding.to_string())
                }
                None => RespValue::Null,
            }
        }
        "IDLETIME" => {
            if args.len() != 2 {
                return RespValue::Error("ERR wrong number of arguments for 'object|idletime' command".to_string());
            }
            let key = match std::str::from_utf8(&args[1]) {
                Ok(s) => s,
                Err(_) => return RespValue::Error("ERR invalid key".to_string()),
            };

            let db_instance = match db.get_db(db_index) {
                Some(d) => d,
                None => return RespValue::Error("ERR invalid database".to_string()),
            };

            if db_instance.exists(key) {
                // In our simple implementation, we don't track access time
                // Return 0 to indicate recently accessed
                RespValue::Integer(0)
            } else {
                RespValue::Null
            }
        }
        _ => RespValue::Error(format!("ERR unknown subcommand '{}'", subcommand)),
    }
}

/// Serialize a RedisValue to bytes
fn serialize_value(value: &RedisValue) -> Vec<u8> {
    let mut result = Vec::new();

    match value {
        RedisValue::String(bytes) => {
            result.push(0); // Type: String
            let len = (bytes.len() as u32).to_le_bytes();
            result.extend_from_slice(&len);
            result.extend_from_slice(bytes);
        }
        RedisValue::List(list) => {
            result.push(1); // Type: List
            let len = (list.len() as u32).to_le_bytes();
            result.extend_from_slice(&len);
            for item in list {
                let item_len = (item.len() as u32).to_le_bytes();
                result.extend_from_slice(&item_len);
                result.extend_from_slice(item);
            }
        }
        RedisValue::Set(set) => {
            result.push(2); // Type: Set
            let len = (set.len() as u32).to_le_bytes();
            result.extend_from_slice(&len);
            for item in set {
                let item_len = (item.len() as u32).to_le_bytes();
                result.extend_from_slice(&item_len);
                result.extend_from_slice(item);
            }
        }
        RedisValue::Hash(hash) => {
            result.push(3); // Type: Hash
            let len = (hash.len() as u32).to_le_bytes();
            result.extend_from_slice(&len);
            for (key, value) in hash {
                let key_len = (key.len() as u32).to_le_bytes();
                result.extend_from_slice(&key_len);
                result.extend_from_slice(key);
                let val_len = (value.len() as u32).to_le_bytes();
                result.extend_from_slice(&val_len);
                result.extend_from_slice(value);
            }
        }
        RedisValue::ZSet(zset) => {
            result.push(4); // Type: ZSet
            let len = (zset.members.len() as u32).to_le_bytes();
            result.extend_from_slice(&len);
            for (member, score) in &zset.members {
                let member_len = (member.len() as u32).to_le_bytes();
                result.extend_from_slice(&member_len);
                result.extend_from_slice(member);
                result.extend_from_slice(&score.to_le_bytes());
            }
        }
        RedisValue::Stream(_) => {
            result.push(5); // Type: Stream (simplified)
            // For now, just mark as stream type
        }
    }

    result
}

/// Deserialize bytes to a RedisValue
fn deserialize_value(bytes: &[u8]) -> Result<RedisValue, String> {
    if bytes.is_empty() {
        return Err("Empty serialized data".to_string());
    }

    let type_byte = bytes[0];
    let data = &bytes[1..];

    match type_byte {
        0 => {
            // String
            if data.len() < 4 {
                return Err("Invalid string data".to_string());
            }
            let len = u32::from_le_bytes([data[0], data[1], data[2], data[3]]) as usize;
            if data.len() < 4 + len {
                return Err("Invalid string length".to_string());
            }
            let bytes = Bytes::copy_from_slice(&data[4..4 + len]);
            Ok(RedisValue::String(bytes))
        }
        1 => {
            // List
            if data.len() < 4 {
                return Err("Invalid list data".to_string());
            }
            let count = u32::from_le_bytes([data[0], data[1], data[2], data[3]]) as usize;
            let mut list = std::collections::LinkedList::new();
            let mut pos = 4;

            for _ in 0..count {
                if pos + 4 > data.len() {
                    return Err("Invalid list item".to_string());
                }
                let item_len = u32::from_le_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]]) as usize;
                pos += 4;
                if pos + item_len > data.len() {
                    return Err("Invalid list item length".to_string());
                }
                let item = Bytes::copy_from_slice(&data[pos..pos + item_len]);
                list.push_back(item);
                pos += item_len;
            }

            Ok(RedisValue::List(list))
        }
        2 => {
            // Set
            if data.len() < 4 {
                return Err("Invalid set data".to_string());
            }
            let count = u32::from_le_bytes([data[0], data[1], data[2], data[3]]) as usize;
            let mut set = std::collections::HashSet::new();
            let mut pos = 4;

            for _ in 0..count {
                if pos + 4 > data.len() {
                    return Err("Invalid set item".to_string());
                }
                let item_len = u32::from_le_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]]) as usize;
                pos += 4;
                if pos + item_len > data.len() {
                    return Err("Invalid set item length".to_string());
                }
                let item = Bytes::copy_from_slice(&data[pos..pos + item_len]);
                set.insert(item);
                pos += item_len;
            }

            Ok(RedisValue::Set(set))
        }
        3 => {
            // Hash
            if data.len() < 4 {
                return Err("Invalid hash data".to_string());
            }
            let count = u32::from_le_bytes([data[0], data[1], data[2], data[3]]) as usize;
            let mut hash = std::collections::HashMap::new();
            let mut pos = 4;

            for _ in 0..count {
                // Read key
                if pos + 4 > data.len() {
                    return Err("Invalid hash key".to_string());
                }
                let key_len = u32::from_le_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]]) as usize;
                pos += 4;
                if pos + key_len > data.len() {
                    return Err("Invalid hash key length".to_string());
                }
                let key = Bytes::copy_from_slice(&data[pos..pos + key_len]);
                pos += key_len;

                // Read value
                if pos + 4 > data.len() {
                    return Err("Invalid hash value".to_string());
                }
                let val_len = u32::from_le_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]]) as usize;
                pos += 4;
                if pos + val_len > data.len() {
                    return Err("Invalid hash value length".to_string());
                }
                let value = Bytes::copy_from_slice(&data[pos..pos + val_len]);
                pos += val_len;

                hash.insert(key, value);
            }

            Ok(RedisValue::Hash(hash))
        }
        4 => {
            // ZSet
            if data.len() < 4 {
                return Err("Invalid zset data".to_string());
            }
            let count = u32::from_le_bytes([data[0], data[1], data[2], data[3]]) as usize;
            let mut zset = crate::storage::types::ZSet::new();
            let mut pos = 4;

            for _ in 0..count {
                // Read member
                if pos + 4 > data.len() {
                    return Err("Invalid zset member".to_string());
                }
                let member_len = u32::from_le_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]]) as usize;
                pos += 4;
                if pos + member_len > data.len() {
                    return Err("Invalid zset member length".to_string());
                }
                let member = Bytes::copy_from_slice(&data[pos..pos + member_len]);
                pos += member_len;

                // Read score
                if pos + 8 > data.len() {
                    return Err("Invalid zset score".to_string());
                }
                let score = f64::from_le_bytes([
                    data[pos], data[pos + 1], data[pos + 2], data[pos + 3],
                    data[pos + 4], data[pos + 5], data[pos + 6], data[pos + 7],
                ]);
                pos += 8;

                zset.members.insert(member.clone(), score);
                zset.scores.insert((ordered_float::OrderedFloat(score), member), ());
            }

            Ok(RedisValue::ZSet(zset))
        }
        _ => Err(format!("Unknown type byte: {}", type_byte)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_rename() {
        let db = Arc::new(Database::new(16));
        let db_instance = db.get_db(0).unwrap();

        // Set a key
        db_instance.set("oldkey".to_string(), RedisValue::String(Bytes::from("value")));

        // Rename it
        let result = rename(&db, 0, vec![b"oldkey".to_vec(), b"newkey".to_vec()]).await;
        assert_eq!(result, RespValue::SimpleString("OK".to_string()));

        // Check old key doesn't exist, new key does
        assert!(!db_instance.exists("oldkey"));
        assert!(db_instance.exists("newkey"));
    }

    #[tokio::test]
    async fn test_copy() {
        let db = Arc::new(Database::new(16));
        let db_instance = db.get_db(0).unwrap();

        // Set a key
        db_instance.set("source".to_string(), RedisValue::String(Bytes::from("value")));

        // Copy it
        let result = copy(&db, 0, vec![b"source".to_vec(), b"dest".to_vec()]).await;
        assert_eq!(result, RespValue::Integer(1));

        // Check both exist
        assert!(db_instance.exists("source"));
        assert!(db_instance.exists("dest"));
    }

    #[tokio::test]
    async fn test_dump_restore() {
        let db = Arc::new(Database::new(16));
        let db_instance = db.get_db(0).unwrap();

        // Set a key
        db_instance.set("mykey".to_string(), RedisValue::String(Bytes::from("hello")));

        // Dump it
        let dump_result = dump(&db, 0, vec![b"mykey".to_vec()]).await;
        let serialized = match dump_result {
            RespValue::BulkString(Some(data)) => data,
            _ => panic!("Expected bulk string"),
        };

        // Delete the key
        db_instance.delete("mykey");

        // Restore it
        let result = restore(
            &db,
            0,
            vec![b"mykey".to_vec(), b"0".to_vec(), serialized],
        )
        .await;
        assert_eq!(result, RespValue::SimpleString("OK".to_string()));

        // Verify restored
        assert!(db_instance.exists("mykey"));
    }
}
