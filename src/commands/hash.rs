// Hash command handlers

use crate::protocol::RespValue;
use crate::storage::db::Database;
use crate::storage::types::RedisValue;
use bytes::Bytes;
use std::collections::HashMap;
use std::sync::Arc;

/// HSET key field value [field value ...]
pub async fn hset(db: &Arc<Database>, db_index: usize, args: Vec<Vec<u8>>) -> RespValue {
    if args.len() < 3 || args.len() % 2 == 0 {
        return RespValue::Error("ERR wrong number of arguments for 'hset' command".to_string());
    }

    let key = match std::str::from_utf8(&args[0]) {
        Ok(s) => s.to_string(),
        Err(_) => return RespValue::Error("ERR invalid key".to_string()),
    };

    let db_instance = match db.get_db(db_index) {
        Some(d) => d,
        None => return RespValue::Error("ERR invalid database".to_string()),
    };

    let mut hash = match db_instance.get(&key) {
        Some(RedisValue::Hash(h)) => h,
        Some(_) => {
            return RespValue::Error(
                "WRONGTYPE Operation against a key holding the wrong kind of value".to_string(),
            )
        }
        None => HashMap::new(),
    };

    let mut added = 0;
    for chunk in args[1..].chunks(2) {
        let field = Bytes::from(chunk[0].clone());
        let value = Bytes::from(chunk[1].clone());

        if hash.insert(field, value).is_none() {
            added += 1;
        }
    }

    db_instance.set(key, RedisValue::Hash(hash));
    RespValue::Integer(added)
}

/// HGET key field
pub async fn hget(db: &Arc<Database>, db_index: usize, args: Vec<Vec<u8>>) -> RespValue {
    if args.len() != 2 {
        return RespValue::Error("ERR wrong number of arguments for 'hget' command".to_string());
    }

    let key = match std::str::from_utf8(&args[0]) {
        Ok(s) => s,
        Err(_) => return RespValue::Error("ERR invalid key".to_string()),
    };

    let field = Bytes::from(args[1].clone());

    let db_instance = match db.get_db(db_index) {
        Some(d) => d,
        None => return RespValue::Error("ERR invalid database".to_string()),
    };

    match db_instance.get(key) {
        Some(RedisValue::Hash(hash)) => match hash.get(&field) {
            Some(value) => RespValue::BulkString(Some(value.to_vec())),
            None => RespValue::BulkString(None),
        },
        Some(_) => RespValue::Error(
            "WRONGTYPE Operation against a key holding the wrong kind of value".to_string(),
        ),
        None => RespValue::BulkString(None),
    }
}

/// HDEL key field [field ...]
pub async fn hdel(db: &Arc<Database>, db_index: usize, args: Vec<Vec<u8>>) -> RespValue {
    if args.len() < 2 {
        return RespValue::Error("ERR wrong number of arguments for 'hdel' command".to_string());
    }

    let key = match std::str::from_utf8(&args[0]) {
        Ok(s) => s.to_string(),
        Err(_) => return RespValue::Error("ERR invalid key".to_string()),
    };

    let db_instance = match db.get_db(db_index) {
        Some(d) => d,
        None => return RespValue::Error("ERR invalid database".to_string()),
    };

    let mut hash = match db_instance.get(&key) {
        Some(RedisValue::Hash(h)) => h,
        Some(_) => {
            return RespValue::Error(
                "WRONGTYPE Operation against a key holding the wrong kind of value".to_string(),
            )
        }
        None => return RespValue::Integer(0),
    };

    let mut deleted = 0;
    for field_bytes in &args[1..] {
        let field = Bytes::from(field_bytes.clone());
        if hash.remove(&field).is_some() {
            deleted += 1;
        }
    }

    if hash.is_empty() {
        db_instance.delete(&key);
    } else {
        db_instance.set(key, RedisValue::Hash(hash));
    }

    RespValue::Integer(deleted)
}

/// HEXISTS key field
pub async fn hexists(db: &Arc<Database>, db_index: usize, args: Vec<Vec<u8>>) -> RespValue {
    if args.len() != 2 {
        return RespValue::Error("ERR wrong number of arguments for 'hexists' command".to_string());
    }

    let key = match std::str::from_utf8(&args[0]) {
        Ok(s) => s,
        Err(_) => return RespValue::Error("ERR invalid key".to_string()),
    };

    let field = Bytes::from(args[1].clone());

    let db_instance = match db.get_db(db_index) {
        Some(d) => d,
        None => return RespValue::Error("ERR invalid database".to_string()),
    };

    match db_instance.get(key) {
        Some(RedisValue::Hash(hash)) => {
            RespValue::Integer(if hash.contains_key(&field) { 1 } else { 0 })
        }
        Some(_) => RespValue::Error(
            "WRONGTYPE Operation against a key holding the wrong kind of value".to_string(),
        ),
        None => RespValue::Integer(0),
    }
}

/// HGETALL key
pub async fn hgetall(db: &Arc<Database>, db_index: usize, args: Vec<Vec<u8>>) -> RespValue {
    if args.len() != 1 {
        return RespValue::Error("ERR wrong number of arguments for 'hgetall' command".to_string());
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
        Some(RedisValue::Hash(hash)) => {
            let mut result = Vec::new();
            for (field, value) in hash.iter() {
                result.push(RespValue::BulkString(Some(field.to_vec())));
                result.push(RespValue::BulkString(Some(value.to_vec())));
            }
            RespValue::Array(Some(result))
        }
        Some(_) => RespValue::Error(
            "WRONGTYPE Operation against a key holding the wrong kind of value".to_string(),
        ),
        None => RespValue::Array(Some(vec![])),
    }
}

/// HKEYS key
pub async fn hkeys(db: &Arc<Database>, db_index: usize, args: Vec<Vec<u8>>) -> RespValue {
    if args.len() != 1 {
        return RespValue::Error("ERR wrong number of arguments for 'hkeys' command".to_string());
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
        Some(RedisValue::Hash(hash)) => {
            let result: Vec<RespValue> = hash
                .keys()
                .map(|k| RespValue::BulkString(Some(k.to_vec())))
                .collect();
            RespValue::Array(Some(result))
        }
        Some(_) => RespValue::Error(
            "WRONGTYPE Operation against a key holding the wrong kind of value".to_string(),
        ),
        None => RespValue::Array(Some(vec![])),
    }
}

/// HVALS key
pub async fn hvals(db: &Arc<Database>, db_index: usize, args: Vec<Vec<u8>>) -> RespValue {
    if args.len() != 1 {
        return RespValue::Error("ERR wrong number of arguments for 'hvals' command".to_string());
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
        Some(RedisValue::Hash(hash)) => {
            let result: Vec<RespValue> = hash
                .values()
                .map(|v| RespValue::BulkString(Some(v.to_vec())))
                .collect();
            RespValue::Array(Some(result))
        }
        Some(_) => RespValue::Error(
            "WRONGTYPE Operation against a key holding the wrong kind of value".to_string(),
        ),
        None => RespValue::Array(Some(vec![])),
    }
}

/// HLEN key
pub async fn hlen(db: &Arc<Database>, db_index: usize, args: Vec<Vec<u8>>) -> RespValue {
    if args.len() != 1 {
        return RespValue::Error("ERR wrong number of arguments for 'hlen' command".to_string());
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
        Some(RedisValue::Hash(hash)) => RespValue::Integer(hash.len() as i64),
        Some(_) => RespValue::Error(
            "WRONGTYPE Operation against a key holding the wrong kind of value".to_string(),
        ),
        None => RespValue::Integer(0),
    }
}

/// HMGET key field [field ...]
pub async fn hmget(db: &Arc<Database>, db_index: usize, args: Vec<Vec<u8>>) -> RespValue {
    if args.len() < 2 {
        return RespValue::Error("ERR wrong number of arguments for 'hmget' command".to_string());
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
        Some(RedisValue::Hash(hash)) => {
            let mut result = Vec::new();
            for field_bytes in &args[1..] {
                let field = Bytes::from(field_bytes.clone());
                match hash.get(&field) {
                    Some(value) => result.push(RespValue::BulkString(Some(value.to_vec()))),
                    None => result.push(RespValue::BulkString(None)),
                }
            }
            RespValue::Array(Some(result))
        }
        Some(_) => RespValue::Error(
            "WRONGTYPE Operation against a key holding the wrong kind of value".to_string(),
        ),
        None => {
            let result = vec![RespValue::BulkString(None); args.len() - 1];
            RespValue::Array(Some(result))
        }
    }
}

/// HMSET key field value [field value ...]
pub async fn hmset(db: &Arc<Database>, db_index: usize, args: Vec<Vec<u8>>) -> RespValue {
    if args.len() < 3 || args.len() % 2 == 0 {
        return RespValue::Error("ERR wrong number of arguments for 'hmset' command".to_string());
    }

    let key = match std::str::from_utf8(&args[0]) {
        Ok(s) => s.to_string(),
        Err(_) => return RespValue::Error("ERR invalid key".to_string()),
    };

    let db_instance = match db.get_db(db_index) {
        Some(d) => d,
        None => return RespValue::Error("ERR invalid database".to_string()),
    };

    let mut hash = match db_instance.get(&key) {
        Some(RedisValue::Hash(h)) => h,
        Some(_) => {
            return RespValue::Error(
                "WRONGTYPE Operation against a key holding the wrong kind of value".to_string(),
            )
        }
        None => HashMap::new(),
    };

    for chunk in args[1..].chunks(2) {
        let field = Bytes::from(chunk[0].clone());
        let value = Bytes::from(chunk[1].clone());
        hash.insert(field, value);
    }

    db_instance.set(key, RedisValue::Hash(hash));
    RespValue::SimpleString("OK".to_string())
}

/// HSETNX key field value
pub async fn hsetnx(db: &Arc<Database>, db_index: usize, args: Vec<Vec<u8>>) -> RespValue {
    if args.len() != 3 {
        return RespValue::Error("ERR wrong number of arguments for 'hsetnx' command".to_string());
    }

    let key = match std::str::from_utf8(&args[0]) {
        Ok(s) => s.to_string(),
        Err(_) => return RespValue::Error("ERR invalid key".to_string()),
    };

    let field = Bytes::from(args[1].clone());
    let value = Bytes::from(args[2].clone());

    let db_instance = match db.get_db(db_index) {
        Some(d) => d,
        None => return RespValue::Error("ERR invalid database".to_string()),
    };

    let mut hash = match db_instance.get(&key) {
        Some(RedisValue::Hash(h)) => h,
        Some(_) => {
            return RespValue::Error(
                "WRONGTYPE Operation against a key holding the wrong kind of value".to_string(),
            )
        }
        None => HashMap::new(),
    };

    if hash.contains_key(&field) {
        return RespValue::Integer(0);
    }

    hash.insert(field, value);
    db_instance.set(key, RedisValue::Hash(hash));
    RespValue::Integer(1)
}

/// HINCRBY key field increment
pub async fn hincrby(db: &Arc<Database>, db_index: usize, args: Vec<Vec<u8>>) -> RespValue {
    if args.len() != 3 {
        return RespValue::Error("ERR wrong number of arguments for 'hincrby' command".to_string());
    }

    let key = match std::str::from_utf8(&args[0]) {
        Ok(s) => s.to_string(),
        Err(_) => return RespValue::Error("ERR invalid key".to_string()),
    };

    let field = Bytes::from(args[1].clone());

    let increment = match std::str::from_utf8(&args[2]) {
        Ok(s) => match s.parse::<i64>() {
            Ok(n) => n,
            Err(_) => {
                return RespValue::Error("ERR value is not an integer or out of range".to_string())
            }
        },
        Err(_) => return RespValue::Error("ERR value is not an integer or out of range".to_string()),
    };

    let db_instance = match db.get_db(db_index) {
        Some(d) => d,
        None => return RespValue::Error("ERR invalid database".to_string()),
    };

    let mut hash = match db_instance.get(&key) {
        Some(RedisValue::Hash(h)) => h,
        Some(_) => {
            return RespValue::Error(
                "WRONGTYPE Operation against a key holding the wrong kind of value".to_string(),
            )
        }
        None => HashMap::new(),
    };

    let current_value = match hash.get(&field) {
        Some(bytes) => match std::str::from_utf8(bytes) {
            Ok(s) => match s.parse::<i64>() {
                Ok(n) => n,
                Err(_) => {
                    return RespValue::Error("ERR hash value is not an integer".to_string())
                }
            },
            Err(_) => return RespValue::Error("ERR hash value is not an integer".to_string()),
        },
        None => 0,
    };

    let new_value = match current_value.checked_add(increment) {
        Some(v) => v,
        None => return RespValue::Error("ERR increment would overflow".to_string()),
    };

    hash.insert(field, Bytes::from(new_value.to_string().into_bytes()));
    db_instance.set(key, RedisValue::Hash(hash));
    RespValue::Integer(new_value)
}

/// HINCRBYFLOAT key field increment
pub async fn hincrbyfloat(db: &Arc<Database>, db_index: usize, args: Vec<Vec<u8>>) -> RespValue {
    if args.len() != 3 {
        return RespValue::Error(
            "ERR wrong number of arguments for 'hincrbyfloat' command".to_string(),
        );
    }

    let key = match std::str::from_utf8(&args[0]) {
        Ok(s) => s.to_string(),
        Err(_) => return RespValue::Error("ERR invalid key".to_string()),
    };

    let field = Bytes::from(args[1].clone());

    let increment = match std::str::from_utf8(&args[2]) {
        Ok(s) => match s.parse::<f64>() {
            Ok(n) => n,
            Err(_) => return RespValue::Error("ERR value is not a valid float".to_string()),
        },
        Err(_) => return RespValue::Error("ERR value is not a valid float".to_string()),
    };

    let db_instance = match db.get_db(db_index) {
        Some(d) => d,
        None => return RespValue::Error("ERR invalid database".to_string()),
    };

    let mut hash = match db_instance.get(&key) {
        Some(RedisValue::Hash(h)) => h,
        Some(_) => {
            return RespValue::Error(
                "WRONGTYPE Operation against a key holding the wrong kind of value".to_string(),
            )
        }
        None => HashMap::new(),
    };

    let current_value = match hash.get(&field) {
        Some(bytes) => match std::str::from_utf8(bytes) {
            Ok(s) => match s.parse::<f64>() {
                Ok(n) => n,
                Err(_) => return RespValue::Error("ERR hash value is not a float".to_string()),
            },
            Err(_) => return RespValue::Error("ERR hash value is not a float".to_string()),
        },
        None => 0.0,
    };

    let new_value = current_value + increment;

    // Format the float properly (remove trailing zeros for clean output)
    let formatted = if new_value.fract() == 0.0 && new_value.abs() < 1e10 {
        format!("{:.1}", new_value)
    } else {
        format!("{}", new_value)
    };

    hash.insert(field, Bytes::from(formatted.clone().into_bytes()));
    db_instance.set(key, RedisValue::Hash(hash));
    RespValue::BulkString(Some(formatted.into_bytes()))
}

/// HSTRLEN key field
pub async fn hstrlen(db: &Arc<Database>, db_index: usize, args: Vec<Vec<u8>>) -> RespValue {
    if args.len() != 2 {
        return RespValue::Error("ERR wrong number of arguments for 'hstrlen' command".to_string());
    }

    let key = match std::str::from_utf8(&args[0]) {
        Ok(s) => s,
        Err(_) => return RespValue::Error("ERR invalid key".to_string()),
    };

    let field = Bytes::from(args[1].clone());

    let db_instance = match db.get_db(db_index) {
        Some(d) => d,
        None => return RespValue::Error("ERR invalid database".to_string()),
    };

    match db_instance.get(key) {
        Some(RedisValue::Hash(hash)) => match hash.get(&field) {
            Some(value) => RespValue::Integer(value.len() as i64),
            None => RespValue::Integer(0),
        },
        Some(_) => RespValue::Error(
            "WRONGTYPE Operation against a key holding the wrong kind of value".to_string(),
        ),
        None => RespValue::Integer(0),
    }
}

/// HSCAN key cursor [MATCH pattern] [COUNT count]
/// Incrementally iterate hash fields and values
pub async fn hscan(db: &Arc<Database>, db_index: usize, args: Vec<Vec<u8>>) -> RespValue {
    if args.len() < 2 {
        return RespValue::Error("ERR wrong number of arguments for 'hscan' command".to_string());
    }

    let key = match std::str::from_utf8(&args[0]) {
        Ok(s) => s,
        Err(_) => return RespValue::Error("ERR invalid key".to_string()),
    };

    let cursor = match std::str::from_utf8(&args[1]) {
        Ok(s) => match s.parse::<usize>() {
            Ok(n) => n,
            Err(_) => return RespValue::Error("ERR invalid cursor".to_string()),
        },
        Err(_) => return RespValue::Error("ERR invalid cursor".to_string()),
    };

    let db_instance = match db.get_db(db_index) {
        Some(d) => d,
        None => return RespValue::Error("ERR invalid database".to_string()),
    };

    // Parse optional COUNT parameter
    let mut count = 10; // Default count
    let mut i = 2;
    while i < args.len() {
        if let Ok(s) = std::str::from_utf8(&args[i]) {
            if s.to_uppercase() == "COUNT" && i + 1 < args.len() {
                if let Ok(c_str) = std::str::from_utf8(&args[i + 1]) {
                    if let Ok(c) = c_str.parse::<usize>() {
                        count = c;
                    }
                }
                i += 2;
            } else if s.to_uppercase() == "MATCH" && i + 1 < args.len() {
                // MATCH pattern support (basic implementation - just skip for now)
                i += 2;
            } else {
                i += 1;
            }
        } else {
            i += 1;
        }
    }

    match db_instance.get(key) {
        Some(RedisValue::Hash(hash)) => {
            let fields: Vec<_> = hash.iter().collect();
            let start = cursor;
            let end = (start + count).min(fields.len());
            let next_cursor = if end >= fields.len() { 0 } else { end };

            // Build results array with field-value pairs
            let mut results = Vec::new();
            for i in start..end {
                if let Some((field, value)) = fields.get(i) {
                    results.push(RespValue::BulkString(Some(field.to_vec())));
                    results.push(RespValue::BulkString(Some(value.to_vec())));
                }
            }

            // Return [next_cursor, [field1, value1, field2, value2, ...]]
            RespValue::Array(Some(vec![
                RespValue::BulkString(Some(next_cursor.to_string().into_bytes())),
                RespValue::Array(Some(results)),
            ]))
        }
        Some(_) => RespValue::Error(
            "WRONGTYPE Operation against a key holding the wrong kind of value".to_string(),
        ),
        None => {
            // Empty hash - return cursor 0 and empty array
            RespValue::Array(Some(vec![
                RespValue::BulkString(Some(b"0".to_vec())),
                RespValue::Array(Some(vec![])),
            ]))
        }
    }
}

/// HRANDFIELD key [count [WITHVALUES]]
/// Return random field(s) from hash
pub async fn hrandfield(db: &Arc<Database>, db_index: usize, args: Vec<Vec<u8>>) -> RespValue {
    if args.is_empty() {
        return RespValue::Error("ERR wrong number of arguments for 'hrandfield' command".to_string());
    }

    let key = match std::str::from_utf8(&args[0]) {
        Ok(s) => s,
        Err(_) => return RespValue::Error("ERR invalid key".to_string()),
    };

    let db_instance = match db.get_db(db_index) {
        Some(d) => d,
        None => return RespValue::Error("ERR invalid database".to_string()),
    };

    // Parse optional count and WITHVALUES parameters
    let count = if args.len() > 1 {
        match std::str::from_utf8(&args[1]) {
            Ok(s) => match s.parse::<i64>() {
                Ok(n) => Some(n),
                Err(_) => return RespValue::Error("ERR value is not an integer or out of range".to_string()),
            },
            Err(_) => return RespValue::Error("ERR value is not an integer or out of range".to_string()),
        }
    } else {
        None
    };

    let withvalues = if args.len() > 2 {
        if let Ok(s) = std::str::from_utf8(&args[2]) {
            s.to_uppercase() == "WITHVALUES"
        } else {
            false
        }
    } else {
        false
    };

    match db_instance.get(key) {
        Some(RedisValue::Hash(hash)) => {
            if hash.is_empty() {
                return if count.is_some() {
                    RespValue::Array(Some(vec![]))
                } else {
                    RespValue::BulkString(None)
                };
            }

            let fields: Vec<_> = hash.iter().collect();

            match count {
                None => {
                    // Return single random field (no count specified)
                    use rand::Rng;
                    let mut rng = rand::thread_rng();
                    let idx = rng.gen_range(0..fields.len());
                    if let Some((field, _)) = fields.get(idx) {
                        RespValue::BulkString(Some(field.to_vec()))
                    } else {
                        RespValue::BulkString(None)
                    }
                }
                Some(n) => {
                    // Return multiple random fields
                    use rand::seq::SliceRandom;
                    let mut rng = rand::thread_rng();

                    let abs_count = n.unsigned_abs() as usize;
                    let allow_duplicates = n < 0;

                    let mut results = Vec::new();

                    if allow_duplicates {
                        // Allow duplicates - just pick random items count times
                        for _ in 0..abs_count {
                            if let Some((field, value)) = fields.choose(&mut rng) {
                                results.push(RespValue::BulkString(Some(field.to_vec())));
                                if withvalues {
                                    results.push(RespValue::BulkString(Some(value.to_vec())));
                                }
                            }
                        }
                    } else {
                        // No duplicates - shuffle and take first N
                        let mut shuffled = fields.clone();
                        shuffled.shuffle(&mut rng);
                        let take_count = abs_count.min(shuffled.len());

                        for i in 0..take_count {
                            if let Some((field, value)) = shuffled.get(i) {
                                results.push(RespValue::BulkString(Some(field.to_vec())));
                                if withvalues {
                                    results.push(RespValue::BulkString(Some(value.to_vec())));
                                }
                            }
                        }
                    }

                    RespValue::Array(Some(results))
                }
            }
        }
        Some(_) => RespValue::Error(
            "WRONGTYPE Operation against a key holding the wrong kind of value".to_string(),
        ),
        None => {
            if count.is_some() {
                RespValue::Array(Some(vec![]))
            } else {
                RespValue::BulkString(None)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_hset_hget() {
        let db = Arc::new(Database::new(16));

        let result = hset(
            &db,
            0,
            vec![
                b"myhash".to_vec(),
                b"field1".to_vec(),
                b"value1".to_vec(),
            ],
        )
        .await;
        assert_eq!(result, RespValue::Integer(1));

        let result = hget(&db, 0, vec![b"myhash".to_vec(), b"field1".to_vec()]).await;
        assert_eq!(result, RespValue::BulkString(Some(b"value1".to_vec())));
    }

    #[tokio::test]
    async fn test_hgetall() {
        let db = Arc::new(Database::new(16));

        hset(
            &db,
            0,
            vec![
                b"myhash".to_vec(),
                b"field1".to_vec(),
                b"value1".to_vec(),
                b"field2".to_vec(),
                b"value2".to_vec(),
            ],
        )
        .await;

        let result = hgetall(&db, 0, vec![b"myhash".to_vec()]).await;
        if let RespValue::Array(Some(arr)) = result {
            assert_eq!(arr.len(), 4); // 2 fields * 2 (field + value)
        } else {
            panic!("Expected array");
        }
    }

    #[tokio::test]
    async fn test_hdel() {
        let db = Arc::new(Database::new(16));

        hset(
            &db,
            0,
            vec![
                b"myhash".to_vec(),
                b"field1".to_vec(),
                b"value1".to_vec(),
            ],
        )
        .await;

        let result = hdel(&db, 0, vec![b"myhash".to_vec(), b"field1".to_vec()]).await;
        assert_eq!(result, RespValue::Integer(1));

        let result = hexists(&db, 0, vec![b"myhash".to_vec(), b"field1".to_vec()]).await;
        assert_eq!(result, RespValue::Integer(0));
    }
}
