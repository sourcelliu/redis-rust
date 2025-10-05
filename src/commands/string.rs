// String command handlers

use crate::protocol::RespValue;
use crate::storage::db::Database;
use crate::storage::types::RedisValue;
use bytes::Bytes;
use std::sync::Arc;
use std::time::Duration;

/// SET key value [EX seconds] [PX milliseconds] [EXAT unix-time-seconds] [PXAT unix-time-milliseconds] [NX|XX] [KEEPTTL] [GET]
pub async fn set(db: &Arc<Database>, db_index: usize, args: Vec<Vec<u8>>) -> RespValue {
    if args.len() < 2 {
        return RespValue::Error("ERR wrong number of arguments for 'set' command".to_string());
    }

    let key = match std::str::from_utf8(&args[0]) {
        Ok(s) => s.to_string(),
        Err(_) => return RespValue::Error("ERR invalid key".to_string()),
    };

    let value = Bytes::from(args[1].clone());

    // Parse options
    let mut expiration: Option<Duration> = None;
    let mut nx = false; // Only set if key doesn't exist
    let mut xx = false; // Only set if key exists
    let mut keep_ttl = false;
    let mut get = false; // Return old value

    let mut i = 2;
    while i < args.len() {
        let option = match std::str::from_utf8(&args[i]) {
            Ok(s) => s.to_uppercase(),
            Err(_) => return RespValue::Error("ERR invalid option".to_string()),
        };

        match option.as_str() {
            "EX" => {
                // Expiration in seconds
                if i + 1 >= args.len() {
                    return RespValue::Error("ERR syntax error".to_string());
                }
                let seconds = match std::str::from_utf8(&args[i + 1]) {
                    Ok(s) => match s.parse::<u64>() {
                        Ok(n) => n,
                        Err(_) => return RespValue::Error("ERR value is not an integer or out of range".to_string()),
                    },
                    Err(_) => return RespValue::Error("ERR value is not an integer or out of range".to_string()),
                };
                expiration = Some(Duration::from_secs(seconds));
                i += 2;
            }
            "PX" => {
                // Expiration in milliseconds
                if i + 1 >= args.len() {
                    return RespValue::Error("ERR syntax error".to_string());
                }
                let millis = match std::str::from_utf8(&args[i + 1]) {
                    Ok(s) => match s.parse::<u64>() {
                        Ok(n) => n,
                        Err(_) => return RespValue::Error("ERR value is not an integer or out of range".to_string()),
                    },
                    Err(_) => return RespValue::Error("ERR value is not an integer or out of range".to_string()),
                };
                expiration = Some(Duration::from_millis(millis));
                i += 2;
            }
            "EXAT" => {
                // Absolute expiration time in seconds
                if i + 1 >= args.len() {
                    return RespValue::Error("ERR syntax error".to_string());
                }
                let timestamp = match std::str::from_utf8(&args[i + 1]) {
                    Ok(s) => match s.parse::<u64>() {
                        Ok(n) => n,
                        Err(_) => return RespValue::Error("ERR value is not an integer or out of range".to_string()),
                    },
                    Err(_) => return RespValue::Error("ERR value is not an integer or out of range".to_string()),
                };
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_secs();
                if timestamp > now {
                    expiration = Some(Duration::from_secs(timestamp - now));
                } else {
                    expiration = Some(Duration::from_secs(0));
                }
                i += 2;
            }
            "PXAT" => {
                // Absolute expiration time in milliseconds
                if i + 1 >= args.len() {
                    return RespValue::Error("ERR syntax error".to_string());
                }
                let timestamp = match std::str::from_utf8(&args[i + 1]) {
                    Ok(s) => match s.parse::<u64>() {
                        Ok(n) => n,
                        Err(_) => return RespValue::Error("ERR value is not an integer or out of range".to_string()),
                    },
                    Err(_) => return RespValue::Error("ERR value is not an integer or out of range".to_string()),
                };
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_millis() as u64;
                if timestamp > now {
                    expiration = Some(Duration::from_millis(timestamp - now));
                } else {
                    expiration = Some(Duration::from_millis(0));
                }
                i += 2;
            }
            "NX" => {
                nx = true;
                i += 1;
            }
            "XX" => {
                xx = true;
                i += 1;
            }
            "KEEPTTL" => {
                keep_ttl = true;
                i += 1;
            }
            "GET" => {
                get = true;
                i += 1;
            }
            _ => {
                return RespValue::Error(format!("ERR syntax error"));
            }
        }
    }

    // NX and XX are mutually exclusive
    if nx && xx {
        return RespValue::Error("ERR NX and XX options at the same time are not compatible".to_string());
    }

    let db_instance = match db.get_db(db_index) {
        Some(d) => d,
        None => return RespValue::Error("ERR invalid database".to_string()),
    };

    // Get old value if GET option is specified
    let old_value = if get {
        match db_instance.get(&key) {
            Some(RedisValue::String(bytes)) => Some(bytes.to_vec()),
            Some(_) => return RespValue::Error("WRONGTYPE Operation against a key holding the wrong kind of value".to_string()),
            None => None,
        }
    } else {
        None
    };

    // Check NX condition (only set if key doesn't exist)
    if nx && db_instance.exists(&key) {
        return if get {
            match old_value {
                Some(v) => RespValue::BulkString(Some(v)),
                None => RespValue::BulkString(None),
            }
        } else {
            RespValue::BulkString(None)
        };
    }

    // Check XX condition (only set if key exists)
    if xx && !db_instance.exists(&key) {
        return if get {
            RespValue::BulkString(None)
        } else {
            RespValue::BulkString(None)
        };
    }

    // Set the value
    db_instance.set(key.clone(), RedisValue::String(value));

    // Set expiration if specified
    if let Some(duration) = expiration {
        if !keep_ttl {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64;
            let expire_at_ms = now + duration.as_millis() as u64;
            db_instance.set_expiry(&key, expire_at_ms);
        }
    } else if !keep_ttl {
        // Clear expiration if not KEEPTTL and no new expiration
        db_instance.persist(&key);
    }

    // Return response
    if get {
        match old_value {
            Some(v) => RespValue::BulkString(Some(v)),
            None => RespValue::BulkString(None),
        }
    } else {
        RespValue::SimpleString("OK".to_string())
    }
}

/// GET key
pub async fn get(db: &Arc<Database>, db_index: usize, args: Vec<Vec<u8>>) -> RespValue {
    if args.len() != 1 {
        return RespValue::Error("ERR wrong number of arguments for 'get' command".to_string());
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
        Some(RedisValue::String(bytes)) => RespValue::BulkString(Some(bytes.to_vec())),
        Some(_) => RespValue::Error("WRONGTYPE Operation against a key holding the wrong kind of value".to_string()),
        None => RespValue::BulkString(None),
    }
}

/// DEL key [key ...]
pub async fn del(db: &Arc<Database>, db_index: usize, args: Vec<Vec<u8>>) -> RespValue {
    if args.is_empty() {
        return RespValue::Error("ERR wrong number of arguments for 'del' command".to_string());
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

/// EXISTS key [key ...]
pub async fn exists(db: &Arc<Database>, db_index: usize, args: Vec<Vec<u8>>) -> RespValue {
    if args.is_empty() {
        return RespValue::Error("ERR wrong number of arguments for 'exists' command".to_string());
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

        if db_instance.exists(key) {
            count += 1;
        }
    }

    RespValue::Integer(count)
}

/// APPEND key value
pub async fn append(db: &Arc<Database>, db_index: usize, args: Vec<Vec<u8>>) -> RespValue {
    if args.len() != 2 {
        return RespValue::Error("ERR wrong number of arguments for 'append' command".to_string());
    }

    let key = match std::str::from_utf8(&args[0]) {
        Ok(s) => s.to_string(),
        Err(_) => return RespValue::Error("ERR invalid key".to_string()),
    };

    let append_value = Bytes::from(args[1].clone());

    let db_instance = match db.get_db(db_index) {
        Some(d) => d,
        None => return RespValue::Error("ERR invalid database".to_string()),
    };

    match db_instance.get(&key) {
        Some(RedisValue::String(mut current)) => {
            let mut new_vec = current.to_vec();
            new_vec.extend_from_slice(&append_value);
            let new_bytes = Bytes::from(new_vec);
            let len = new_bytes.len();
            db_instance.set(key, RedisValue::String(new_bytes));
            RespValue::Integer(len as i64)
        }
        Some(_) => RespValue::Error("WRONGTYPE Operation against a key holding the wrong kind of value".to_string()),
        None => {
            let len = append_value.len();
            db_instance.set(key, RedisValue::String(append_value));
            RespValue::Integer(len as i64)
        }
    }
}

/// STRLEN key
pub async fn strlen(db: &Arc<Database>, db_index: usize, args: Vec<Vec<u8>>) -> RespValue {
    if args.len() != 1 {
        return RespValue::Error("ERR wrong number of arguments for 'strlen' command".to_string());
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
        Some(RedisValue::String(bytes)) => RespValue::Integer(bytes.len() as i64),
        Some(_) => RespValue::Error("WRONGTYPE Operation against a key holding the wrong kind of value".to_string()),
        None => RespValue::Integer(0),
    }
}

/// INCR key
pub async fn incr(db: &Arc<Database>, db_index: usize, args: Vec<Vec<u8>>) -> RespValue {
    incrby(db, db_index, vec![args[0].clone(), b"1".to_vec()]).await
}

/// DECR key
pub async fn decr(db: &Arc<Database>, db_index: usize, args: Vec<Vec<u8>>) -> RespValue {
    decrby(db, db_index, vec![args[0].clone(), b"1".to_vec()]).await
}

/// INCRBY key increment
pub async fn incrby(db: &Arc<Database>, db_index: usize, args: Vec<Vec<u8>>) -> RespValue {
    if args.len() != 2 {
        return RespValue::Error("ERR wrong number of arguments for 'incrby' command".to_string());
    }

    let key = match std::str::from_utf8(&args[0]) {
        Ok(s) => s.to_string(),
        Err(_) => return RespValue::Error("ERR invalid key".to_string()),
    };

    let increment = match std::str::from_utf8(&args[1]) {
        Ok(s) => match s.parse::<i64>() {
            Ok(n) => n,
            Err(_) => return RespValue::Error("ERR value is not an integer or out of range".to_string()),
        },
        Err(_) => return RespValue::Error("ERR value is not an integer or out of range".to_string()),
    };

    let db_instance = match db.get_db(db_index) {
        Some(d) => d,
        None => return RespValue::Error("ERR invalid database".to_string()),
    };

    let current_value = match db_instance.get(&key) {
        Some(RedisValue::String(bytes)) => {
            match std::str::from_utf8(&bytes) {
                Ok(s) => match s.parse::<i64>() {
                    Ok(n) => n,
                    Err(_) => return RespValue::Error("ERR value is not an integer or out of range".to_string()),
                },
                Err(_) => return RespValue::Error("ERR value is not an integer or out of range".to_string()),
            }
        }
        Some(_) => return RespValue::Error("WRONGTYPE Operation against a key holding the wrong kind of value".to_string()),
        None => 0,
    };

    let new_value = current_value + increment;
    db_instance.set(key, RedisValue::String(Bytes::from(new_value.to_string())));
    RespValue::Integer(new_value)
}

/// DECRBY key decrement
pub async fn decrby(db: &Arc<Database>, db_index: usize, args: Vec<Vec<u8>>) -> RespValue {
    if args.len() != 2 {
        return RespValue::Error("ERR wrong number of arguments for 'decrby' command".to_string());
    }

    // Negate the increment for DECRBY
    let decrement = match std::str::from_utf8(&args[1]) {
        Ok(s) => match s.parse::<i64>() {
            Ok(n) => -n,
            Err(_) => return RespValue::Error("ERR value is not an integer or out of range".to_string()),
        },
        Err(_) => return RespValue::Error("ERR value is not an integer or out of range".to_string()),
    };

    let decrement_bytes = decrement.to_string().into_bytes();
    incrby(db, db_index, vec![args[0].clone(), decrement_bytes]).await
}

/// INCRBYFLOAT key increment
pub async fn incrbyfloat(db: &Arc<Database>, db_index: usize, args: Vec<Vec<u8>>) -> RespValue {
    if args.len() != 2 {
        return RespValue::Error("ERR wrong number of arguments for 'incrbyfloat' command".to_string());
    }

    let key = match std::str::from_utf8(&args[0]) {
        Ok(s) => s.to_string(),
        Err(_) => return RespValue::Error("ERR invalid key".to_string()),
    };

    let increment = match std::str::from_utf8(&args[1]) {
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

    let current_value = match db_instance.get(&key) {
        Some(RedisValue::String(bytes)) => {
            match std::str::from_utf8(&bytes) {
                Ok(s) => match s.parse::<f64>() {
                    Ok(n) => n,
                    Err(_) => return RespValue::Error("ERR value is not a valid float".to_string()),
                },
                Err(_) => return RespValue::Error("ERR value is not a valid float".to_string()),
            }
        }
        Some(_) => return RespValue::Error("WRONGTYPE Operation against a key holding the wrong kind of value".to_string()),
        None => 0.0,
    };

    let new_value = current_value + increment;

    // Format the float, removing unnecessary trailing zeros
    let formatted = if new_value.fract() == 0.0 && new_value.abs() < 1e10 {
        format!("{:.1}", new_value)
    } else {
        format!("{}", new_value)
    };

    db_instance.set(key, RedisValue::String(Bytes::from(formatted.clone())));
    RespValue::BulkString(Some(formatted.into_bytes()))
}

/// PSETEX key milliseconds value
pub async fn psetex(db: &Arc<Database>, db_index: usize, args: Vec<Vec<u8>>) -> RespValue {
    if args.len() != 3 {
        return RespValue::Error("ERR wrong number of arguments for 'psetex' command".to_string());
    }

    let key = match std::str::from_utf8(&args[0]) {
        Ok(s) => s.to_string(),
        Err(_) => return RespValue::Error("ERR invalid key".to_string()),
    };

    let milliseconds = match std::str::from_utf8(&args[1]) {
        Ok(s) => match s.parse::<u64>() {
            Ok(n) => n,
            Err(_) => return RespValue::Error("ERR value is not an integer or out of range".to_string()),
        },
        Err(_) => return RespValue::Error("ERR value is not an integer or out of range".to_string()),
    };

    let value = Bytes::from(args[2].clone());

    let db_instance = match db.get_db(db_index) {
        Some(d) => d,
        None => return RespValue::Error("ERR invalid database".to_string()),
    };

    db_instance.set(key.clone(), RedisValue::String(value));

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64;
    db_instance.set_expiry(&key, now + milliseconds);

    RespValue::SimpleString("OK".to_string())
}

/// GETRANGE key start end
pub async fn getrange(db: &Arc<Database>, db_index: usize, args: Vec<Vec<u8>>) -> RespValue {
    if args.len() != 3 {
        return RespValue::Error("ERR wrong number of arguments for 'getrange' command".to_string());
    }

    let key = match std::str::from_utf8(&args[0]) {
        Ok(s) => s,
        Err(_) => return RespValue::Error("ERR invalid key".to_string()),
    };

    let start = match std::str::from_utf8(&args[1]) {
        Ok(s) => match s.parse::<i64>() {
            Ok(n) => n,
            Err(_) => return RespValue::Error("ERR value is not an integer or out of range".to_string()),
        },
        Err(_) => return RespValue::Error("ERR value is not an integer or out of range".to_string()),
    };

    let end = match std::str::from_utf8(&args[2]) {
        Ok(s) => match s.parse::<i64>() {
            Ok(n) => n,
            Err(_) => return RespValue::Error("ERR value is not an integer or out of range".to_string()),
        },
        Err(_) => return RespValue::Error("ERR value is not an integer or out of range".to_string()),
    };

    let db_instance = match db.get_db(db_index) {
        Some(d) => d,
        None => return RespValue::Error("ERR invalid database".to_string()),
    };

    match db_instance.get(key) {
        Some(RedisValue::String(bytes)) => {
            let len = bytes.len() as i64;
            let start_idx = normalize_index(start, len);
            let end_idx = normalize_index(end, len);

            if start_idx > end_idx || start_idx >= len {
                return RespValue::BulkString(Some(vec![]));
            }

            let result = bytes[start_idx as usize..=(end_idx as usize).min(len as usize - 1)].to_vec();
            RespValue::BulkString(Some(result))
        }
        Some(_) => RespValue::Error("WRONGTYPE Operation against a key holding the wrong kind of value".to_string()),
        None => RespValue::BulkString(Some(vec![])),
    }
}

/// SETRANGE key offset value
pub async fn setrange(db: &Arc<Database>, db_index: usize, args: Vec<Vec<u8>>) -> RespValue {
    if args.len() != 3 {
        return RespValue::Error("ERR wrong number of arguments for 'setrange' command".to_string());
    }

    let key = match std::str::from_utf8(&args[0]) {
        Ok(s) => s.to_string(),
        Err(_) => return RespValue::Error("ERR invalid key".to_string()),
    };

    let offset = match std::str::from_utf8(&args[1]) {
        Ok(s) => match s.parse::<usize>() {
            Ok(n) => n,
            Err(_) => return RespValue::Error("ERR value is not an integer or out of range".to_string()),
        },
        Err(_) => return RespValue::Error("ERR value is not an integer or out of range".to_string()),
    };

    let value = &args[2];

    let db_instance = match db.get_db(db_index) {
        Some(d) => d,
        None => return RespValue::Error("ERR invalid database".to_string()),
    };

    let mut current = match db_instance.get(&key) {
        Some(RedisValue::String(bytes)) => bytes.to_vec(),
        Some(_) => return RespValue::Error("WRONGTYPE Operation against a key holding the wrong kind of value".to_string()),
        None => vec![],
    };

    // Extend with zeros if necessary
    if offset > current.len() {
        current.resize(offset, 0);
    }

    // Replace bytes starting at offset
    for (i, &byte) in value.iter().enumerate() {
        let idx = offset + i;
        if idx >= current.len() {
            current.push(byte);
        } else {
            current[idx] = byte;
        }
    }

    let len = current.len();
    db_instance.set(key, RedisValue::String(Bytes::from(current)));
    RespValue::Integer(len as i64)
}

/// MGET key [key ...]
pub async fn mget(db: &Arc<Database>, db_index: usize, args: Vec<Vec<u8>>) -> RespValue {
    if args.is_empty() {
        return RespValue::Error("ERR wrong number of arguments for 'mget' command".to_string());
    }

    let db_instance = match db.get_db(db_index) {
        Some(d) => d,
        None => return RespValue::Error("ERR invalid database".to_string()),
    };

    let mut results = Vec::new();
    for key_bytes in args {
        let key = match std::str::from_utf8(&key_bytes) {
            Ok(s) => s,
            Err(_) => {
                results.push(RespValue::BulkString(None));
                continue;
            }
        };

        match db_instance.get(key) {
            Some(RedisValue::String(bytes)) => results.push(RespValue::BulkString(Some(bytes.to_vec()))),
            _ => results.push(RespValue::BulkString(None)),
        }
    }

    RespValue::Array(Some(results))
}

/// MSET key value [key value ...]
pub async fn mset(db: &Arc<Database>, db_index: usize, args: Vec<Vec<u8>>) -> RespValue {
    if args.is_empty() || args.len() % 2 != 0 {
        return RespValue::Error("ERR wrong number of arguments for 'mset' command".to_string());
    }

    let db_instance = match db.get_db(db_index) {
        Some(d) => d,
        None => return RespValue::Error("ERR invalid database".to_string()),
    };

    for chunk in args.chunks(2) {
        let key = match std::str::from_utf8(&chunk[0]) {
            Ok(s) => s.to_string(),
            Err(_) => continue,
        };

        let value = Bytes::from(chunk[1].clone());
        db_instance.set(key, RedisValue::String(value));
    }

    RespValue::SimpleString("OK".to_string())
}

/// GETEX key [EX seconds | PX milliseconds | EXAT unix-time-seconds | PXAT unix-time-milliseconds | PERSIST]
pub async fn getex(db: &Arc<Database>, db_index: usize, args: Vec<Vec<u8>>) -> RespValue {
    if args.is_empty() {
        return RespValue::Error("ERR wrong number of arguments for 'getex' command".to_string());
    }

    let key = match std::str::from_utf8(&args[0]) {
        Ok(s) => s.to_string(),
        Err(_) => return RespValue::Error("ERR invalid key".to_string()),
    };

    let db_instance = match db.get_db(db_index) {
        Some(d) => d,
        None => return RespValue::Error("ERR invalid database".to_string()),
    };

    // Get the current value
    let value = match db_instance.get(&key) {
        Some(RedisValue::String(bytes)) => bytes.to_vec(),
        Some(_) => return RespValue::Error("WRONGTYPE Operation against a key holding the wrong kind of value".to_string()),
        None => return RespValue::BulkString(None),
    };

    // Parse expiration options
    if args.len() > 1 {
        let option = match std::str::from_utf8(&args[1]) {
            Ok(s) => s.to_uppercase(),
            Err(_) => return RespValue::Error("ERR invalid option".to_string()),
        };

        match option.as_str() {
            "EX" => {
                if args.len() != 3 {
                    return RespValue::Error("ERR syntax error".to_string());
                }
                let seconds = match std::str::from_utf8(&args[2]) {
                    Ok(s) => match s.parse::<u64>() {
                        Ok(n) => n,
                        Err(_) => return RespValue::Error("ERR value is not an integer or out of range".to_string()),
                    },
                    Err(_) => return RespValue::Error("ERR value is not an integer or out of range".to_string()),
                };
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_millis() as u64;
                db_instance.set_expiry(&key, now + seconds * 1000);
            }
            "PX" => {
                if args.len() != 3 {
                    return RespValue::Error("ERR syntax error".to_string());
                }
                let millis = match std::str::from_utf8(&args[2]) {
                    Ok(s) => match s.parse::<u64>() {
                        Ok(n) => n,
                        Err(_) => return RespValue::Error("ERR value is not an integer or out of range".to_string()),
                    },
                    Err(_) => return RespValue::Error("ERR value is not an integer or out of range".to_string()),
                };
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_millis() as u64;
                db_instance.set_expiry(&key, now + millis);
            }
            "EXAT" => {
                if args.len() != 3 {
                    return RespValue::Error("ERR syntax error".to_string());
                }
                let timestamp = match std::str::from_utf8(&args[2]) {
                    Ok(s) => match s.parse::<u64>() {
                        Ok(n) => n,
                        Err(_) => return RespValue::Error("ERR value is not an integer or out of range".to_string()),
                    },
                    Err(_) => return RespValue::Error("ERR value is not an integer or out of range".to_string()),
                };
                db_instance.set_expiry(&key, timestamp * 1000);
            }
            "PXAT" => {
                if args.len() != 3 {
                    return RespValue::Error("ERR syntax error".to_string());
                }
                let timestamp = match std::str::from_utf8(&args[2]) {
                    Ok(s) => match s.parse::<u64>() {
                        Ok(n) => n,
                        Err(_) => return RespValue::Error("ERR value is not an integer or out of range".to_string()),
                    },
                    Err(_) => return RespValue::Error("ERR value is not an integer or out of range".to_string()),
                };
                db_instance.set_expiry(&key, timestamp);
            }
            "PERSIST" => {
                if args.len() != 2 {
                    return RespValue::Error("ERR syntax error".to_string());
                }
                db_instance.persist(&key);
            }
            _ => return RespValue::Error("ERR syntax error".to_string()),
        }
    }

    RespValue::BulkString(Some(value))
}

/// GETDEL key
pub async fn getdel(db: &Arc<Database>, db_index: usize, args: Vec<Vec<u8>>) -> RespValue {
    if args.len() != 1 {
        return RespValue::Error("ERR wrong number of arguments for 'getdel' command".to_string());
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
        Some(RedisValue::String(bytes)) => {
            let value = bytes.to_vec();
            db_instance.delete(key);
            RespValue::BulkString(Some(value))
        }
        Some(_) => RespValue::Error("WRONGTYPE Operation against a key holding the wrong kind of value".to_string()),
        None => RespValue::BulkString(None),
    }
}

/// SETEX key seconds value
pub async fn setex(db: &Arc<Database>, db_index: usize, args: Vec<Vec<u8>>) -> RespValue {
    if args.len() != 3 {
        return RespValue::Error("ERR wrong number of arguments for 'setex' command".to_string());
    }

    let key = match std::str::from_utf8(&args[0]) {
        Ok(s) => s.to_string(),
        Err(_) => return RespValue::Error("ERR invalid key".to_string()),
    };

    let seconds = match std::str::from_utf8(&args[1]) {
        Ok(s) => match s.parse::<u64>() {
            Ok(n) => n,
            Err(_) => return RespValue::Error("ERR value is not an integer or out of range".to_string()),
        },
        Err(_) => return RespValue::Error("ERR value is not an integer or out of range".to_string()),
    };

    let value = Bytes::from(args[2].clone());

    let db_instance = match db.get_db(db_index) {
        Some(d) => d,
        None => return RespValue::Error("ERR invalid database".to_string()),
    };

    db_instance.set(key.clone(), RedisValue::String(value));

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64;
    db_instance.set_expiry(&key, now + seconds * 1000);

    RespValue::SimpleString("OK".to_string())
}

/// SETNX key value
pub async fn setnx(db: &Arc<Database>, db_index: usize, args: Vec<Vec<u8>>) -> RespValue {
    if args.len() != 2 {
        return RespValue::Error("ERR wrong number of arguments for 'setnx' command".to_string());
    }

    let key = match std::str::from_utf8(&args[0]) {
        Ok(s) => s.to_string(),
        Err(_) => return RespValue::Error("ERR invalid key".to_string()),
    };

    let value = Bytes::from(args[1].clone());

    let db_instance = match db.get_db(db_index) {
        Some(d) => d,
        None => return RespValue::Error("ERR invalid database".to_string()),
    };

    if db_instance.exists(&key) {
        RespValue::Integer(0)
    } else {
        db_instance.set(key, RedisValue::String(value));
        RespValue::Integer(1)
    }
}

/// MSETNX key value [key value ...]
pub async fn msetnx(db: &Arc<Database>, db_index: usize, args: Vec<Vec<u8>>) -> RespValue {
    if args.is_empty() || args.len() % 2 != 0 {
        return RespValue::Error("ERR wrong number of arguments for 'msetnx' command".to_string());
    }

    let db_instance = match db.get_db(db_index) {
        Some(d) => d,
        None => return RespValue::Error("ERR invalid database".to_string()),
    };

    // Check if any key exists
    for chunk in args.chunks(2) {
        let key = match std::str::from_utf8(&chunk[0]) {
            Ok(s) => s,
            Err(_) => continue,
        };

        if db_instance.exists(key) {
            return RespValue::Integer(0);
        }
    }

    // All keys don't exist, set them all
    for chunk in args.chunks(2) {
        let key = match std::str::from_utf8(&chunk[0]) {
            Ok(s) => s.to_string(),
            Err(_) => continue,
        };

        let value = Bytes::from(chunk[1].clone());
        db_instance.set(key, RedisValue::String(value));
    }

    RespValue::Integer(1)
}

// Helper function to normalize negative indices
fn normalize_index(index: i64, len: i64) -> i64 {
    if index < 0 {
        (len + index).max(0)
    } else {
        index.min(len - 1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_set_get() {
        let db = Arc::new(Database::new(16));
        let result = set(&db, 0, vec![b"key".to_vec(), b"value".to_vec()]).await;
        assert_eq!(result, RespValue::SimpleString("OK".to_string()));

        let result = get(&db, 0, vec![b"key".to_vec()]).await;
        assert_eq!(result, RespValue::BulkString(Some(b"value".to_vec())));
    }

    #[tokio::test]
    async fn test_incr_decr() {
        let db = Arc::new(Database::new(16));
        set(&db, 0, vec![b"counter".to_vec(), b"10".to_vec()]).await;

        let result = incr(&db, 0, vec![b"counter".to_vec()]).await;
        assert_eq!(result, RespValue::Integer(11));

        let result = decr(&db, 0, vec![b"counter".to_vec()]).await;
        assert_eq!(result, RespValue::Integer(10));
    }

    #[test]
    fn test_normalize_index() {
        assert_eq!(normalize_index(0, 10), 0);
        assert_eq!(normalize_index(5, 10), 5);
        assert_eq!(normalize_index(-1, 10), 9);
        assert_eq!(normalize_index(-5, 10), 5);
        assert_eq!(normalize_index(100, 10), 9);
    }

    #[tokio::test]
    async fn test_set_with_ex() {
        let db = Arc::new(Database::new(16));
        let result = set(&db, 0, vec![b"key".to_vec(), b"value".to_vec(), b"EX".to_vec(), b"10".to_vec()]).await;
        assert_eq!(result, RespValue::SimpleString("OK".to_string()));

        let db_instance = db.get_db(0).unwrap();
        let ttl = db_instance.get_ttl_ms("key");
        assert!(ttl > 9000 && ttl <= 10000); // Should be around 10 seconds
    }

    #[tokio::test]
    async fn test_set_with_px() {
        let db = Arc::new(Database::new(16));
        let result = set(&db, 0, vec![b"key".to_vec(), b"value".to_vec(), b"PX".to_vec(), b"5000".to_vec()]).await;
        assert_eq!(result, RespValue::SimpleString("OK".to_string()));

        let db_instance = db.get_db(0).unwrap();
        let ttl = db_instance.get_ttl_ms("key");
        assert!(ttl > 4900 && ttl <= 5000); // Should be around 5000 milliseconds
    }

    #[tokio::test]
    async fn test_set_nx() {
        let db = Arc::new(Database::new(16));

        // First SET with NX should succeed
        let result = set(&db, 0, vec![b"key".to_vec(), b"value1".to_vec(), b"NX".to_vec()]).await;
        assert_eq!(result, RespValue::SimpleString("OK".to_string()));

        // Second SET with NX should fail (return null)
        let result = set(&db, 0, vec![b"key".to_vec(), b"value2".to_vec(), b"NX".to_vec()]).await;
        assert_eq!(result, RespValue::BulkString(None));

        // Value should still be value1
        let result = get(&db, 0, vec![b"key".to_vec()]).await;
        assert_eq!(result, RespValue::BulkString(Some(b"value1".to_vec())));
    }

    #[tokio::test]
    async fn test_set_xx() {
        let db = Arc::new(Database::new(16));

        // First SET with XX should fail (key doesn't exist)
        let result = set(&db, 0, vec![b"key".to_vec(), b"value1".to_vec(), b"XX".to_vec()]).await;
        assert_eq!(result, RespValue::BulkString(None));

        // Create the key first
        set(&db, 0, vec![b"key".to_vec(), b"value1".to_vec()]).await;

        // Now SET with XX should succeed
        let result = set(&db, 0, vec![b"key".to_vec(), b"value2".to_vec(), b"XX".to_vec()]).await;
        assert_eq!(result, RespValue::SimpleString("OK".to_string()));

        // Value should be value2
        let result = get(&db, 0, vec![b"key".to_vec()]).await;
        assert_eq!(result, RespValue::BulkString(Some(b"value2".to_vec())));
    }

    #[tokio::test]
    async fn test_set_get_option() {
        let db = Arc::new(Database::new(16));

        // First SET without GET
        set(&db, 0, vec![b"key".to_vec(), b"value1".to_vec()]).await;

        // SET with GET should return old value
        let result = set(&db, 0, vec![b"key".to_vec(), b"value2".to_vec(), b"GET".to_vec()]).await;
        assert_eq!(result, RespValue::BulkString(Some(b"value1".to_vec())));

        // Value should be updated to value2
        let result = get(&db, 0, vec![b"key".to_vec()]).await;
        assert_eq!(result, RespValue::BulkString(Some(b"value2".to_vec())));
    }

    #[tokio::test]
    async fn test_set_keepttl() {
        let db = Arc::new(Database::new(16));

        // Set key with expiration
        set(&db, 0, vec![b"key".to_vec(), b"value1".to_vec(), b"EX".to_vec(), b"100".to_vec()]).await;

        let db_instance = db.get_db(0).unwrap();
        let ttl_before = db_instance.get_ttl_ms("key");
        assert!(ttl_before > 0);

        // Update value with KEEPTTL
        set(&db, 0, vec![b"key".to_vec(), b"value2".to_vec(), b"KEEPTTL".to_vec()]).await;

        let ttl_after = db_instance.get_ttl_ms("key");
        assert!(ttl_after > 0);
        assert!(ttl_after <= ttl_before); // TTL should be preserved (or slightly less due to time)

        // Value should be updated
        let result = get(&db, 0, vec![b"key".to_vec()]).await;
        assert_eq!(result, RespValue::BulkString(Some(b"value2".to_vec())));
    }

    #[tokio::test]
    async fn test_set_nx_xx_conflict() {
        let db = Arc::new(Database::new(16));

        // NX and XX together should return error
        let result = set(&db, 0, vec![b"key".to_vec(), b"value".to_vec(), b"NX".to_vec(), b"XX".to_vec()]).await;
        match result {
            RespValue::Error(msg) => assert!(msg.contains("not compatible")),
            _ => panic!("Expected error for NX and XX together"),
        }
    }

    #[tokio::test]
    async fn test_set_combined_options() {
        let db = Arc::new(Database::new(16));

        // Set initial value
        set(&db, 0, vec![b"key".to_vec(), b"oldvalue".to_vec()]).await;

        // SET with EX and GET combined
        let result = set(&db, 0, vec![
            b"key".to_vec(),
            b"newvalue".to_vec(),
            b"EX".to_vec(),
            b"50".to_vec(),
            b"GET".to_vec()
        ]).await;

        // Should return old value
        assert_eq!(result, RespValue::BulkString(Some(b"oldvalue".to_vec())));

        // New value should be set
        let result = get(&db, 0, vec![b"key".to_vec()]).await;
        assert_eq!(result, RespValue::BulkString(Some(b"newvalue".to_vec())));

        // TTL should be set
        let db_instance = db.get_db(0).unwrap();
        let ttl = db_instance.get_ttl_ms("key");
        assert!(ttl > 0 && ttl <= 50000);
    }
}
