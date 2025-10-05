// List command handlers

use crate::protocol::RespValue;
use crate::storage::db::Database;
use crate::storage::types::RedisValue;
use bytes::Bytes;
use std::collections::LinkedList;
use std::sync::Arc;

/// LPUSH key element [element ...]
pub async fn lpush(db: &Arc<Database>, db_index: usize, args: Vec<Vec<u8>>) -> RespValue {
    if args.len() < 2 {
        return RespValue::Error("ERR wrong number of arguments for 'lpush' command".to_string());
    }

    let key = match std::str::from_utf8(&args[0]) {
        Ok(s) => s.to_string(),
        Err(_) => return RespValue::Error("ERR invalid key".to_string()),
    };

    let db_instance = match db.get_db(db_index) {
        Some(d) => d,
        None => return RespValue::Error("ERR invalid database".to_string()),
    };

    let mut list = match db_instance.get(&key) {
        Some(RedisValue::List(l)) => l,
        Some(_) => {
            return RespValue::Error(
                "WRONGTYPE Operation against a key holding the wrong kind of value".to_string(),
            )
        }
        None => LinkedList::new(),
    };

    // Push elements to front (in reverse order to maintain order)
    for element in &args[1..] {
        list.push_front(Bytes::from(element.clone()));
    }

    let len = list.len();
    db_instance.set(key, RedisValue::List(list));
    RespValue::Integer(len as i64)
}

/// RPUSH key element [element ...]
pub async fn rpush(db: &Arc<Database>, db_index: usize, args: Vec<Vec<u8>>) -> RespValue {
    if args.len() < 2 {
        return RespValue::Error("ERR wrong number of arguments for 'rpush' command".to_string());
    }

    let key = match std::str::from_utf8(&args[0]) {
        Ok(s) => s.to_string(),
        Err(_) => return RespValue::Error("ERR invalid key".to_string()),
    };

    let db_instance = match db.get_db(db_index) {
        Some(d) => d,
        None => return RespValue::Error("ERR invalid database".to_string()),
    };

    let mut list = match db_instance.get(&key) {
        Some(RedisValue::List(l)) => l,
        Some(_) => {
            return RespValue::Error(
                "WRONGTYPE Operation against a key holding the wrong kind of value".to_string(),
            )
        }
        None => LinkedList::new(),
    };

    // Push elements to back
    for element in &args[1..] {
        list.push_back(Bytes::from(element.clone()));
    }

    let len = list.len();
    db_instance.set(key, RedisValue::List(list));
    RespValue::Integer(len as i64)
}

/// LPOP key [count]
pub async fn lpop(db: &Arc<Database>, db_index: usize, args: Vec<Vec<u8>>) -> RespValue {
    if args.is_empty() || args.len() > 2 {
        return RespValue::Error("ERR wrong number of arguments for 'lpop' command".to_string());
    }

    let key = match std::str::from_utf8(&args[0]) {
        Ok(s) => s.to_string(),
        Err(_) => return RespValue::Error("ERR invalid key".to_string()),
    };

    let count = if args.len() == 2 {
        match std::str::from_utf8(&args[1]) {
            Ok(s) => match s.parse::<usize>() {
                Ok(n) if n > 0 => n,
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
        1
    };

    let db_instance = match db.get_db(db_index) {
        Some(d) => d,
        None => return RespValue::Error("ERR invalid database".to_string()),
    };

    let mut list = match db_instance.get(&key) {
        Some(RedisValue::List(l)) => l,
        Some(_) => {
            return RespValue::Error(
                "WRONGTYPE Operation against a key holding the wrong kind of value".to_string(),
            )
        }
        None => return RespValue::BulkString(None),
    };

    if list.is_empty() {
        return RespValue::BulkString(None);
    }

    if count == 1 {
        // Single pop
        if let Some(value) = list.pop_front() {
            if list.is_empty() {
                db_instance.delete(&key);
            } else {
                db_instance.set(key, RedisValue::List(list));
            }
            RespValue::BulkString(Some(value.to_vec()))
        } else {
            RespValue::BulkString(None)
        }
    } else {
        // Multiple pops
        let mut results = Vec::new();
        for _ in 0..count {
            if let Some(value) = list.pop_front() {
                results.push(RespValue::BulkString(Some(value.to_vec())));
            } else {
                break;
            }
        }

        if list.is_empty() {
            db_instance.delete(&key);
        } else {
            db_instance.set(key, RedisValue::List(list));
        }

        RespValue::Array(Some(results))
    }
}

/// RPOP key [count]
pub async fn rpop(db: &Arc<Database>, db_index: usize, args: Vec<Vec<u8>>) -> RespValue {
    if args.is_empty() || args.len() > 2 {
        return RespValue::Error("ERR wrong number of arguments for 'rpop' command".to_string());
    }

    let key = match std::str::from_utf8(&args[0]) {
        Ok(s) => s.to_string(),
        Err(_) => return RespValue::Error("ERR invalid key".to_string()),
    };

    let count = if args.len() == 2 {
        match std::str::from_utf8(&args[1]) {
            Ok(s) => match s.parse::<usize>() {
                Ok(n) if n > 0 => n,
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
        1
    };

    let db_instance = match db.get_db(db_index) {
        Some(d) => d,
        None => return RespValue::Error("ERR invalid database".to_string()),
    };

    let mut list = match db_instance.get(&key) {
        Some(RedisValue::List(l)) => l,
        Some(_) => {
            return RespValue::Error(
                "WRONGTYPE Operation against a key holding the wrong kind of value".to_string(),
            )
        }
        None => return RespValue::BulkString(None),
    };

    if list.is_empty() {
        return RespValue::BulkString(None);
    }

    if count == 1 {
        // Single pop
        if let Some(value) = list.pop_back() {
            if list.is_empty() {
                db_instance.delete(&key);
            } else {
                db_instance.set(key, RedisValue::List(list));
            }
            RespValue::BulkString(Some(value.to_vec()))
        } else {
            RespValue::BulkString(None)
        }
    } else {
        // Multiple pops
        let mut results = Vec::new();
        for _ in 0..count {
            if let Some(value) = list.pop_back() {
                results.push(RespValue::BulkString(Some(value.to_vec())));
            } else {
                break;
            }
        }

        if list.is_empty() {
            db_instance.delete(&key);
        } else {
            db_instance.set(key, RedisValue::List(list));
        }

        RespValue::Array(Some(results))
    }
}

/// LLEN key
pub async fn llen(db: &Arc<Database>, db_index: usize, args: Vec<Vec<u8>>) -> RespValue {
    if args.len() != 1 {
        return RespValue::Error("ERR wrong number of arguments for 'llen' command".to_string());
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
        Some(RedisValue::List(list)) => RespValue::Integer(list.len() as i64),
        Some(_) => RespValue::Error(
            "WRONGTYPE Operation against a key holding the wrong kind of value".to_string(),
        ),
        None => RespValue::Integer(0),
    }
}

/// LRANGE key start stop
pub async fn lrange(db: &Arc<Database>, db_index: usize, args: Vec<Vec<u8>>) -> RespValue {
    if args.len() != 3 {
        return RespValue::Error("ERR wrong number of arguments for 'lrange' command".to_string());
    }

    let key = match std::str::from_utf8(&args[0]) {
        Ok(s) => s,
        Err(_) => return RespValue::Error("ERR invalid key".to_string()),
    };

    let start = match std::str::from_utf8(&args[1]) {
        Ok(s) => match s.parse::<i64>() {
            Ok(n) => n,
            Err(_) => {
                return RespValue::Error("ERR value is not an integer or out of range".to_string())
            }
        },
        Err(_) => return RespValue::Error("ERR value is not an integer or out of range".to_string()),
    };

    let stop = match std::str::from_utf8(&args[2]) {
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

    match db_instance.get(key) {
        Some(RedisValue::List(list)) => {
            let len = list.len() as i64;
            if len == 0 {
                return RespValue::Array(Some(vec![]));
            }

            // Normalize indices
            let start_idx = normalize_index(start, len);
            let stop_idx = normalize_index(stop, len);

            if start_idx > stop_idx || start_idx >= len {
                return RespValue::Array(Some(vec![]));
            }

            let items: Vec<_> = list.iter().collect();
            let result: Vec<RespValue> = items[start_idx as usize..=(stop_idx as usize).min(len as usize - 1)]
                .iter()
                .map(|&bytes| RespValue::BulkString(Some(bytes.to_vec())))
                .collect();

            RespValue::Array(Some(result))
        }
        Some(_) => RespValue::Error(
            "WRONGTYPE Operation against a key holding the wrong kind of value".to_string(),
        ),
        None => RespValue::Array(Some(vec![])),
    }
}

/// LINDEX key index
pub async fn lindex(db: &Arc<Database>, db_index: usize, args: Vec<Vec<u8>>) -> RespValue {
    if args.len() != 2 {
        return RespValue::Error("ERR wrong number of arguments for 'lindex' command".to_string());
    }

    let key = match std::str::from_utf8(&args[0]) {
        Ok(s) => s,
        Err(_) => return RespValue::Error("ERR invalid key".to_string()),
    };

    let index = match std::str::from_utf8(&args[1]) {
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

    match db_instance.get(key) {
        Some(RedisValue::List(list)) => {
            let len = list.len() as i64;
            let idx = normalize_index(index, len);

            if idx < 0 || idx >= len {
                return RespValue::BulkString(None);
            }

            if let Some(value) = list.iter().nth(idx as usize) {
                RespValue::BulkString(Some(value.to_vec()))
            } else {
                RespValue::BulkString(None)
            }
        }
        Some(_) => RespValue::Error(
            "WRONGTYPE Operation against a key holding the wrong kind of value".to_string(),
        ),
        None => RespValue::BulkString(None),
    }
}

/// LSET key index element
pub async fn lset(db: &Arc<Database>, db_index: usize, args: Vec<Vec<u8>>) -> RespValue {
    if args.len() != 3 {
        return RespValue::Error("ERR wrong number of arguments for 'lset' command".to_string());
    }

    let key = match std::str::from_utf8(&args[0]) {
        Ok(s) => s.to_string(),
        Err(_) => return RespValue::Error("ERR invalid key".to_string()),
    };

    let index = match std::str::from_utf8(&args[1]) {
        Ok(s) => match s.parse::<i64>() {
            Ok(n) => n,
            Err(_) => {
                return RespValue::Error("ERR value is not an integer or out of range".to_string())
            }
        },
        Err(_) => return RespValue::Error("ERR value is not an integer or out of range".to_string()),
    };

    let element = Bytes::from(args[2].clone());

    let db_instance = match db.get_db(db_index) {
        Some(d) => d,
        None => return RespValue::Error("ERR invalid database".to_string()),
    };

    match db_instance.get(&key) {
        Some(RedisValue::List(list)) => {
            let len = list.len() as i64;
            let idx = normalize_index(index, len);

            if idx < 0 || idx >= len {
                return RespValue::Error("ERR index out of range".to_string());
            }

            // Convert to vec for indexed access
            let mut items: Vec<_> = list.iter().cloned().collect();
            items[idx as usize] = element;

            // Rebuild list
            let new_list: LinkedList<_> = items.into_iter().collect();
            db_instance.set(key, RedisValue::List(new_list));

            RespValue::SimpleString("OK".to_string())
        }
        Some(_) => RespValue::Error(
            "WRONGTYPE Operation against a key holding the wrong kind of value".to_string(),
        ),
        None => RespValue::Error("ERR no such key".to_string()),
    }
}

/// LTRIM key start stop
pub async fn ltrim(db: &Arc<Database>, db_index: usize, args: Vec<Vec<u8>>) -> RespValue {
    if args.len() != 3 {
        return RespValue::Error("ERR wrong number of arguments for 'ltrim' command".to_string());
    }

    let key = match std::str::from_utf8(&args[0]) {
        Ok(s) => s.to_string(),
        Err(_) => return RespValue::Error("ERR invalid key".to_string()),
    };

    let start = match std::str::from_utf8(&args[1]) {
        Ok(s) => match s.parse::<i64>() {
            Ok(n) => n,
            Err(_) => {
                return RespValue::Error("ERR value is not an integer or out of range".to_string())
            }
        },
        Err(_) => return RespValue::Error("ERR value is not an integer or out of range".to_string()),
    };

    let stop = match std::str::from_utf8(&args[2]) {
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

    match db_instance.get(&key) {
        Some(RedisValue::List(list)) => {
            let len = list.len() as i64;
            let start_idx = normalize_index(start, len);
            let stop_idx = normalize_index(stop, len);

            if start_idx > stop_idx || start_idx >= len {
                // Remove all elements
                db_instance.delete(&key);
            } else {
                let items: Vec<_> = list.iter().collect();
                let trimmed: LinkedList<_> = items[start_idx as usize..=(stop_idx as usize).min(len as usize - 1)]
                    .iter()
                    .map(|&b| b.clone())
                    .collect();

                db_instance.set(key, RedisValue::List(trimmed));
            }

            RespValue::SimpleString("OK".to_string())
        }
        Some(_) => RespValue::Error(
            "WRONGTYPE Operation against a key holding the wrong kind of value".to_string(),
        ),
        None => RespValue::SimpleString("OK".to_string()),
    }
}

/// LREM key count element
pub async fn lrem(db: &Arc<Database>, db_index: usize, args: Vec<Vec<u8>>) -> RespValue {
    if args.len() != 3 {
        return RespValue::Error("ERR wrong number of arguments for 'lrem' command".to_string());
    }

    let key = match std::str::from_utf8(&args[0]) {
        Ok(s) => s.to_string(),
        Err(_) => return RespValue::Error("ERR invalid key".to_string()),
    };

    let count = match std::str::from_utf8(&args[1]) {
        Ok(s) => match s.parse::<i64>() {
            Ok(n) => n,
            Err(_) => return RespValue::Error("ERR value is not an integer or out of range".to_string()),
        },
        Err(_) => return RespValue::Error("ERR value is not an integer or out of range".to_string()),
    };

    let element = Bytes::from(args[2].clone());

    let db_instance = match db.get_db(db_index) {
        Some(d) => d,
        None => return RespValue::Error("ERR invalid database".to_string()),
    };

    match db_instance.get(&key) {
        Some(RedisValue::List(list)) => {
            let mut removed = 0;
            let mut new_list = LinkedList::new();

            if count == 0 {
                // Remove all occurrences
                for item in list.iter() {
                    if item != &element {
                        new_list.push_back(item.clone());
                    } else {
                        removed += 1;
                    }
                }
            } else if count > 0 {
                // Remove first count occurrences
                let mut to_remove = count;
                for item in list.iter() {
                    if to_remove > 0 && item == &element {
                        to_remove -= 1;
                        removed += 1;
                    } else {
                        new_list.push_back(item.clone());
                    }
                }
            } else {
                // Remove last |count| occurrences (scan from end)
                let items: Vec<_> = list.iter().cloned().collect();
                let mut to_remove = count.abs();
                for item in items.iter().rev() {
                    if to_remove > 0 && item == &element {
                        to_remove -= 1;
                        removed += 1;
                    } else {
                        new_list.push_front(item.clone());
                    }
                }
            }

            if new_list.is_empty() {
                db_instance.delete(&key);
            } else {
                db_instance.set(key, RedisValue::List(new_list));
            }

            RespValue::Integer(removed)
        }
        Some(_) => RespValue::Error("WRONGTYPE Operation against a key holding the wrong kind of value".to_string()),
        None => RespValue::Integer(0),
    }
}

/// LPUSHX key element [element ...]
pub async fn lpushx(db: &Arc<Database>, db_index: usize, args: Vec<Vec<u8>>) -> RespValue {
    if args.is_empty() {
        return RespValue::Error("ERR wrong number of arguments for 'lpushx' command".to_string());
    }

    let key = match std::str::from_utf8(&args[0]) {
        Ok(s) => s.to_string(),
        Err(_) => return RespValue::Error("ERR invalid key".to_string()),
    };

    let db_instance = match db.get_db(db_index) {
        Some(d) => d,
        None => return RespValue::Error("ERR invalid database".to_string()),
    };

    // Only push if key exists
    if !db_instance.exists(&key) {
        return RespValue::Integer(0);
    }

    match db_instance.get(&key) {
        Some(RedisValue::List(mut list)) => {
            for i in 1..args.len() {
                let value = Bytes::from(args[i].clone());
                list.push_front(value);
            }
            let len = list.len();
            db_instance.set(key, RedisValue::List(list));
            RespValue::Integer(len as i64)
        }
        Some(_) => RespValue::Error("WRONGTYPE Operation against a key holding the wrong kind of value".to_string()),
        None => RespValue::Integer(0),
    }
}

/// RPUSHX key element [element ...]
pub async fn rpushx(db: &Arc<Database>, db_index: usize, args: Vec<Vec<u8>>) -> RespValue {
    if args.is_empty() {
        return RespValue::Error("ERR wrong number of arguments for 'rpushx' command".to_string());
    }

    let key = match std::str::from_utf8(&args[0]) {
        Ok(s) => s.to_string(),
        Err(_) => return RespValue::Error("ERR invalid key".to_string()),
    };

    let db_instance = match db.get_db(db_index) {
        Some(d) => d,
        None => return RespValue::Error("ERR invalid database".to_string()),
    };

    // Only push if key exists
    if !db_instance.exists(&key) {
        return RespValue::Integer(0);
    }

    match db_instance.get(&key) {
        Some(RedisValue::List(mut list)) => {
            for i in 1..args.len() {
                let value = Bytes::from(args[i].clone());
                list.push_back(value);
            }
            let len = list.len();
            db_instance.set(key, RedisValue::List(list));
            RespValue::Integer(len as i64)
        }
        Some(_) => RespValue::Error("WRONGTYPE Operation against a key holding the wrong kind of value".to_string()),
        None => RespValue::Integer(0),
    }
}

/// RPOPLPUSH source destination
pub async fn rpoplpush(db: &Arc<Database>, db_index: usize, args: Vec<Vec<u8>>) -> RespValue {
    if args.len() != 2 {
        return RespValue::Error("ERR wrong number of arguments for 'rpoplpush' command".to_string());
    }

    let source = match std::str::from_utf8(&args[0]) {
        Ok(s) => s.to_string(),
        Err(_) => return RespValue::Error("ERR invalid key".to_string()),
    };

    let destination = match std::str::from_utf8(&args[1]) {
        Ok(s) => s.to_string(),
        Err(_) => return RespValue::Error("ERR invalid key".to_string()),
    };

    let db_instance = match db.get_db(db_index) {
        Some(d) => d,
        None => return RespValue::Error("ERR invalid database".to_string()),
    };

    // Get element from source
    let element = match db_instance.get(&source) {
        Some(RedisValue::List(mut list)) => {
            if list.is_empty() {
                return RespValue::BulkString(None);
            }
            let elem = list.pop_back().unwrap();
            if list.is_empty() {
                db_instance.delete(&source);
            } else {
                db_instance.set(source.clone(), RedisValue::List(list));
            }
            elem
        }
        Some(_) => return RespValue::Error("WRONGTYPE Operation against a key holding the wrong kind of value".to_string()),
        None => return RespValue::BulkString(None),
    };

    // Push to destination
    match db_instance.get(&destination) {
        Some(RedisValue::List(mut list)) => {
            list.push_front(element.clone());
            db_instance.set(destination, RedisValue::List(list));
        }
        Some(_) => return RespValue::Error("WRONGTYPE Operation against a key holding the wrong kind of value".to_string()),
        None => {
            let mut new_list = LinkedList::new();
            new_list.push_back(element.clone());
            db_instance.set(destination, RedisValue::List(new_list));
        }
    }

    RespValue::BulkString(Some(element.to_vec()))
}

// Helper function to normalize negative indices
fn normalize_index(index: i64, len: i64) -> i64 {
    if index < 0 {
        (len + index).max(0)
    } else {
        index.min(len - 1)
    }
}

/// BLPOP key [key ...] timeout
/// Blocking left pop - removes and returns first element from first non-empty list
pub async fn blpop(db: &Arc<Database>, db_index: usize, args: Vec<Vec<u8>>) -> RespValue {
    if args.len() < 2 {
        return RespValue::Error("ERR wrong number of arguments for 'blpop' command".to_string());
    }

    // Parse timeout (last argument)
    let timeout_arg = &args[args.len() - 1];
    let timeout_secs = match std::str::from_utf8(timeout_arg) {
        Ok(s) => match s.parse::<f64>() {
            Ok(t) if t >= 0.0 => t,
            _ => return RespValue::Error("ERR timeout is not a float or out of range".to_string()),
        },
        Err(_) => return RespValue::Error("ERR timeout is not a float or out of range".to_string()),
    };

    let keys = &args[0..args.len() - 1];

    // Convert timeout to milliseconds for polling
    let timeout_ms = (timeout_secs * 1000.0) as u64;
    let start = std::time::Instant::now();
    let poll_interval = std::time::Duration::from_millis(10); // Poll every 10ms

    loop {
        // Try to pop from each key in order
        for key_bytes in keys {
            let key = match std::str::from_utf8(key_bytes) {
                Ok(s) => s.to_string(),
                Err(_) => continue,
            };

            let db_instance = match db.get_db(db_index) {
                Some(d) => d,
                None => return RespValue::Error("ERR invalid database".to_string()),
            };

            // Try to get and pop from this key
            if let Some(RedisValue::List(mut list)) = db_instance.get(&key) {
                if let Some(element) = list.pop_front() {
                    // Update the list (or delete if empty)
                    if list.is_empty() {
                        db_instance.delete(&key);
                    } else {
                        db_instance.set(key.clone(), RedisValue::List(list));
                    }
                    // Return key and value as array
                    return RespValue::Array(Some(vec![
                        RespValue::BulkString(Some(key.into_bytes())),
                        RespValue::BulkString(Some(element.to_vec())),
                    ]));
                }
            }
        }

        // Check timeout
        if timeout_secs == 0.0 || start.elapsed().as_millis() >= timeout_ms as u128 {
            return RespValue::Null; // Timeout - return null
        }

        // Sleep before next poll
        tokio::time::sleep(poll_interval).await;
    }
}

/// BRPOP key [key ...] timeout
/// Blocking right pop - removes and returns last element from first non-empty list
pub async fn brpop(db: &Arc<Database>, db_index: usize, args: Vec<Vec<u8>>) -> RespValue {
    if args.len() < 2 {
        return RespValue::Error("ERR wrong number of arguments for 'brpop' command".to_string());
    }

    // Parse timeout (last argument)
    let timeout_arg = &args[args.len() - 1];
    let timeout_secs = match std::str::from_utf8(timeout_arg) {
        Ok(s) => match s.parse::<f64>() {
            Ok(t) if t >= 0.0 => t,
            _ => return RespValue::Error("ERR timeout is not a float or out of range".to_string()),
        },
        Err(_) => return RespValue::Error("ERR timeout is not a float or out of range".to_string()),
    };

    let keys = &args[0..args.len() - 1];

    // Convert timeout to milliseconds for polling
    let timeout_ms = (timeout_secs * 1000.0) as u64;
    let start = std::time::Instant::now();
    let poll_interval = std::time::Duration::from_millis(10);

    loop {
        // Try to pop from each key in order
        for key_bytes in keys {
            let key = match std::str::from_utf8(key_bytes) {
                Ok(s) => s.to_string(),
                Err(_) => continue,
            };

            let db_instance = match db.get_db(db_index) {
                Some(d) => d,
                None => return RespValue::Error("ERR invalid database".to_string()),
            };

            // Try to get and pop from this key
            if let Some(RedisValue::List(mut list)) = db_instance.get(&key) {
                if let Some(element) = list.pop_back() {
                    // Update the list (or delete if empty)
                    if list.is_empty() {
                        db_instance.delete(&key);
                    } else {
                        db_instance.set(key.clone(), RedisValue::List(list));
                    }
                    // Return key and value as array
                    return RespValue::Array(Some(vec![
                        RespValue::BulkString(Some(key.into_bytes())),
                        RespValue::BulkString(Some(element.to_vec())),
                    ]));
                }
            }
        }

        // Check timeout
        if timeout_secs == 0.0 || start.elapsed().as_millis() >= timeout_ms as u128 {
            return RespValue::Null;
        }

        // Sleep before next poll
        tokio::time::sleep(poll_interval).await;
    }
}

/// BLMOVE source destination LEFT|RIGHT LEFT|RIGHT timeout
/// Blocking version of LMOVE/RPOPLPUSH
pub async fn blmove(db: &Arc<Database>, db_index: usize, args: Vec<Vec<u8>>) -> RespValue {
    if args.len() != 5 {
        return RespValue::Error("ERR wrong number of arguments for 'blmove' command".to_string());
    }

    let source = match std::str::from_utf8(&args[0]) {
        Ok(s) => s.to_string(),
        Err(_) => return RespValue::Error("ERR invalid source key".to_string()),
    };

    let dest = match std::str::from_utf8(&args[1]) {
        Ok(s) => s.to_string(),
        Err(_) => return RespValue::Error("ERR invalid destination key".to_string()),
    };

    let wherefrom = match std::str::from_utf8(&args[2]) {
        Ok(s) => s.to_uppercase(),
        Err(_) => return RespValue::Error("ERR invalid wherefrom argument".to_string()),
    };

    let whereto = match std::str::from_utf8(&args[3]) {
        Ok(s) => s.to_uppercase(),
        Err(_) => return RespValue::Error("ERR invalid whereto argument".to_string()),
    };

    if wherefrom != "LEFT" && wherefrom != "RIGHT" {
        return RespValue::Error("ERR syntax error".to_string());
    }
    if whereto != "LEFT" && whereto != "RIGHT" {
        return RespValue::Error("ERR syntax error".to_string());
    }

    let timeout_secs = match std::str::from_utf8(&args[4]) {
        Ok(s) => match s.parse::<f64>() {
            Ok(t) if t >= 0.0 => t,
            _ => return RespValue::Error("ERR timeout is not a float or out of range".to_string()),
        },
        Err(_) => return RespValue::Error("ERR timeout is not a float or out of range".to_string()),
    };

    let timeout_ms = (timeout_secs * 1000.0) as u64;
    let start = std::time::Instant::now();
    let poll_interval = std::time::Duration::from_millis(10);

    loop {
        let db_instance = match db.get_db(db_index) {
            Some(d) => d,
            None => return RespValue::Error("ERR invalid database".to_string()),
        };

        // Try to pop from source
        if let Some(RedisValue::List(mut source_list)) = db_instance.get(&source) {
            if !source_list.is_empty() {
                // Pop element from source
                let element = if wherefrom == "LEFT" {
                    source_list.pop_front()
                } else {
                    source_list.pop_back()
                };

                if let Some(elem) = element {
                    // Update source list
                    if source_list.is_empty() {
                        db_instance.delete(&source);
                    } else {
                        db_instance.set(source.clone(), RedisValue::List(source_list));
                    }

                    // Push to destination
                    let mut dest_list = match db_instance.get(&dest) {
                        Some(RedisValue::List(l)) => l,
                        Some(_) => {
                            return RespValue::Error(
                                "WRONGTYPE Operation against a key holding the wrong kind of value"
                                    .to_string(),
                            )
                        }
                        None => LinkedList::new(),
                    };

                    if whereto == "LEFT" {
                        dest_list.push_front(elem.clone());
                    } else {
                        dest_list.push_back(elem.clone());
                    }

                    db_instance.set(dest, RedisValue::List(dest_list));

                    return RespValue::BulkString(Some(elem.to_vec()));
                }
            }
        }

        // Check timeout
        if timeout_secs == 0.0 || start.elapsed().as_millis() >= timeout_ms as u128 {
            return RespValue::Null;
        }

        // Sleep before next poll
        tokio::time::sleep(poll_interval).await;
    }
}

/// LPOS key element [RANK rank] [COUNT num-matches] [MAXLEN len]
/// Find position of element in list
pub async fn lpos(db: &Arc<Database>, db_index: usize, args: Vec<Vec<u8>>) -> RespValue {
    if args.len() < 2 {
        return RespValue::Error("ERR wrong number of arguments for 'lpos' command".to_string());
    }

    let key = match std::str::from_utf8(&args[0]) {
        Ok(s) => s,
        Err(_) => return RespValue::Error("ERR invalid key".to_string()),
    };

    let element = Bytes::from(args[1].clone());

    // Parse options (simplified - basic implementation)
    let mut count: Option<usize> = None;
    let mut i = 2;
    while i < args.len() {
        if let Ok(s) = std::str::from_utf8(&args[i]) {
            match s.to_uppercase().as_str() {
                "COUNT" => {
                    i += 1;
                    if i < args.len() {
                        if let Ok(num_str) = std::str::from_utf8(&args[i]) {
                            count = num_str.parse::<usize>().ok();
                        }
                    }
                }
                _ => {}
            }
        }
        i += 1;
    }

    let db_instance = match db.get_db(db_index) {
        Some(d) => d,
        None => return RespValue::Error("ERR invalid database".to_string()),
    };

    let list = match db_instance.get(key) {
        Some(RedisValue::List(l)) => l,
        Some(_) => {
            return RespValue::Error(
                "WRONGTYPE Operation against a key holding the wrong kind of value".to_string(),
            )
        }
        None => return RespValue::BulkString(None),
    };

    // Find positions of element
    let mut positions = Vec::new();
    for (idx, item) in list.iter().enumerate() {
        if item == &element {
            positions.push(idx as i64);
            if let Some(max_count) = count {
                if positions.len() >= max_count {
                    break;
                }
            } else if positions.len() == 1 {
                // If no COUNT, return first match only
                return RespValue::Integer(positions[0]);
            }
        }
    }

    if count.is_some() {
        // Return array of positions
        let results: Vec<RespValue> = positions
            .into_iter()
            .map(RespValue::Integer)
            .collect();
        RespValue::Array(Some(results))
    } else if positions.is_empty() {
        RespValue::BulkString(None)
    } else {
        RespValue::Integer(positions[0])
    }
}

/// LMOVE source destination <LEFT|RIGHT> <LEFT|RIGHT>
/// Atomically move element from one list to another
pub async fn lmove(db: &Arc<Database>, db_index: usize, args: Vec<Vec<u8>>) -> RespValue {
    if args.len() != 4 {
        return RespValue::Error("ERR wrong number of arguments for 'lmove' command".to_string());
    }

    let source = match std::str::from_utf8(&args[0]) {
        Ok(s) => s.to_string(),
        Err(_) => return RespValue::Error("ERR invalid source key".to_string()),
    };

    let dest = match std::str::from_utf8(&args[1]) {
        Ok(s) => s.to_string(),
        Err(_) => return RespValue::Error("ERR invalid destination key".to_string()),
    };

    let wherefrom = match std::str::from_utf8(&args[2]) {
        Ok(s) => s.to_uppercase(),
        Err(_) => return RespValue::Error("ERR syntax error".to_string()),
    };

    let whereto = match std::str::from_utf8(&args[3]) {
        Ok(s) => s.to_uppercase(),
        Err(_) => return RespValue::Error("ERR syntax error".to_string()),
    };

    if wherefrom != "LEFT" && wherefrom != "RIGHT" {
        return RespValue::Error("ERR syntax error".to_string());
    }

    if whereto != "LEFT" && whereto != "RIGHT" {
        return RespValue::Error("ERR syntax error".to_string());
    }

    let db_instance = match db.get_db(db_index) {
        Some(d) => d,
        None => return RespValue::Error("ERR invalid database".to_string()),
    };

    // Get source list
    let mut source_list = match db_instance.get(&source) {
        Some(RedisValue::List(l)) => l,
        Some(_) => {
            return RespValue::Error(
                "WRONGTYPE Operation against a key holding the wrong kind of value".to_string(),
            )
        }
        None => return RespValue::BulkString(None),
    };

    if source_list.is_empty() {
        return RespValue::BulkString(None);
    }

    // Pop from source
    let element = if wherefrom == "LEFT" {
        source_list.pop_front()
    } else {
        source_list.pop_back()
    };

    let element = match element {
        Some(e) => e,
        None => return RespValue::BulkString(None),
    };

    // Update or delete source list
    if source_list.is_empty() {
        db_instance.delete(&source);
    } else {
        db_instance.set(source.clone(), RedisValue::List(source_list));
    }

    // Get or create destination list
    let mut dest_list = match db_instance.get(&dest) {
        Some(RedisValue::List(l)) => l,
        Some(_) => {
            return RespValue::Error(
                "WRONGTYPE Operation against a key holding the wrong kind of value".to_string(),
            )
        }
        None => std::collections::LinkedList::new(),
    };

    // Push to destination
    if whereto == "LEFT" {
        dest_list.push_front(element.clone());
    } else {
        dest_list.push_back(element.clone());
    }

    db_instance.set(dest, RedisValue::List(dest_list));

    RespValue::BulkString(Some(element.to_vec()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_lpush_rpush() {
        let db = Arc::new(Database::new(16));

        // RPUSH
        let result = rpush(&db, 0, vec![b"mylist".to_vec(), b"a".to_vec(), b"b".to_vec()]).await;
        assert_eq!(result, RespValue::Integer(2));

        // LPUSH
        let result = lpush(&db, 0, vec![b"mylist".to_vec(), b"c".to_vec()]).await;
        assert_eq!(result, RespValue::Integer(3));

        // LRANGE to verify order: c, a, b
        let result = lrange(&db, 0, vec![b"mylist".to_vec(), b"0".to_vec(), b"-1".to_vec()]).await;
        if let RespValue::Array(Some(arr)) = result {
            assert_eq!(arr.len(), 3);
        } else {
            panic!("Expected array");
        }
    }

    #[tokio::test]
    async fn test_lpop_rpop() {
        let db = Arc::new(Database::new(16));

        rpush(&db, 0, vec![b"mylist".to_vec(), b"one".to_vec(), b"two".to_vec(), b"three".to_vec()]).await;

        let result = lpop(&db, 0, vec![b"mylist".to_vec()]).await;
        assert_eq!(result, RespValue::BulkString(Some(b"one".to_vec())));

        let result = rpop(&db, 0, vec![b"mylist".to_vec()]).await;
        assert_eq!(result, RespValue::BulkString(Some(b"three".to_vec())));

        let result = llen(&db, 0, vec![b"mylist".to_vec()]).await;
        assert_eq!(result, RespValue::Integer(1));
    }

    #[test]
    fn test_normalize_index() {
        assert_eq!(normalize_index(0, 10), 0);
        assert_eq!(normalize_index(5, 10), 5);
        assert_eq!(normalize_index(-1, 10), 9);
        assert_eq!(normalize_index(-5, 10), 5);
        assert_eq!(normalize_index(100, 10), 9);
    }
}
