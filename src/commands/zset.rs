// Sorted Set (ZSet) command handlers

use crate::protocol::RespValue;
use crate::storage::db::Database;
use crate::storage::types::{RedisValue, ZSet};
use bytes::Bytes;
use ordered_float::OrderedFloat;
use std::sync::Arc;

/// ZADD key score member [score member ...]
pub async fn zadd(db: &Arc<Database>, db_index: usize, args: Vec<Vec<u8>>) -> RespValue {
    if args.len() < 3 || args.len() % 2 == 0 {
        return RespValue::Error("ERR wrong number of arguments for 'zadd' command".to_string());
    }

    let key = match std::str::from_utf8(&args[0]) {
        Ok(s) => s.to_string(),
        Err(_) => return RespValue::Error("ERR invalid key".to_string()),
    };

    let db_instance = match db.get_db(db_index) {
        Some(d) => d,
        None => return RespValue::Error("ERR invalid database".to_string()),
    };

    let mut zset = match db_instance.get(&key) {
        Some(RedisValue::ZSet(z)) => z,
        Some(_) => {
            return RespValue::Error(
                "WRONGTYPE Operation against a key holding the wrong kind of value".to_string(),
            )
        }
        None => ZSet::new(),
    };

    let mut added = 0;
    for chunk in args[1..].chunks(2) {
        let score = match std::str::from_utf8(&chunk[0]) {
            Ok(s) => match s.parse::<f64>() {
                Ok(n) => n,
                Err(_) => {
                    return RespValue::Error("ERR value is not a valid float".to_string())
                }
            },
            Err(_) => return RespValue::Error("ERR value is not a valid float".to_string()),
        };

        let member = Bytes::from(chunk[1].clone());

        // Remove old score entry if member exists
        if let Some(old_score) = zset.members.get(&member) {
            zset.scores.remove(&(OrderedFloat(*old_score), member.clone()));
        } else {
            added += 1;
        }

        // Add new score entry
        zset.scores.insert((OrderedFloat(score), member.clone()), ());
        zset.members.insert(member, score);
    }

    db_instance.set(key, RedisValue::ZSet(zset));
    RespValue::Integer(added)
}

/// ZREM key member [member ...]
pub async fn zrem(db: &Arc<Database>, db_index: usize, args: Vec<Vec<u8>>) -> RespValue {
    if args.len() < 2 {
        return RespValue::Error("ERR wrong number of arguments for 'zrem' command".to_string());
    }

    let key = match std::str::from_utf8(&args[0]) {
        Ok(s) => s.to_string(),
        Err(_) => return RespValue::Error("ERR invalid key".to_string()),
    };

    let db_instance = match db.get_db(db_index) {
        Some(d) => d,
        None => return RespValue::Error("ERR invalid database".to_string()),
    };

    let mut zset = match db_instance.get(&key) {
        Some(RedisValue::ZSet(z)) => z,
        Some(_) => {
            return RespValue::Error(
                "WRONGTYPE Operation against a key holding the wrong kind of value".to_string(),
            )
        }
        None => return RespValue::Integer(0),
    };

    let mut removed = 0;
    for member_bytes in &args[1..] {
        let member = Bytes::from(member_bytes.clone());
        if let Some(score) = zset.members.remove(&member) {
            zset.scores.remove(&(OrderedFloat(score), member));
            removed += 1;
        }
    }

    if zset.is_empty() {
        db_instance.delete(&key);
    } else {
        db_instance.set(key, RedisValue::ZSet(zset));
    }

    RespValue::Integer(removed)
}

/// ZSCORE key member
pub async fn zscore(db: &Arc<Database>, db_index: usize, args: Vec<Vec<u8>>) -> RespValue {
    if args.len() != 2 {
        return RespValue::Error("ERR wrong number of arguments for 'zscore' command".to_string());
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
        Some(RedisValue::ZSet(zset)) => match zset.members.get(&member) {
            Some(score) => RespValue::BulkString(Some(score.to_string().into_bytes())),
            None => RespValue::BulkString(None),
        },
        Some(_) => RespValue::Error(
            "WRONGTYPE Operation against a key holding the wrong kind of value".to_string(),
        ),
        None => RespValue::BulkString(None),
    }
}

/// ZCARD key
pub async fn zcard(db: &Arc<Database>, db_index: usize, args: Vec<Vec<u8>>) -> RespValue {
    if args.len() != 1 {
        return RespValue::Error("ERR wrong number of arguments for 'zcard' command".to_string());
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
        Some(RedisValue::ZSet(zset)) => RespValue::Integer(zset.len() as i64),
        Some(_) => RespValue::Error(
            "WRONGTYPE Operation against a key holding the wrong kind of value".to_string(),
        ),
        None => RespValue::Integer(0),
    }
}

/// ZCOUNT key min max
pub async fn zcount(db: &Arc<Database>, db_index: usize, args: Vec<Vec<u8>>) -> RespValue {
    if args.len() != 3 {
        return RespValue::Error("ERR wrong number of arguments for 'zcount' command".to_string());
    }

    let key = match std::str::from_utf8(&args[0]) {
        Ok(s) => s,
        Err(_) => return RespValue::Error("ERR invalid key".to_string()),
    };

    let min = match parse_score_range(&args[1]) {
        Ok(s) => s,
        Err(e) => return RespValue::Error(e),
    };

    let max = match parse_score_range(&args[2]) {
        Ok(s) => s,
        Err(e) => return RespValue::Error(e),
    };

    let db_instance = match db.get_db(db_index) {
        Some(d) => d,
        None => return RespValue::Error("ERR invalid database".to_string()),
    };

    match db_instance.get(key) {
        Some(RedisValue::ZSet(zset)) => {
            let count = zset
                .members
                .values()
                .filter(|&&score| score >= min && score <= max)
                .count();
            RespValue::Integer(count as i64)
        }
        Some(_) => RespValue::Error(
            "WRONGTYPE Operation against a key holding the wrong kind of value".to_string(),
        ),
        None => RespValue::Integer(0),
    }
}

/// ZRANGE key start stop [WITHSCORES]
pub async fn zrange(db: &Arc<Database>, db_index: usize, args: Vec<Vec<u8>>) -> RespValue {
    if args.len() < 3 || args.len() > 4 {
        return RespValue::Error("ERR wrong number of arguments for 'zrange' command".to_string());
    }

    let key = match std::str::from_utf8(&args[0]) {
        Ok(s) => s,
        Err(_) => return RespValue::Error("ERR invalid key".to_string()),
    };

    let start = match std::str::from_utf8(&args[1]) {
        Ok(s) => match s.parse::<i64>() {
            Ok(n) => n,
            Err(_) => return RespValue::Error("ERR value is not an integer".to_string()),
        },
        Err(_) => return RespValue::Error("ERR value is not an integer".to_string()),
    };

    let stop = match std::str::from_utf8(&args[2]) {
        Ok(s) => match s.parse::<i64>() {
            Ok(n) => n,
            Err(_) => return RespValue::Error("ERR value is not an integer".to_string()),
        },
        Err(_) => return RespValue::Error("ERR value is not an integer".to_string()),
    };

    let with_scores = if args.len() == 4 {
        match std::str::from_utf8(&args[3]) {
            Ok(s) if s.to_uppercase() == "WITHSCORES" => true,
            _ => return RespValue::Error("ERR syntax error".to_string()),
        }
    } else {
        false
    };

    let db_instance = match db.get_db(db_index) {
        Some(d) => d,
        None => return RespValue::Error("ERR invalid database".to_string()),
    };

    match db_instance.get(key) {
        Some(RedisValue::ZSet(zset)) => {
            let len = zset.len() as i64;
            if len == 0 {
                return RespValue::Array(Some(vec![]));
            }

            // Normalize negative indices
            let start = if start < 0 {
                (len + start).max(0)
            } else {
                start.min(len - 1)
            };
            let stop = if stop < 0 {
                (len + stop).max(0)
            } else {
                stop.min(len - 1)
            };

            if start > stop {
                return RespValue::Array(Some(vec![]));
            }

            let mut result = Vec::new();
            for (i, ((_, member), _)) in zset.scores.iter().enumerate() {
                let idx = i as i64;
                if idx >= start && idx <= stop {
                    result.push(RespValue::BulkString(Some(member.to_vec())));
                    if with_scores {
                        if let Some(&score) = zset.members.get(member) {
                            result.push(RespValue::BulkString(Some(score.to_string().into_bytes())));
                        }
                    }
                }
                if idx > stop {
                    break;
                }
            }

            RespValue::Array(Some(result))
        }
        Some(_) => RespValue::Error(
            "WRONGTYPE Operation against a key holding the wrong kind of value".to_string(),
        ),
        None => RespValue::Array(Some(vec![])),
    }
}

/// ZREVRANGE key start stop [WITHSCORES]
pub async fn zrevrange(db: &Arc<Database>, db_index: usize, args: Vec<Vec<u8>>) -> RespValue {
    if args.len() < 3 || args.len() > 4 {
        return RespValue::Error(
            "ERR wrong number of arguments for 'zrevrange' command".to_string(),
        );
    }

    let key = match std::str::from_utf8(&args[0]) {
        Ok(s) => s,
        Err(_) => return RespValue::Error("ERR invalid key".to_string()),
    };

    let start = match std::str::from_utf8(&args[1]) {
        Ok(s) => match s.parse::<i64>() {
            Ok(n) => n,
            Err(_) => return RespValue::Error("ERR value is not an integer".to_string()),
        },
        Err(_) => return RespValue::Error("ERR value is not an integer".to_string()),
    };

    let stop = match std::str::from_utf8(&args[2]) {
        Ok(s) => match s.parse::<i64>() {
            Ok(n) => n,
            Err(_) => return RespValue::Error("ERR value is not an integer".to_string()),
        },
        Err(_) => return RespValue::Error("ERR value is not an integer".to_string()),
    };

    let with_scores = if args.len() == 4 {
        match std::str::from_utf8(&args[3]) {
            Ok(s) if s.to_uppercase() == "WITHSCORES" => true,
            _ => return RespValue::Error("ERR syntax error".to_string()),
        }
    } else {
        false
    };

    let db_instance = match db.get_db(db_index) {
        Some(d) => d,
        None => return RespValue::Error("ERR invalid database".to_string()),
    };

    match db_instance.get(key) {
        Some(RedisValue::ZSet(zset)) => {
            let len = zset.len() as i64;
            if len == 0 {
                return RespValue::Array(Some(vec![]));
            }

            // Normalize negative indices
            let start = if start < 0 {
                (len + start).max(0)
            } else {
                start.min(len - 1)
            };
            let stop = if stop < 0 {
                (len + stop).max(0)
            } else {
                stop.min(len - 1)
            };

            if start > stop {
                return RespValue::Array(Some(vec![]));
            }

            let mut result = Vec::new();
            for (i, ((_, member), _)) in zset.scores.iter().rev().enumerate() {
                let idx = i as i64;
                if idx >= start && idx <= stop {
                    result.push(RespValue::BulkString(Some(member.to_vec())));
                    if with_scores {
                        if let Some(&score) = zset.members.get(member) {
                            result.push(RespValue::BulkString(Some(score.to_string().into_bytes())));
                        }
                    }
                }
                if idx > stop {
                    break;
                }
            }

            RespValue::Array(Some(result))
        }
        Some(_) => RespValue::Error(
            "WRONGTYPE Operation against a key holding the wrong kind of value".to_string(),
        ),
        None => RespValue::Array(Some(vec![])),
    }
}

/// ZRANGEBYSCORE key min max [WITHSCORES] [LIMIT offset count]
pub async fn zrangebyscore(db: &Arc<Database>, db_index: usize, args: Vec<Vec<u8>>) -> RespValue {
    if args.len() < 3 {
        return RespValue::Error(
            "ERR wrong number of arguments for 'zrangebyscore' command".to_string(),
        );
    }

    let key = match std::str::from_utf8(&args[0]) {
        Ok(s) => s,
        Err(_) => return RespValue::Error("ERR invalid key".to_string()),
    };

    let min = match parse_score_range(&args[1]) {
        Ok(s) => s,
        Err(e) => return RespValue::Error(e),
    };

    let max = match parse_score_range(&args[2]) {
        Ok(s) => s,
        Err(e) => return RespValue::Error(e),
    };

    let mut with_scores = false;
    let mut _limit_offset = 0i64;
    let mut _limit_count = -1i64;

    // Parse optional arguments
    let mut i = 3;
    while i < args.len() {
        match std::str::from_utf8(&args[i]) {
            Ok(s) if s.to_uppercase() == "WITHSCORES" => {
                with_scores = true;
                i += 1;
            }
            Ok(s) if s.to_uppercase() == "LIMIT" => {
                if i + 2 >= args.len() {
                    return RespValue::Error("ERR syntax error".to_string());
                }
                // LIMIT not fully implemented yet
                i += 3;
            }
            _ => return RespValue::Error("ERR syntax error".to_string()),
        }
    }

    let db_instance = match db.get_db(db_index) {
        Some(d) => d,
        None => return RespValue::Error("ERR invalid database".to_string()),
    };

    match db_instance.get(key) {
        Some(RedisValue::ZSet(zset)) => {
            let mut result = Vec::new();
            for ((score, member), _) in zset.scores.iter() {
                let s = score.into_inner();
                if s >= min && s <= max {
                    result.push(RespValue::BulkString(Some(member.to_vec())));
                    if with_scores {
                        result.push(RespValue::BulkString(Some(s.to_string().into_bytes())));
                    }
                }
            }
            RespValue::Array(Some(result))
        }
        Some(_) => RespValue::Error(
            "WRONGTYPE Operation against a key holding the wrong kind of value".to_string(),
        ),
        None => RespValue::Array(Some(vec![])),
    }
}

/// ZRANK key member
pub async fn zrank(db: &Arc<Database>, db_index: usize, args: Vec<Vec<u8>>) -> RespValue {
    if args.len() != 2 {
        return RespValue::Error("ERR wrong number of arguments for 'zrank' command".to_string());
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
        Some(RedisValue::ZSet(zset)) => {
            // Check if member exists
            if !zset.members.contains_key(&member) {
                return RespValue::BulkString(None);
            }

            // Find rank by iterating through sorted scores
            for (rank, ((_, m), _)) in zset.scores.iter().enumerate() {
                if m == &member {
                    return RespValue::Integer(rank as i64);
                }
            }
            RespValue::BulkString(None)
        }
        Some(_) => RespValue::Error(
            "WRONGTYPE Operation against a key holding the wrong kind of value".to_string(),
        ),
        None => RespValue::BulkString(None),
    }
}

/// ZREVRANK key member
pub async fn zrevrank(db: &Arc<Database>, db_index: usize, args: Vec<Vec<u8>>) -> RespValue {
    if args.len() != 2 {
        return RespValue::Error(
            "ERR wrong number of arguments for 'zrevrank' command".to_string(),
        );
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
        Some(RedisValue::ZSet(zset)) => {
            // Check if member exists
            if !zset.members.contains_key(&member) {
                return RespValue::BulkString(None);
            }

            // Find rank by iterating through sorted scores in reverse
            for (rank, ((_, m), _)) in zset.scores.iter().rev().enumerate() {
                if m == &member {
                    return RespValue::Integer(rank as i64);
                }
            }
            RespValue::BulkString(None)
        }
        Some(_) => RespValue::Error(
            "WRONGTYPE Operation against a key holding the wrong kind of value".to_string(),
        ),
        None => RespValue::BulkString(None),
    }
}

/// ZINCRBY key increment member
pub async fn zincrby(db: &Arc<Database>, db_index: usize, args: Vec<Vec<u8>>) -> RespValue {
    if args.len() != 3 {
        return RespValue::Error("ERR wrong number of arguments for 'zincrby' command".to_string());
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

    let member = Bytes::from(args[2].clone());

    let db_instance = match db.get_db(db_index) {
        Some(d) => d,
        None => return RespValue::Error("ERR invalid database".to_string()),
    };

    let mut zset = match db_instance.get(&key) {
        Some(RedisValue::ZSet(z)) => z,
        Some(_) => {
            return RespValue::Error(
                "WRONGTYPE Operation against a key holding the wrong kind of value".to_string(),
            )
        }
        None => ZSet::new(),
    };

    // Get current score or default to 0.0
    let current_score = zset.members.get(&member).copied().unwrap_or(0.0);
    let new_score = current_score + increment;

    // Remove old entry and add new one
    if let Some(&old_score) = zset.members.get(&member) {
        zset.scores.remove(&(OrderedFloat(old_score), member.clone()));
    }

    zset.members.insert(member.clone(), new_score);
    zset.scores.insert((OrderedFloat(new_score), member), ());

    db_instance.set(key, RedisValue::ZSet(zset));

    // Format the score for response
    let formatted = if new_score.fract() == 0.0 && new_score.abs() < 1e10 {
        format!("{:.1}", new_score)
    } else {
        format!("{}", new_score)
    };
    RespValue::BulkString(Some(formatted.into_bytes()))
}

/// ZPOPMIN key [count]
pub async fn zpopmin(db: &Arc<Database>, db_index: usize, args: Vec<Vec<u8>>) -> RespValue {
    if args.is_empty() || args.len() > 2 {
        return RespValue::Error("ERR wrong number of arguments for 'zpopmin' command".to_string());
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

    let mut zset = match db_instance.get(&key) {
        Some(RedisValue::ZSet(z)) => z,
        Some(_) => {
            return RespValue::Error(
                "WRONGTYPE Operation against a key holding the wrong kind of value".to_string(),
            )
        }
        None => return RespValue::Array(Some(vec![])),
    };

    let mut result = Vec::new();
    let mut popped = 0;

    // Pop minimum elements
    while popped < count && !zset.scores.is_empty() {
        if let Some(((score, member), _)) = zset.scores.iter().next().map(|((s, m), v)| ((*s, m.clone()), v)) {
            zset.scores.remove(&(score, member.clone()));
            zset.members.remove(&member);

            result.push(RespValue::BulkString(Some(member.to_vec())));
            result.push(RespValue::BulkString(Some(format!("{}", score.0).into_bytes())));
            popped += 1;
        }
    }

    if zset.is_empty() {
        db_instance.delete(&key);
    } else {
        db_instance.set(key, RedisValue::ZSet(zset));
    }

    RespValue::Array(Some(result))
}

/// ZPOPMAX key [count]
pub async fn zpopmax(db: &Arc<Database>, db_index: usize, args: Vec<Vec<u8>>) -> RespValue {
    if args.is_empty() || args.len() > 2 {
        return RespValue::Error("ERR wrong number of arguments for 'zpopmax' command".to_string());
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

    let mut zset = match db_instance.get(&key) {
        Some(RedisValue::ZSet(z)) => z,
        Some(_) => {
            return RespValue::Error(
                "WRONGTYPE Operation against a key holding the wrong kind of value".to_string(),
            )
        }
        None => return RespValue::Array(Some(vec![])),
    };

    let mut result = Vec::new();
    let mut popped = 0;

    // Pop maximum elements (iterate in reverse)
    while popped < count && !zset.scores.is_empty() {
        if let Some(((score, member), _)) = zset.scores.iter().next_back().map(|((s, m), v)| ((*s, m.clone()), v)) {
            zset.scores.remove(&(score, member.clone()));
            zset.members.remove(&member);

            result.push(RespValue::BulkString(Some(member.to_vec())));
            result.push(RespValue::BulkString(Some(format!("{}", score.0).into_bytes())));
            popped += 1;
        }
    }

    if zset.is_empty() {
        db_instance.delete(&key);
    } else {
        db_instance.set(key, RedisValue::ZSet(zset));
    }

    RespValue::Array(Some(result))
}

/// ZREMRANGEBYRANK key start stop
pub async fn zremrangebyrank(
    db: &Arc<Database>,
    db_index: usize,
    args: Vec<Vec<u8>>,
) -> RespValue {
    if args.len() != 3 {
        return RespValue::Error(
            "ERR wrong number of arguments for 'zremrangebyrank' command".to_string(),
        );
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
        Err(_) => {
            return RespValue::Error("ERR value is not an integer or out of range".to_string())
        }
    };

    let stop = match std::str::from_utf8(&args[2]) {
        Ok(s) => match s.parse::<i64>() {
            Ok(n) => n,
            Err(_) => {
                return RespValue::Error("ERR value is not an integer or out of range".to_string())
            }
        },
        Err(_) => {
            return RespValue::Error("ERR value is not an integer or out of range".to_string())
        }
    };

    let db_instance = match db.get_db(db_index) {
        Some(d) => d,
        None => return RespValue::Error("ERR invalid database".to_string()),
    };

    let mut zset = match db_instance.get(&key) {
        Some(RedisValue::ZSet(z)) => z,
        Some(_) => {
            return RespValue::Error(
                "WRONGTYPE Operation against a key holding the wrong kind of value".to_string(),
            )
        }
        None => return RespValue::Integer(0),
    };

    let len = zset.scores.len() as i64;
    if len == 0 {
        return RespValue::Integer(0);
    }

    // Handle negative indices
    let start_idx = if start < 0 {
        (len + start).max(0) as usize
    } else {
        start.min(len) as usize
    };

    let stop_idx = if stop < 0 {
        (len + stop).max(-1) as usize
    } else {
        stop.min(len - 1) as usize
    };

    if start_idx > stop_idx {
        return RespValue::Integer(0);
    }

    // Collect members to remove
    let members_to_remove: Vec<_> = zset
        .scores
        .iter()
        .skip(start_idx)
        .take(stop_idx - start_idx + 1)
        .map(|((score, member), _)| (*score, member.clone()))
        .collect();

    let removed = members_to_remove.len() as i64;

    // Remove them
    for (score, member) in members_to_remove {
        zset.scores.remove(&(score, member.clone()));
        zset.members.remove(&member);
    }

    if zset.is_empty() {
        db_instance.delete(&key);
    } else {
        db_instance.set(key, RedisValue::ZSet(zset));
    }

    RespValue::Integer(removed)
}

/// ZREMRANGEBYSCORE key min max
pub async fn zremrangebyscore(
    db: &Arc<Database>,
    db_index: usize,
    args: Vec<Vec<u8>>,
) -> RespValue {
    if args.len() != 3 {
        return RespValue::Error(
            "ERR wrong number of arguments for 'zremrangebyscore' command".to_string(),
        );
    }

    let key = match std::str::from_utf8(&args[0]) {
        Ok(s) => s.to_string(),
        Err(_) => return RespValue::Error("ERR invalid key".to_string()),
    };

    let min_score = match parse_score_range(&args[1]) {
        Ok(s) => s,
        Err(e) => return RespValue::Error(e),
    };

    let max_score = match parse_score_range(&args[2]) {
        Ok(s) => s,
        Err(e) => return RespValue::Error(e),
    };

    let db_instance = match db.get_db(db_index) {
        Some(d) => d,
        None => return RespValue::Error("ERR invalid database".to_string()),
    };

    let mut zset = match db_instance.get(&key) {
        Some(RedisValue::ZSet(z)) => z,
        Some(_) => {
            return RespValue::Error(
                "WRONGTYPE Operation against a key holding the wrong kind of value".to_string(),
            )
        }
        None => return RespValue::Integer(0),
    };

    // Collect members in score range
    let members_to_remove: Vec<_> = zset
        .scores
        .iter()
        .filter(|((score, _), _)| score.0 >= min_score && score.0 <= max_score)
        .map(|((score, member), _)| (*score, member.clone()))
        .collect();

    let removed = members_to_remove.len() as i64;

    // Remove them
    for (score, member) in members_to_remove {
        zset.scores.remove(&(score, member.clone()));
        zset.members.remove(&member);
    }

    if zset.is_empty() {
        db_instance.delete(&key);
    } else {
        db_instance.set(key, RedisValue::ZSet(zset));
    }

    RespValue::Integer(removed)
}

/// ZREVRANGEBYSCORE key max min [WITHSCORES] [LIMIT offset count]
/// Return members in reverse score order
pub async fn zrevrangebyscore(db: &Arc<Database>, db_index: usize, args: Vec<Vec<u8>>) -> RespValue {
    if args.len() < 3 {
        return RespValue::Error("ERR wrong number of arguments for 'zrevrangebyscore' command".to_string());
    }

    let key = match std::str::from_utf8(&args[0]) {
        Ok(s) => s,
        Err(_) => return RespValue::Error("ERR invalid key".to_string()),
    };

    let max_score = match parse_score_range(&args[1]) {
        Ok(s) => s,
        Err(e) => return RespValue::Error(e),
    };

    let min_score = match parse_score_range(&args[2]) {
        Ok(s) => s,
        Err(e) => return RespValue::Error(e),
    };

    let mut withscores = false;
    let mut limit_offset = 0;
    let mut limit_count = None;
    let mut i = 3;

    while i < args.len() {
        if let Ok(s) = std::str::from_utf8(&args[i]) {
            match s.to_uppercase().as_str() {
                "WITHSCORES" => withscores = true,
                "LIMIT" => {
                    if i + 2 >= args.len() {
                        return RespValue::Error("ERR syntax error".to_string());
                    }
                    i += 1;
                    limit_offset = match std::str::from_utf8(&args[i]) {
                        Ok(s) => match s.parse::<usize>() {
                            Ok(n) => n,
                            Err(_) => return RespValue::Error("ERR value is not an integer".to_string()),
                        },
                        Err(_) => return RespValue::Error("ERR value is not an integer".to_string()),
                    };
                    i += 1;
                    limit_count = match std::str::from_utf8(&args[i]) {
                        Ok(s) => match s.parse::<i64>() {
                            Ok(n) if n >= 0 => Some(n as usize),
                            _ => return RespValue::Error("ERR value is out of range".to_string()),
                        },
                        Err(_) => return RespValue::Error("ERR value is not an integer".to_string()),
                    };
                }
                _ => return RespValue::Error("ERR syntax error".to_string()),
            }
        }
        i += 1;
    }

    let db_instance = match db.get_db(db_index) {
        Some(d) => d,
        None => return RespValue::Error("ERR invalid database".to_string()),
    };

    let zset = match db_instance.get(key) {
        Some(RedisValue::ZSet(z)) => z,
        Some(_) => {
            return RespValue::Error(
                "WRONGTYPE Operation against a key holding the wrong kind of value".to_string(),
            )
        }
        None => return RespValue::Array(Some(vec![])),
    };

    // Collect matching members (reversed order)
    let results: Vec<_> = zset
        .scores
        .iter()
        .filter(|((score, _), _)| score.0 >= min_score && score.0 <= max_score)
        .rev() // Reverse order
        .skip(limit_offset)
        .take(limit_count.unwrap_or(usize::MAX))
        .collect();

    let mut output = Vec::new();
    for ((score, member), _) in results {
        output.push(RespValue::BulkString(Some(member.to_vec())));
        if withscores {
            output.push(RespValue::BulkString(Some(format!("{}", score.0).into_bytes())));
        }
    }

    RespValue::Array(Some(output))
}

/// ZLEXCOUNT key min max
/// Count members between lexicographical range
pub async fn zlexcount(db: &Arc<Database>, db_index: usize, args: Vec<Vec<u8>>) -> RespValue {
    if args.len() != 3 {
        return RespValue::Error("ERR wrong number of arguments for 'zlexcount' command".to_string());
    }

    let key = match std::str::from_utf8(&args[0]) {
        Ok(s) => s,
        Err(_) => return RespValue::Error("ERR invalid key".to_string()),
    };

    let min_lex = &args[1];
    let max_lex = &args[2];

    let db_instance = match db.get_db(db_index) {
        Some(d) => d,
        None => return RespValue::Error("ERR invalid database".to_string()),
    };

    let zset = match db_instance.get(key) {
        Some(RedisValue::ZSet(z)) => z,
        Some(_) => {
            return RespValue::Error(
                "WRONGTYPE Operation against a key holding the wrong kind of value".to_string(),
            )
        }
        None => return RespValue::Integer(0),
    };

    let count = zset
        .scores
        .iter()
        .filter(|((_, member), _)| {
            lex_match(member, min_lex, max_lex)
        })
        .count();

    RespValue::Integer(count as i64)
}

/// ZRANGEBYLEX key min max [LIMIT offset count]
/// Return members between lexicographical range
pub async fn zrangebylex(db: &Arc<Database>, db_index: usize, args: Vec<Vec<u8>>) -> RespValue {
    if args.len() < 3 {
        return RespValue::Error("ERR wrong number of arguments for 'zrangebylex' command".to_string());
    }

    let key = match std::str::from_utf8(&args[0]) {
        Ok(s) => s,
        Err(_) => return RespValue::Error("ERR invalid key".to_string()),
    };

    let min_lex = &args[1];
    let max_lex = &args[2];

    let mut limit_offset = 0;
    let mut limit_count = None;

    if args.len() > 3 {
        if let Ok(s) = std::str::from_utf8(&args[3]) {
            if s.to_uppercase() == "LIMIT" {
                if args.len() < 6 {
                    return RespValue::Error("ERR syntax error".to_string());
                }
                limit_offset = match std::str::from_utf8(&args[4]) {
                    Ok(s) => match s.parse::<usize>() {
                        Ok(n) => n,
                        Err(_) => return RespValue::Error("ERR value is not an integer".to_string()),
                    },
                    Err(_) => return RespValue::Error("ERR value is not an integer".to_string()),
                };
                limit_count = match std::str::from_utf8(&args[5]) {
                    Ok(s) => match s.parse::<i64>() {
                        Ok(n) if n >= 0 => Some(n as usize),
                        _ => return RespValue::Error("ERR value is out of range".to_string()),
                    },
                    Err(_) => return RespValue::Error("ERR value is not an integer".to_string()),
                };
            }
        }
    }

    let db_instance = match db.get_db(db_index) {
        Some(d) => d,
        None => return RespValue::Error("ERR invalid database".to_string()),
    };

    let zset = match db_instance.get(key) {
        Some(RedisValue::ZSet(z)) => z,
        Some(_) => {
            return RespValue::Error(
                "WRONGTYPE Operation against a key holding the wrong kind of value".to_string(),
            )
        }
        None => return RespValue::Array(Some(vec![])),
    };

    let results: Vec<RespValue> = zset
        .scores
        .iter()
        .filter(|((_, member), _)| lex_match(member, min_lex, max_lex))
        .skip(limit_offset)
        .take(limit_count.unwrap_or(usize::MAX))
        .map(|((_, member), _)| RespValue::BulkString(Some(member.to_vec())))
        .collect();

    RespValue::Array(Some(results))
}

/// ZREVRANGEBYLEX key max min [LIMIT offset count]
/// Return members in reverse lexicographical range
pub async fn zrevrangebylex(db: &Arc<Database>, db_index: usize, args: Vec<Vec<u8>>) -> RespValue {
    if args.len() < 3 {
        return RespValue::Error("ERR wrong number of arguments for 'zrevrangebylex' command".to_string());
    }

    let key = match std::str::from_utf8(&args[0]) {
        Ok(s) => s,
        Err(_) => return RespValue::Error("ERR invalid key".to_string()),
    };

    let max_lex = &args[1];
    let min_lex = &args[2];

    let mut limit_offset = 0;
    let mut limit_count = None;

    if args.len() > 3 {
        if let Ok(s) = std::str::from_utf8(&args[3]) {
            if s.to_uppercase() == "LIMIT" {
                if args.len() < 6 {
                    return RespValue::Error("ERR syntax error".to_string());
                }
                limit_offset = match std::str::from_utf8(&args[4]) {
                    Ok(s) => match s.parse::<usize>() {
                        Ok(n) => n,
                        Err(_) => return RespValue::Error("ERR value is not an integer".to_string()),
                    },
                    Err(_) => return RespValue::Error("ERR value is not an integer".to_string()),
                };
                limit_count = match std::str::from_utf8(&args[5]) {
                    Ok(s) => match s.parse::<i64>() {
                        Ok(n) if n >= 0 => Some(n as usize),
                        _ => return RespValue::Error("ERR value is out of range".to_string()),
                    },
                    Err(_) => return RespValue::Error("ERR value is not an integer".to_string()),
                };
            }
        }
    }

    let db_instance = match db.get_db(db_index) {
        Some(d) => d,
        None => return RespValue::Error("ERR invalid database".to_string()),
    };

    let zset = match db_instance.get(key) {
        Some(RedisValue::ZSet(z)) => z,
        Some(_) => {
            return RespValue::Error(
                "WRONGTYPE Operation against a key holding the wrong kind of value".to_string(),
            )
        }
        None => return RespValue::Array(Some(vec![])),
    };

    let results: Vec<RespValue> = zset
        .scores
        .iter()
        .filter(|((_, member), _)| lex_match(member, min_lex, max_lex))
        .rev()
        .skip(limit_offset)
        .take(limit_count.unwrap_or(usize::MAX))
        .map(|((_, member), _)| RespValue::BulkString(Some(member.to_vec())))
        .collect();

    RespValue::Array(Some(results))
}

/// ZREMRANGEBYLEX key min max
/// Remove members between lexicographical range
pub async fn zremrangebylex(db: &Arc<Database>, db_index: usize, args: Vec<Vec<u8>>) -> RespValue {
    if args.len() != 3 {
        return RespValue::Error("ERR wrong number of arguments for 'zremrangebylex' command".to_string());
    }

    let key = match std::str::from_utf8(&args[0]) {
        Ok(s) => s.to_string(),
        Err(_) => return RespValue::Error("ERR invalid key".to_string()),
    };

    let min_lex = &args[1];
    let max_lex = &args[2];

    let db_instance = match db.get_db(db_index) {
        Some(d) => d,
        None => return RespValue::Error("ERR invalid database".to_string()),
    };

    let mut zset = match db_instance.get(&key) {
        Some(RedisValue::ZSet(z)) => z,
        Some(_) => {
            return RespValue::Error(
                "WRONGTYPE Operation against a key holding the wrong kind of value".to_string(),
            )
        }
        None => return RespValue::Integer(0),
    };

    let to_remove: Vec<_> = zset
        .scores
        .iter()
        .filter(|((_, member), _)| lex_match(member, min_lex, max_lex))
        .map(|((score, member), _)| (*score, member.clone()))
        .collect();

    let removed = to_remove.len();

    for (score, member) in to_remove {
        zset.scores.remove(&(score, member.clone()));
        zset.members.remove(&member);
    }

    if zset.is_empty() {
        db_instance.delete(&key);
    } else {
        db_instance.set(key, RedisValue::ZSet(zset));
    }

    RespValue::Integer(removed as i64)
}

/// ZSCAN key cursor [MATCH pattern] [COUNT count]
/// Incrementally iterate sorted set elements
pub async fn zscan(db: &Arc<Database>, db_index: usize, args: Vec<Vec<u8>>) -> RespValue {
    if args.len() < 2 {
        return RespValue::Error("ERR wrong number of arguments for 'zscan' command".to_string());
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

    let mut count = 10; // Default count
    let mut i = 2;

    while i < args.len() {
        if let Ok(s) = std::str::from_utf8(&args[i]) {
            match s.to_uppercase().as_str() {
                "COUNT" => {
                    i += 1;
                    if i < args.len() {
                        if let Ok(c_str) = std::str::from_utf8(&args[i]) {
                            count = c_str.parse::<usize>().unwrap_or(10);
                        }
                    }
                }
                "MATCH" => {
                    // Pattern matching not implemented in this basic version
                    i += 1;
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

    let zset = match db_instance.get(key) {
        Some(RedisValue::ZSet(z)) => z,
        Some(_) => {
            return RespValue::Error(
                "WRONGTYPE Operation against a key holding the wrong kind of value".to_string(),
            )
        }
        None => {
            return RespValue::Array(Some(vec![
                RespValue::BulkString(Some(b"0".to_vec())),
                RespValue::Array(Some(vec![])),
            ]));
        }
    };

    let members: Vec<_> = zset.scores.iter().collect();
    let start = cursor;
    let end = (start + count).min(members.len());
    let next_cursor = if end >= members.len() { 0 } else { end };

    let mut results = Vec::new();
    for i in start..end {
        if let Some(((score, member), _)) = members.get(i) {
            results.push(RespValue::BulkString(Some(member.to_vec())));
            results.push(RespValue::BulkString(Some(format!("{}", score.0).into_bytes())));
        }
    }

    RespValue::Array(Some(vec![
        RespValue::BulkString(Some(next_cursor.to_string().into_bytes())),
        RespValue::Array(Some(results)),
    ]))
}

// Helper function for lexicographical matching
fn lex_match(member: &Bytes, min: &[u8], max: &[u8]) -> bool {
    let min_inclusive = min.get(0) == Some(&b'[');
    let max_inclusive = max.get(0) == Some(&b'[');

    let min_val = if min == b"-" {
        return true; // -inf
    } else if min == b"+" {
        return false; // +inf
    } else if min_inclusive || min.get(0) == Some(&b'(') {
        &min[1..]
    } else {
        min
    };

    let max_val = if max == b"+" {
        return true; // +inf
    } else if max == b"-" {
        return false; // -inf
    } else if max_inclusive || max.get(0) == Some(&b'(') {
        &max[1..]
    } else {
        max
    };

    let min_check = if min == b"-" {
        true
    } else if min_inclusive {
        member.as_ref() >= min_val
    } else {
        member.as_ref() > min_val
    };

    let max_check = if max == b"+" {
        true
    } else if max_inclusive {
        member.as_ref() <= max_val
    } else {
        member.as_ref() < max_val
    };

    min_check && max_check
}

// Helper function to parse score range (handles -inf, +inf)
fn parse_score_range(bytes: &[u8]) -> Result<f64, String> {
    let s = match std::str::from_utf8(bytes) {
        Ok(s) => s,
        Err(_) => return Err("ERR min or max is not a float".to_string()),
    };

    match s {
        "-inf" => Ok(f64::NEG_INFINITY),
        "+inf" | "inf" => Ok(f64::INFINITY),
        _ => s
            .parse::<f64>()
            .map_err(|_| "ERR min or max is not a float".to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_zadd_zscore() {
        let db = Arc::new(Database::new(16));

        let result = zadd(
            &db,
            0,
            vec![
                b"myzset".to_vec(),
                b"1.5".to_vec(),
                b"member1".to_vec(),
                b"2.0".to_vec(),
                b"member2".to_vec(),
            ],
        )
        .await;
        assert_eq!(result, RespValue::Integer(2));

        let result = zscore(&db, 0, vec![b"myzset".to_vec(), b"member1".to_vec()]).await;
        assert_eq!(
            result,
            RespValue::BulkString(Some(b"1.5".to_vec()))
        );
    }

    #[tokio::test]
    async fn test_zcard() {
        let db = Arc::new(Database::new(16));

        zadd(
            &db,
            0,
            vec![
                b"myzset".to_vec(),
                b"1".to_vec(),
                b"a".to_vec(),
                b"2".to_vec(),
                b"b".to_vec(),
            ],
        )
        .await;

        let result = zcard(&db, 0, vec![b"myzset".to_vec()]).await;
        assert_eq!(result, RespValue::Integer(2));
    }

    #[tokio::test]
    async fn test_zrange() {
        let db = Arc::new(Database::new(16));

        zadd(
            &db,
            0,
            vec![
                b"myzset".to_vec(),
                b"1".to_vec(),
                b"a".to_vec(),
                b"2".to_vec(),
                b"b".to_vec(),
                b"3".to_vec(),
                b"c".to_vec(),
            ],
        )
        .await;

        let result = zrange(
            &db,
            0,
            vec![b"myzset".to_vec(), b"0".to_vec(), b"1".to_vec()],
        )
        .await;
        if let RespValue::Array(Some(arr)) = result {
            assert_eq!(arr.len(), 2);
        } else {
            panic!("Expected array");
        }
    }

    #[tokio::test]
    async fn test_zrank() {
        let db = Arc::new(Database::new(16));

        zadd(
            &db,
            0,
            vec![
                b"myzset".to_vec(),
                b"1".to_vec(),
                b"a".to_vec(),
                b"2".to_vec(),
                b"b".to_vec(),
                b"3".to_vec(),
                b"c".to_vec(),
            ],
        )
        .await;

        let result = zrank(&db, 0, vec![b"myzset".to_vec(), b"b".to_vec()]).await;
        assert_eq!(result, RespValue::Integer(1));

        let result = zrevrank(&db, 0, vec![b"myzset".to_vec(), b"b".to_vec()]).await;
        assert_eq!(result, RespValue::Integer(1));
    }
}

/// ZMSCORE key member [member ...]
/// Get scores for multiple members
pub async fn zmscore(db: &Arc<Database>, db_index: usize, args: Vec<Vec<u8>>) -> RespValue {
    if args.len() < 2 {
        return RespValue::Error("ERR wrong number of arguments for 'zmscore' command".to_string());
    }

    let key = match std::str::from_utf8(&args[0]) {
        Ok(s) => s,
        Err(_) => return RespValue::Error("ERR invalid key".to_string()),
    };

    let db_instance = match db.get_db(db_index) {
        Some(d) => d,
        None => return RespValue::Error("ERR invalid database".to_string()),
    };

    let zset = match db_instance.get(key) {
        Some(RedisValue::ZSet(z)) => z,
        Some(_) => {
            return RespValue::Error(
                "WRONGTYPE Operation against a key holding the wrong kind of value".to_string(),
            )
        }
        None => {
            // If key doesn't exist, return array of nulls
            let nulls = vec![RespValue::BulkString(None); args.len() - 1];
            return RespValue::Array(Some(nulls));
        }
    };

    let mut results = Vec::new();
    for member_bytes in &args[1..] {
        let member = Bytes::from(member_bytes.clone());
        if let Some(&score) = zset.members.get(&member) {
            results.push(RespValue::BulkString(Some(format!("{}", score).into_bytes())));
        } else {
            results.push(RespValue::BulkString(None));
        }
    }

    RespValue::Array(Some(results))
}

/// ZDIFF numkeys key [key ...] [WITHSCORES]
/// Compute difference between first and successive sets
pub async fn zdiff(db: &Arc<Database>, db_index: usize, args: Vec<Vec<u8>>) -> RespValue {
    if args.len() < 2 {
        return RespValue::Error("ERR wrong number of arguments for 'zdiff' command".to_string());
    }

    let numkeys: usize = match std::str::from_utf8(&args[0]) {
        Ok(s) => match s.parse() {
            Ok(n) => n,
            Err(_) => return RespValue::Error("ERR value is not an integer or out of range".to_string()),
        },
        Err(_) => return RespValue::Error("ERR value is not an integer or out of range".to_string()),
    };

    if numkeys < 1 {
        return RespValue::Error("ERR at least 1 input key is needed for ZDIFF".to_string());
    }

    if args.len() < numkeys + 1 {
        return RespValue::Error("ERR syntax error".to_string());
    }

    let with_scores = if args.len() > numkeys + 1 {
        match std::str::from_utf8(&args[numkeys + 1]) {
            Ok(s) => s.to_uppercase() == "WITHSCORES",
            Err(_) => false,
        }
    } else {
        false
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

    let mut result_zset = match db_instance.get(first_key) {
        Some(RedisValue::ZSet(z)) => z.clone(),
        Some(_) => {
            return RespValue::Error(
                "WRONGTYPE Operation against a key holding the wrong kind of value".to_string(),
            )
        }
        None => ZSet::new(),
    };

    // Remove members from subsequent sets
    for i in 2..=numkeys {
        let key = match std::str::from_utf8(&args[i]) {
            Ok(s) => s,
            Err(_) => continue,
        };

        if let Some(RedisValue::ZSet(zset)) = db_instance.get(key) {
            for member in zset.members.keys() {
                if let Some(&score) = result_zset.members.get(member) {
                    result_zset.scores.remove(&(OrderedFloat(score), member.clone()));
                    result_zset.members.remove(member);
                }
            }
        }
    }

    // Build response
    let mut response = Vec::new();
    for ((_, member), _) in result_zset.scores.iter() {
        response.push(RespValue::BulkString(Some(member.to_vec())));
        if with_scores {
            let score = result_zset.members.get(member).unwrap();
            response.push(RespValue::BulkString(Some(format!("{}", score).into_bytes())));
        }
    }

    RespValue::Array(Some(response))
}

/// ZDIFFSTORE destination numkeys key [key ...]
/// Store difference between first and successive sets
pub async fn zdiffstore(db: &Arc<Database>, db_index: usize, args: Vec<Vec<u8>>) -> RespValue {
    if args.len() < 3 {
        return RespValue::Error("ERR wrong number of arguments for 'zdiffstore' command".to_string());
    }

    let dest = match std::str::from_utf8(&args[0]) {
        Ok(s) => s.to_string(),
        Err(_) => return RespValue::Error("ERR invalid destination key".to_string()),
    };

    let numkeys: usize = match std::str::from_utf8(&args[1]) {
        Ok(s) => match s.parse() {
            Ok(n) => n,
            Err(_) => return RespValue::Error("ERR value is not an integer or out of range".to_string()),
        },
        Err(_) => return RespValue::Error("ERR value is not an integer or out of range".to_string()),
    };

    if numkeys < 1 {
        return RespValue::Error("ERR at least 1 input key is needed for ZDIFFSTORE".to_string());
    }

    if args.len() < numkeys + 2 {
        return RespValue::Error("ERR syntax error".to_string());
    }

    let db_instance = match db.get_db(db_index) {
        Some(d) => d,
        None => return RespValue::Error("ERR invalid database".to_string()),
    };

    // Get first set
    let first_key = match std::str::from_utf8(&args[2]) {
        Ok(s) => s,
        Err(_) => return RespValue::Error("ERR invalid key".to_string()),
    };

    let mut result_zset = match db_instance.get(first_key) {
        Some(RedisValue::ZSet(z)) => z.clone(),
        Some(_) => {
            return RespValue::Error(
                "WRONGTYPE Operation against a key holding the wrong kind of value".to_string(),
            )
        }
        None => ZSet::new(),
    };

    // Remove members from subsequent sets
    for i in 3..(numkeys + 2) {
        if i >= args.len() {
            break;
        }
        let key = match std::str::from_utf8(&args[i]) {
            Ok(s) => s,
            Err(_) => continue,
        };

        if let Some(RedisValue::ZSet(zset)) = db_instance.get(key) {
            for member in zset.members.keys() {
                if let Some(&score) = result_zset.members.get(member) {
                    result_zset.scores.remove(&(OrderedFloat(score), member.clone()));
                    result_zset.members.remove(member);
                }
            }
        }
    }

    let count = result_zset.len() as i64;

    // Store result
    if result_zset.is_empty() {
        db_instance.delete(&dest);
    } else {
        db_instance.set(dest, RedisValue::ZSet(result_zset));
    }

    RespValue::Integer(count)
}

/// ZUNIONSTORE destination numkeys key [key ...] [WEIGHTS weight [weight ...]] [AGGREGATE SUM|MIN|MAX]
/// Compute union and store result
pub async fn zunionstore(db: &Arc<Database>, db_index: usize, args: Vec<Vec<u8>>) -> RespValue {
    if args.len() < 3 {
        return RespValue::Error("ERR wrong number of arguments for 'zunionstore' command".to_string());
    }

    let dest = match std::str::from_utf8(&args[0]) {
        Ok(s) => s.to_string(),
        Err(_) => return RespValue::Error("ERR invalid destination key".to_string()),
    };

    let numkeys: usize = match std::str::from_utf8(&args[1]) {
        Ok(s) => match s.parse() {
            Ok(n) => n,
            Err(_) => return RespValue::Error("ERR value is not an integer or out of range".to_string()),
        },
        Err(_) => return RespValue::Error("ERR value is not an integer or out of range".to_string()),
    };

    if numkeys < 1 {
        return RespValue::Error("ERR at least 1 input key is needed for ZUNIONSTORE".to_string());
    }

    if args.len() < numkeys + 2 {
        return RespValue::Error("ERR syntax error".to_string());
    }

    let db_instance = match db.get_db(db_index) {
        Some(d) => d,
        None => return RespValue::Error("ERR invalid database".to_string()),
    };

    // Parse weights and aggregate (simplified - default weights=1, aggregate=SUM)
    let weights = vec![1.0; numkeys];
    let aggregate = "SUM"; // Default

    // Collect all members with scores from all sets
    let mut result_members: std::collections::HashMap<Bytes, Vec<f64>> = std::collections::HashMap::new();

    for i in 0..numkeys {
        let key = match std::str::from_utf8(&args[2 + i]) {
            Ok(s) => s,
            Err(_) => continue,
        };

        if let Some(RedisValue::ZSet(zset)) = db_instance.get(key) {
            for (member, &score) in &zset.members {
                result_members
                    .entry(member.clone())
                    .or_insert_with(Vec::new)
                    .push(score * weights[i]);
            }
        }
    }

    // Create result zset with aggregated scores
    let mut result_zset = ZSet::new();
    for (member, scores) in result_members {
        let final_score = match aggregate {
            "MIN" => scores.iter().cloned().fold(f64::INFINITY, f64::min),
            "MAX" => scores.iter().cloned().fold(f64::NEG_INFINITY, f64::max),
            _ => scores.iter().sum(), // SUM
        };

        result_zset.members.insert(member.clone(), final_score);
        result_zset.scores.insert((OrderedFloat(final_score), member), ());
    }

    let count = result_zset.len() as i64;

    // Store result
    if result_zset.is_empty() {
        db_instance.delete(&dest);
    } else {
        db_instance.set(dest, RedisValue::ZSet(result_zset));
    }

    RespValue::Integer(count)
}

/// ZINTERSTORE destination numkeys key [key ...] [WEIGHTS weight [weight ...]] [AGGREGATE SUM|MIN|MAX]
/// Compute intersection and store result
pub async fn zinterstore(db: &Arc<Database>, db_index: usize, args: Vec<Vec<u8>>) -> RespValue {
    if args.len() < 3 {
        return RespValue::Error("ERR wrong number of arguments for 'zinterstore' command".to_string());
    }

    let dest = match std::str::from_utf8(&args[0]) {
        Ok(s) => s.to_string(),
        Err(_) => return RespValue::Error("ERR invalid destination key".to_string()),
    };

    let numkeys: usize = match std::str::from_utf8(&args[1]) {
        Ok(s) => match s.parse() {
            Ok(n) => n,
            Err(_) => return RespValue::Error("ERR value is not an integer or out of range".to_string()),
        },
        Err(_) => return RespValue::Error("ERR value is not an integer or out of range".to_string()),
    };

    if numkeys < 1 {
        return RespValue::Error("ERR at least 1 input key is needed for ZINTERSTORE".to_string());
    }

    if args.len() < numkeys + 2 {
        return RespValue::Error("ERR syntax error".to_string());
    }

    let db_instance = match db.get_db(db_index) {
        Some(d) => d,
        None => return RespValue::Error("ERR invalid database".to_string()),
    };

    // Parse weights and aggregate (simplified - default weights=1, aggregate=SUM)
    let weights = vec![1.0; numkeys];
    let aggregate = "SUM"; // Default

    // Get first set as base
    let first_key = match std::str::from_utf8(&args[2]) {
        Ok(s) => s,
        Err(_) => return RespValue::Error("ERR invalid key".to_string()),
    };

    let first_zset = match db_instance.get(first_key) {
        Some(RedisValue::ZSet(z)) => z,
        Some(_) => {
            return RespValue::Error(
                "WRONGTYPE Operation against a key holding the wrong kind of value".to_string(),
            )
        }
        None => {
            // If first set is empty, intersection is empty
            db_instance.delete(&dest);
            return RespValue::Integer(0);
        }
    };

    // Collect all members that exist in ALL sets
    let mut result_members: std::collections::HashMap<Bytes, Vec<f64>> = std::collections::HashMap::new();

    for (member, &score) in &first_zset.members {
        let mut scores_in_all = vec![score * weights[0]];
        let mut exists_in_all = true;

        // Check if member exists in all other sets
        for i in 1..numkeys {
            let key = match std::str::from_utf8(&args[2 + i]) {
                Ok(s) => s,
                Err(_) => {
                    exists_in_all = false;
                    break;
                }
            };

            if let Some(RedisValue::ZSet(zset)) = db_instance.get(key) {
                if let Some(&s) = zset.members.get(member) {
                    scores_in_all.push(s * weights[i]);
                } else {
                    exists_in_all = false;
                    break;
                }
            } else {
                // Set doesn't exist or wrong type - no intersection
                exists_in_all = false;
                break;
            }
        }

        if exists_in_all {
            result_members.insert(member.clone(), scores_in_all);
        }
    }

    // Create result zset with aggregated scores
    let mut result_zset = ZSet::new();
    for (member, scores) in result_members {
        let final_score = match aggregate {
            "MIN" => scores.iter().cloned().fold(f64::INFINITY, f64::min),
            "MAX" => scores.iter().cloned().fold(f64::NEG_INFINITY, f64::max),
            _ => scores.iter().sum(), // SUM
        };

        result_zset.members.insert(member.clone(), final_score);
        result_zset.scores.insert((OrderedFloat(final_score), member), ());
    }

    let count = result_zset.len() as i64;

    // Store result
    if result_zset.is_empty() {
        db_instance.delete(&dest);
    } else {
        db_instance.set(dest, RedisValue::ZSet(result_zset));
    }

    RespValue::Integer(count)
}

/// BZPOPMIN key [key ...] timeout
/// Blocking version of ZPOPMIN - removes and returns the element with lowest score
pub async fn bzpopmin(db: &Arc<Database>, db_index: usize, args: Vec<Vec<u8>>) -> RespValue {
    if args.len() < 2 {
        return RespValue::Error(
            "ERR wrong number of arguments for 'bzpopmin' command".to_string(),
        );
    }

    // Parse timeout (last argument)
    let timeout_bytes = &args[args.len() - 1];
    let timeout_str = match std::str::from_utf8(timeout_bytes) {
        Ok(s) => s,
        Err(_) => return RespValue::Error("ERR timeout is not a float or out of range".to_string()),
    };

    let timeout_secs: f64 = match timeout_str.parse() {
        Ok(t) => {
            if t < 0.0 {
                return RespValue::Error("ERR timeout is negative".to_string());
            }
            t
        }
        Err(_) => return RespValue::Error("ERR timeout is not a float or out of range".to_string()),
    };

    let db_instance = match db.get_db(db_index) {
        Some(d) => d,
        None => return RespValue::Error("ERR invalid database".to_string()),
    };

    // Keys to check (all except last which is timeout)
    let keys = &args[..args.len() - 1];

    // Poll with timeout
    let timeout_ms = (timeout_secs * 1000.0) as u64;
    let start = std::time::Instant::now();
    let poll_interval = std::time::Duration::from_millis(10);

    loop {
        // Try to pop from each key in order
        for key_bytes in keys {
            let key = match std::str::from_utf8(key_bytes) {
                Ok(s) => s,
                Err(_) => continue,
            };

            if let Some(RedisValue::ZSet(mut zset)) = db_instance.get(key) {
                if !zset.is_empty() {
                    // Find minimum score element (first in BTreeMap)
                    let min_entry = zset.scores.iter().next();

                    if let Some(((score_key, member), _)) = min_entry {
                        let member_clone = member.clone();
                        let score_clone = score_key.into_inner();

                        // Remove from both maps
                        zset.scores.remove(&(*score_key, member_clone.clone()));
                        zset.members.remove(&member_clone);

                        // Store back
                        if zset.is_empty() {
                            db_instance.delete(key);
                        } else {
                            db_instance.set(key.to_string(), RedisValue::ZSet(zset));
                        }

                        // Return [key, member, score]
                        return RespValue::Array(Some(vec![
                            RespValue::BulkString(Some(key.as_bytes().to_vec())),
                            RespValue::BulkString(Some(member_clone.to_vec())),
                            RespValue::BulkString(Some(score_clone.to_string().into_bytes())),
                        ]));
                    }
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

/// BZPOPMAX key [key ...] timeout
/// Blocking version of ZPOPMAX - removes and returns the element with highest score
pub async fn bzpopmax(db: &Arc<Database>, db_index: usize, args: Vec<Vec<u8>>) -> RespValue {
    if args.len() < 2 {
        return RespValue::Error(
            "ERR wrong number of arguments for 'bzpopmax' command".to_string(),
        );
    }

    // Parse timeout (last argument)
    let timeout_bytes = &args[args.len() - 1];
    let timeout_str = match std::str::from_utf8(timeout_bytes) {
        Ok(s) => s,
        Err(_) => return RespValue::Error("ERR timeout is not a float or out of range".to_string()),
    };

    let timeout_secs: f64 = match timeout_str.parse() {
        Ok(t) => {
            if t < 0.0 {
                return RespValue::Error("ERR timeout is negative".to_string());
            }
            t
        }
        Err(_) => return RespValue::Error("ERR timeout is not a float or out of range".to_string()),
    };

    let db_instance = match db.get_db(db_index) {
        Some(d) => d,
        None => return RespValue::Error("ERR invalid database".to_string()),
    };

    // Keys to check (all except last which is timeout)
    let keys = &args[..args.len() - 1];

    // Poll with timeout
    let timeout_ms = (timeout_secs * 1000.0) as u64;
    let start = std::time::Instant::now();
    let poll_interval = std::time::Duration::from_millis(10);

    loop {
        // Try to pop from each key in order
        for key_bytes in keys {
            let key = match std::str::from_utf8(key_bytes) {
                Ok(s) => s,
                Err(_) => continue,
            };

            if let Some(RedisValue::ZSet(mut zset)) = db_instance.get(key) {
                if !zset.is_empty() {
                    // Find maximum score element (last in BTreeMap)
                    let max_entry = zset.scores.iter().next_back();

                    if let Some(((score_key, member), _)) = max_entry {
                        let member_clone = member.clone();
                        let score_clone = score_key.into_inner();

                        // Remove from both maps
                        zset.scores.remove(&(*score_key, member_clone.clone()));
                        zset.members.remove(&member_clone);

                        // Store back
                        if zset.is_empty() {
                            db_instance.delete(key);
                        } else {
                            db_instance.set(key.to_string(), RedisValue::ZSet(zset));
                        }

                        // Return [key, member, score]
                        return RespValue::Array(Some(vec![
                            RespValue::BulkString(Some(key.as_bytes().to_vec())),
                            RespValue::BulkString(Some(member_clone.to_vec())),
                            RespValue::BulkString(Some(score_clone.to_string().into_bytes())),
                        ]));
                    }
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
