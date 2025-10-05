// Set command handlers

use crate::protocol::RespValue;
use crate::storage::db::Database;
use crate::storage::types::RedisValue;
use bytes::Bytes;
use std::collections::HashSet;
use std::sync::Arc;

/// SADD key member [member ...]
pub async fn sadd(db: &Arc<Database>, db_index: usize, args: Vec<Vec<u8>>) -> RespValue {
    if args.len() < 2 {
        return RespValue::Error("ERR wrong number of arguments for 'sadd' command".to_string());
    }

    let key = match std::str::from_utf8(&args[0]) {
        Ok(s) => s.to_string(),
        Err(_) => return RespValue::Error("ERR invalid key".to_string()),
    };

    let db_instance = match db.get_db(db_index) {
        Some(d) => d,
        None => return RespValue::Error("ERR invalid database".to_string()),
    };

    let mut set = match db_instance.get(&key) {
        Some(RedisValue::Set(s)) => s,
        Some(_) => {
            return RespValue::Error(
                "WRONGTYPE Operation against a key holding the wrong kind of value".to_string(),
            )
        }
        None => HashSet::new(),
    };

    let mut added = 0;
    for member in &args[1..] {
        if set.insert(Bytes::from(member.clone())) {
            added += 1;
        }
    }

    db_instance.set(key, RedisValue::Set(set));
    RespValue::Integer(added)
}

/// SREM key member [member ...]
pub async fn srem(db: &Arc<Database>, db_index: usize, args: Vec<Vec<u8>>) -> RespValue {
    if args.len() < 2 {
        return RespValue::Error("ERR wrong number of arguments for 'srem' command".to_string());
    }

    let key = match std::str::from_utf8(&args[0]) {
        Ok(s) => s.to_string(),
        Err(_) => return RespValue::Error("ERR invalid key".to_string()),
    };

    let db_instance = match db.get_db(db_index) {
        Some(d) => d,
        None => return RespValue::Error("ERR invalid database".to_string()),
    };

    let mut set = match db_instance.get(&key) {
        Some(RedisValue::Set(s)) => s,
        Some(_) => {
            return RespValue::Error(
                "WRONGTYPE Operation against a key holding the wrong kind of value".to_string(),
            )
        }
        None => return RespValue::Integer(0),
    };

    let mut removed = 0;
    for member in &args[1..] {
        if set.remove(&Bytes::from(member.clone())) {
            removed += 1;
        }
    }

    if set.is_empty() {
        db_instance.delete(&key);
    } else {
        db_instance.set(key, RedisValue::Set(set));
    }

    RespValue::Integer(removed)
}

/// SMEMBERS key
pub async fn smembers(db: &Arc<Database>, db_index: usize, args: Vec<Vec<u8>>) -> RespValue {
    if args.len() != 1 {
        return RespValue::Error("ERR wrong number of arguments for 'smembers' command".to_string());
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
        Some(RedisValue::Set(set)) => {
            let members: Vec<RespValue> = set
                .iter()
                .map(|m| RespValue::BulkString(Some(m.to_vec())))
                .collect();
            RespValue::Array(Some(members))
        }
        Some(_) => RespValue::Error(
            "WRONGTYPE Operation against a key holding the wrong kind of value".to_string(),
        ),
        None => RespValue::Array(Some(vec![])),
    }
}

/// SISMEMBER key member
pub async fn sismember(db: &Arc<Database>, db_index: usize, args: Vec<Vec<u8>>) -> RespValue {
    if args.len() != 2 {
        return RespValue::Error("ERR wrong number of arguments for 'sismember' command".to_string());
    }

    let key = match std::str::from_utf8(&args[0]) {
        Ok(s) => s,
        Err(_) => return RespValue::Error("ERR invalid key".to_string()),
    };

    let member = Bytes::from(args[1].clone());

    let db_instance = match db.get_db(db_index) {
        Some(d) => d,
        None => return RespValue::Error("ERR invalid database".to_string()),
    };

    match db_instance.get(key) {
        Some(RedisValue::Set(set)) => {
            RespValue::Integer(if set.contains(&member) { 1 } else { 0 })
        }
        Some(_) => RespValue::Error(
            "WRONGTYPE Operation against a key holding the wrong kind of value".to_string(),
        ),
        None => RespValue::Integer(0),
    }
}

/// SCARD key
pub async fn scard(db: &Arc<Database>, db_index: usize, args: Vec<Vec<u8>>) -> RespValue {
    if args.len() != 1 {
        return RespValue::Error("ERR wrong number of arguments for 'scard' command".to_string());
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
        Some(RedisValue::Set(set)) => RespValue::Integer(set.len() as i64),
        Some(_) => RespValue::Error(
            "WRONGTYPE Operation against a key holding the wrong kind of value".to_string(),
        ),
        None => RespValue::Integer(0),
    }
}

/// SPOP key [count]
pub async fn spop(db: &Arc<Database>, db_index: usize, args: Vec<Vec<u8>>) -> RespValue {
    if args.is_empty() || args.len() > 2 {
        return RespValue::Error("ERR wrong number of arguments for 'spop' command".to_string());
    }

    let key = match std::str::from_utf8(&args[0]) {
        Ok(s) => s.to_string(),
        Err(_) => return RespValue::Error("ERR invalid key".to_string()),
    };

    let count = if args.len() == 2 {
        match std::str::from_utf8(&args[1]) {
            Ok(s) => match s.parse::<usize>() {
                Ok(n) if n > 0 => Some(n),
                _ => {
                    return RespValue::Error(
                        "ERR value is out of range, must be positive".to_string(),
                    )
                }
            },
            Err(_) => {
                return RespValue::Error("ERR value is not an integer or out of range".to_string())
            }
        }
    } else {
        None
    };

    let db_instance = match db.get_db(db_index) {
        Some(d) => d,
        None => return RespValue::Error("ERR invalid database".to_string()),
    };

    let mut set = match db_instance.get(&key) {
        Some(RedisValue::Set(s)) => s,
        Some(_) => {
            return RespValue::Error(
                "WRONGTYPE Operation against a key holding the wrong kind of value".to_string(),
            )
        }
        None => return RespValue::BulkString(None),
    };

    if set.is_empty() {
        return RespValue::BulkString(None);
    }

    match count {
        None => {
            // Pop single element
            if let Some(member) = set.iter().next().cloned() {
                set.remove(&member);
                if set.is_empty() {
                    db_instance.delete(&key);
                } else {
                    db_instance.set(key, RedisValue::Set(set));
                }
                RespValue::BulkString(Some(member.to_vec()))
            } else {
                RespValue::BulkString(None)
            }
        }
        Some(n) => {
            // Pop multiple elements
            let mut popped = Vec::new();
            let mut members: Vec<_> = set.iter().cloned().collect();

            for _ in 0..n.min(members.len()) {
                if let Some(member) = members.pop() {
                    set.remove(&member);
                    popped.push(RespValue::BulkString(Some(member.to_vec())));
                }
            }

            if set.is_empty() {
                db_instance.delete(&key);
            } else {
                db_instance.set(key, RedisValue::Set(set));
            }

            RespValue::Array(Some(popped))
        }
    }
}

/// SRANDMEMBER key [count]
pub async fn srandmember(db: &Arc<Database>, db_index: usize, args: Vec<Vec<u8>>) -> RespValue {
    if args.is_empty() || args.len() > 2 {
        return RespValue::Error("ERR wrong number of arguments for 'srandmember' command".to_string());
    }

    let key = match std::str::from_utf8(&args[0]) {
        Ok(s) => s,
        Err(_) => return RespValue::Error("ERR invalid key".to_string()),
    };

    let count = if args.len() == 2 {
        match std::str::from_utf8(&args[1]) {
            Ok(s) => match s.parse::<i64>() {
                Ok(n) => Some(n),
                Err(_) => {
                    return RespValue::Error("ERR value is not an integer or out of range".to_string())
                }
            },
            Err(_) => {
                return RespValue::Error("ERR value is not an integer or out of range".to_string())
            }
        }
    } else {
        None
    };

    let db_instance = match db.get_db(db_index) {
        Some(d) => d,
        None => return RespValue::Error("ERR invalid database".to_string()),
    };

    match db_instance.get(key) {
        Some(RedisValue::Set(set)) => {
            if set.is_empty() {
                return if count.is_some() {
                    RespValue::Array(Some(vec![]))
                } else {
                    RespValue::BulkString(None)
                };
            }

            match count {
                None => {
                    // Return single random member
                    if let Some(member) = set.iter().next() {
                        RespValue::BulkString(Some(member.to_vec()))
                    } else {
                        RespValue::BulkString(None)
                    }
                }
                Some(n) => {
                    // Return multiple random members
                    let members: Vec<_> = set.iter().take(n.unsigned_abs() as usize).collect();
                    let result: Vec<RespValue> = members
                        .iter()
                        .map(|&m| RespValue::BulkString(Some(m.to_vec())))
                        .collect();
                    RespValue::Array(Some(result))
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

/// SINTER key [key ...]
pub async fn sinter(db: &Arc<Database>, db_index: usize, args: Vec<Vec<u8>>) -> RespValue {
    if args.is_empty() {
        return RespValue::Error("ERR wrong number of arguments for 'sinter' command".to_string());
    }

    let db_instance = match db.get_db(db_index) {
        Some(d) => d,
        None => return RespValue::Error("ERR invalid database".to_string()),
    };

    // Get first set
    let first_key = match std::str::from_utf8(&args[0]) {
        Ok(s) => s,
        Err(_) => return RespValue::Error("ERR invalid key".to_string()),
    };

    let mut result_set = match db_instance.get(first_key) {
        Some(RedisValue::Set(s)) => s,
        Some(_) => {
            return RespValue::Error(
                "WRONGTYPE Operation against a key holding the wrong kind of value".to_string(),
            )
        }
        None => return RespValue::Array(Some(vec![])),
    };

    // Intersect with remaining sets
    for key_bytes in &args[1..] {
        let key = match std::str::from_utf8(key_bytes) {
            Ok(s) => s,
            Err(_) => continue,
        };

        match db_instance.get(key) {
            Some(RedisValue::Set(set)) => {
                result_set = result_set.intersection(&set).cloned().collect();
            }
            Some(_) => {
                return RespValue::Error(
                    "WRONGTYPE Operation against a key holding the wrong kind of value".to_string(),
                )
            }
            None => return RespValue::Array(Some(vec![])),
        }

        if result_set.is_empty() {
            break;
        }
    }

    let members: Vec<RespValue> = result_set
        .iter()
        .map(|m| RespValue::BulkString(Some(m.to_vec())))
        .collect();

    RespValue::Array(Some(members))
}

/// SUNION key [key ...]
pub async fn sunion(db: &Arc<Database>, db_index: usize, args: Vec<Vec<u8>>) -> RespValue {
    if args.is_empty() {
        return RespValue::Error("ERR wrong number of arguments for 'sunion' command".to_string());
    }

    let db_instance = match db.get_db(db_index) {
        Some(d) => d,
        None => return RespValue::Error("ERR invalid database".to_string()),
    };

    let mut result_set = HashSet::new();

    for key_bytes in &args {
        let key = match std::str::from_utf8(key_bytes) {
            Ok(s) => s,
            Err(_) => continue,
        };

        match db_instance.get(key) {
            Some(RedisValue::Set(set)) => {
                result_set = result_set.union(&set).cloned().collect();
            }
            Some(_) => {
                return RespValue::Error(
                    "WRONGTYPE Operation against a key holding the wrong kind of value".to_string(),
                )
            }
            None => continue,
        }
    }

    let members: Vec<RespValue> = result_set
        .iter()
        .map(|m| RespValue::BulkString(Some(m.to_vec())))
        .collect();

    RespValue::Array(Some(members))
}

/// SDIFF key [key ...]
pub async fn sdiff(db: &Arc<Database>, db_index: usize, args: Vec<Vec<u8>>) -> RespValue {
    if args.is_empty() {
        return RespValue::Error("ERR wrong number of arguments for 'sdiff' command".to_string());
    }

    let db_instance = match db.get_db(db_index) {
        Some(d) => d,
        None => return RespValue::Error("ERR invalid database".to_string()),
    };

    // Get first set
    let first_key = match std::str::from_utf8(&args[0]) {
        Ok(s) => s,
        Err(_) => return RespValue::Error("ERR invalid key".to_string()),
    };

    let mut result_set = match db_instance.get(first_key) {
        Some(RedisValue::Set(s)) => s,
        Some(_) => {
            return RespValue::Error(
                "WRONGTYPE Operation against a key holding the wrong kind of value".to_string(),
            )
        }
        None => return RespValue::Array(Some(vec![])),
    };

    // Subtract remaining sets
    for key_bytes in &args[1..] {
        let key = match std::str::from_utf8(key_bytes) {
            Ok(s) => s,
            Err(_) => continue,
        };

        match db_instance.get(key) {
            Some(RedisValue::Set(set)) => {
                result_set = result_set.difference(&set).cloned().collect();
            }
            Some(_) => {
                return RespValue::Error(
                    "WRONGTYPE Operation against a key holding the wrong kind of value".to_string(),
                )
            }
            None => continue,
        }

        if result_set.is_empty() {
            break;
        }
    }

    let members: Vec<RespValue> = result_set
        .iter()
        .map(|m| RespValue::BulkString(Some(m.to_vec())))
        .collect();

    RespValue::Array(Some(members))
}

/// SINTERSTORE destination key [key ...]
pub async fn sinterstore(db: &Arc<Database>, db_index: usize, args: Vec<Vec<u8>>) -> RespValue {
    if args.len() < 2 {
        return RespValue::Error(
            "ERR wrong number of arguments for 'sinterstore' command".to_string(),
        );
    }

    let destination = match std::str::from_utf8(&args[0]) {
        Ok(s) => s.to_string(),
        Err(_) => return RespValue::Error("ERR invalid destination key".to_string()),
    };

    let db_instance = match db.get_db(db_index) {
        Some(d) => d,
        None => return RespValue::Error("ERR invalid database".to_string()),
    };

    // Get first set
    let first_key = match std::str::from_utf8(&args[1]) {
        Ok(s) => s,
        Err(_) => return RespValue::Error("ERR invalid key".to_string()),
    };

    let mut result_set = match db_instance.get(first_key) {
        Some(RedisValue::Set(s)) => s,
        Some(_) => {
            return RespValue::Error(
                "WRONGTYPE Operation against a key holding the wrong kind of value".to_string(),
            )
        }
        None => return RespValue::Integer(0),
    };

    // Intersect with remaining sets
    for key_bytes in &args[2..] {
        let key = match std::str::from_utf8(key_bytes) {
            Ok(s) => s,
            Err(_) => continue,
        };

        match db_instance.get(key) {
            Some(RedisValue::Set(set)) => {
                result_set = result_set.intersection(&set).cloned().collect();
            }
            Some(_) => {
                return RespValue::Error(
                    "WRONGTYPE Operation against a key holding the wrong kind of value".to_string(),
                )
            }
            None => return RespValue::Integer(0),
        }

        if result_set.is_empty() {
            break;
        }
    }

    let count = result_set.len();
    if count > 0 {
        db_instance.set(destination, RedisValue::Set(result_set));
    } else {
        db_instance.delete(&destination);
    }

    RespValue::Integer(count as i64)
}

/// SUNIONSTORE destination key [key ...]
pub async fn sunionstore(db: &Arc<Database>, db_index: usize, args: Vec<Vec<u8>>) -> RespValue {
    if args.len() < 2 {
        return RespValue::Error(
            "ERR wrong number of arguments for 'sunionstore' command".to_string(),
        );
    }

    let destination = match std::str::from_utf8(&args[0]) {
        Ok(s) => s.to_string(),
        Err(_) => return RespValue::Error("ERR invalid destination key".to_string()),
    };

    let db_instance = match db.get_db(db_index) {
        Some(d) => d,
        None => return RespValue::Error("ERR invalid database".to_string()),
    };

    let mut result_set = HashSet::new();

    for key_bytes in &args[1..] {
        let key = match std::str::from_utf8(key_bytes) {
            Ok(s) => s,
            Err(_) => continue,
        };

        match db_instance.get(key) {
            Some(RedisValue::Set(set)) => {
                result_set = result_set.union(&set).cloned().collect();
            }
            Some(_) => {
                return RespValue::Error(
                    "WRONGTYPE Operation against a key holding the wrong kind of value".to_string(),
                )
            }
            None => continue,
        }
    }

    let count = result_set.len();
    if count > 0 {
        db_instance.set(destination, RedisValue::Set(result_set));
    } else {
        db_instance.delete(&destination);
    }

    RespValue::Integer(count as i64)
}

/// SDIFFSTORE destination key [key ...]
pub async fn sdiffstore(db: &Arc<Database>, db_index: usize, args: Vec<Vec<u8>>) -> RespValue {
    if args.len() < 2 {
        return RespValue::Error(
            "ERR wrong number of arguments for 'sdiffstore' command".to_string(),
        );
    }

    let destination = match std::str::from_utf8(&args[0]) {
        Ok(s) => s.to_string(),
        Err(_) => return RespValue::Error("ERR invalid destination key".to_string()),
    };

    let db_instance = match db.get_db(db_index) {
        Some(d) => d,
        None => return RespValue::Error("ERR invalid database".to_string()),
    };

    // Get first set
    let first_key = match std::str::from_utf8(&args[1]) {
        Ok(s) => s,
        Err(_) => return RespValue::Error("ERR invalid key".to_string()),
    };

    let mut result_set = match db_instance.get(first_key) {
        Some(RedisValue::Set(s)) => s,
        Some(_) => {
            return RespValue::Error(
                "WRONGTYPE Operation against a key holding the wrong kind of value".to_string(),
            )
        }
        None => HashSet::new(),
    };

    // Subtract remaining sets
    for key_bytes in &args[2..] {
        let key = match std::str::from_utf8(key_bytes) {
            Ok(s) => s,
            Err(_) => continue,
        };

        match db_instance.get(key) {
            Some(RedisValue::Set(set)) => {
                result_set = result_set.difference(&set).cloned().collect();
            }
            Some(_) => {
                return RespValue::Error(
                    "WRONGTYPE Operation against a key holding the wrong kind of value".to_string(),
                )
            }
            None => continue,
        }

        if result_set.is_empty() {
            break;
        }
    }

    let count = result_set.len();
    if count > 0 {
        db_instance.set(destination, RedisValue::Set(result_set));
    } else {
        db_instance.delete(&destination);
    }

    RespValue::Integer(count as i64)
}

/// SMOVE source destination member
pub async fn smove(db: &Arc<Database>, db_index: usize, args: Vec<Vec<u8>>) -> RespValue {
    if args.len() != 3 {
        return RespValue::Error("ERR wrong number of arguments for 'smove' command".to_string());
    }

    let source = match std::str::from_utf8(&args[0]) {
        Ok(s) => s.to_string(),
        Err(_) => return RespValue::Error("ERR invalid source key".to_string()),
    };

    let destination = match std::str::from_utf8(&args[1]) {
        Ok(s) => s.to_string(),
        Err(_) => return RespValue::Error("ERR invalid destination key".to_string()),
    };

    let member = Bytes::from(args[2].clone());

    let db_instance = match db.get_db(db_index) {
        Some(d) => d,
        None => return RespValue::Error("ERR invalid database".to_string()),
    };

    // Remove from source
    let mut source_set = match db_instance.get(&source) {
        Some(RedisValue::Set(s)) => s,
        Some(_) => {
            return RespValue::Error(
                "WRONGTYPE Operation against a key holding the wrong kind of value".to_string(),
            )
        }
        None => return RespValue::Integer(0),
    };

    if !source_set.remove(&member) {
        return RespValue::Integer(0);
    }

    // Update or delete source
    if source_set.is_empty() {
        db_instance.delete(&source);
    } else {
        db_instance.set(source.clone(), RedisValue::Set(source_set));
    }

    // Add to destination
    let mut dest_set = match db_instance.get(&destination) {
        Some(RedisValue::Set(s)) => s,
        Some(_) => {
            return RespValue::Error(
                "WRONGTYPE Operation against a key holding the wrong kind of value".to_string(),
            )
        }
        None => HashSet::new(),
    };

    dest_set.insert(member);
    db_instance.set(destination, RedisValue::Set(dest_set));

    RespValue::Integer(1)
}

/// SMISMEMBER key member [member ...]
/// Check if multiple members exist in set
pub async fn smismember(db: &Arc<Database>, db_index: usize, args: Vec<Vec<u8>>) -> RespValue {
    if args.len() < 2 {
        return RespValue::Error("ERR wrong number of arguments for 'smismember' command".to_string());
    }

    let key = match std::str::from_utf8(&args[0]) {
        Ok(s) => s,
        Err(_) => return RespValue::Error("ERR invalid key".to_string()),
    };

    let db_instance = match db.get_db(db_index) {
        Some(d) => d,
        None => return RespValue::Error("ERR invalid database".to_string()),
    };

    let set = match db_instance.get(key) {
        Some(RedisValue::Set(s)) => s,
        Some(_) => {
            return RespValue::Error(
                "WRONGTYPE Operation against a key holding the wrong kind of value".to_string(),
            )
        }
        None => {
            // If set doesn't exist, all members are not present
            let results = vec![RespValue::Integer(0); args.len() - 1];
            return RespValue::Array(Some(results));
        }
    };

    let mut results = Vec::new();
    for member_bytes in &args[1..] {
        let member = Bytes::from(member_bytes.clone());
        let exists = set.contains(&member);
        results.push(RespValue::Integer(if exists { 1 } else { 0 }));
    }

    RespValue::Array(Some(results))
}

/// SSCAN key cursor [MATCH pattern] [COUNT count]
/// Incrementally iterate set members
pub async fn sscan(db: &Arc<Database>, db_index: usize, args: Vec<Vec<u8>>) -> RespValue {
    if args.len() < 2 {
        return RespValue::Error("ERR wrong number of arguments for 'sscan' command".to_string());
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
        Some(RedisValue::Set(set)) => {
            let members: Vec<_> = set.iter().collect();
            let start = cursor;
            let end = (start + count).min(members.len());
            let next_cursor = if end >= members.len() { 0 } else { end };

            // Build results array with members
            let mut results = Vec::new();
            for i in start..end {
                if let Some(member) = members.get(i) {
                    results.push(RespValue::BulkString(Some(member.to_vec())));
                }
            }

            // Return [next_cursor, [member1, member2, ...]]
            RespValue::Array(Some(vec![
                RespValue::BulkString(Some(next_cursor.to_string().into_bytes())),
                RespValue::Array(Some(results)),
            ]))
        }
        Some(_) => RespValue::Error(
            "WRONGTYPE Operation against a key holding the wrong kind of value".to_string(),
        ),
        None => {
            // Empty set - return cursor 0 and empty array
            RespValue::Array(Some(vec![
                RespValue::BulkString(Some(b"0".to_vec())),
                RespValue::Array(Some(vec![])),
            ]))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_sadd_smembers() {
        let db = Arc::new(Database::new(16));

        let result = sadd(&db, 0, vec![b"myset".to_vec(), b"a".to_vec(), b"b".to_vec(), b"c".to_vec()]).await;
        assert_eq!(result, RespValue::Integer(3));

        let result = smembers(&db, 0, vec![b"myset".to_vec()]).await;
        if let RespValue::Array(Some(arr)) = result {
            assert_eq!(arr.len(), 3);
        } else {
            panic!("Expected array");
        }
    }

    #[tokio::test]
    async fn test_srem() {
        let db = Arc::new(Database::new(16));

        sadd(&db, 0, vec![b"myset".to_vec(), b"a".to_vec(), b"b".to_vec()]).await;

        let result = srem(&db, 0, vec![b"myset".to_vec(), b"a".to_vec()]).await;
        assert_eq!(result, RespValue::Integer(1));

        let result = sismember(&db, 0, vec![b"myset".to_vec(), b"a".to_vec()]).await;
        assert_eq!(result, RespValue::Integer(0));
    }

    #[tokio::test]
    async fn test_sinter() {
        let db = Arc::new(Database::new(16));

        sadd(&db, 0, vec![b"set1".to_vec(), b"a".to_vec(), b"b".to_vec(), b"c".to_vec()]).await;
        sadd(&db, 0, vec![b"set2".to_vec(), b"b".to_vec(), b"c".to_vec(), b"d".to_vec()]).await;

        let result = sinter(&db, 0, vec![b"set1".to_vec(), b"set2".to_vec()]).await;
        if let RespValue::Array(Some(arr)) = result {
            assert_eq!(arr.len(), 2); // b and c
        } else {
            panic!("Expected array");
        }
    }

    #[tokio::test]
    async fn test_sunion() {
        let db = Arc::new(Database::new(16));

        sadd(&db, 0, vec![b"set1".to_vec(), b"a".to_vec(), b"b".to_vec()]).await;
        sadd(&db, 0, vec![b"set2".to_vec(), b"c".to_vec(), b"d".to_vec()]).await;

        let result = sunion(&db, 0, vec![b"set1".to_vec(), b"set2".to_vec()]).await;
        if let RespValue::Array(Some(arr)) = result {
            assert_eq!(arr.len(), 4); // a, b, c, d
        } else {
            panic!("Expected array");
        }
    }

    #[tokio::test]
    async fn test_sdiff() {
        let db = Arc::new(Database::new(16));

        sadd(&db, 0, vec![b"set1".to_vec(), b"a".to_vec(), b"b".to_vec(), b"c".to_vec()]).await;
        sadd(&db, 0, vec![b"set2".to_vec(), b"c".to_vec(), b"d".to_vec()]).await;

        let result = sdiff(&db, 0, vec![b"set1".to_vec(), b"set2".to_vec()]).await;
        if let RespValue::Array(Some(arr)) = result {
            assert_eq!(arr.len(), 2); // a and b
        } else {
            panic!("Expected array");
        }
    }
}
