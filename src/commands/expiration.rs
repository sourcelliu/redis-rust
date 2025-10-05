// Key expiration command handlers

use crate::protocol::RespValue;
use crate::storage::db::{current_timestamp_ms, Database};
use std::sync::Arc;

/// EXPIRE key seconds
pub async fn expire(db: &Arc<Database>, db_index: usize, args: Vec<Vec<u8>>) -> RespValue {
    if args.len() != 2 {
        return RespValue::Error("ERR wrong number of arguments for 'expire' command".to_string());
    }

    let key = match std::str::from_utf8(&args[0]) {
        Ok(s) => s,
        Err(_) => return RespValue::Error("ERR invalid key".to_string()),
    };

    let seconds = match std::str::from_utf8(&args[1]) {
        Ok(s) => match s.parse::<i64>() {
            Ok(n) => n,
            Err(_) => return RespValue::Error("ERR value is not an integer".to_string()),
        },
        Err(_) => return RespValue::Error("ERR value is not an integer".to_string()),
    };

    let db_instance = match db.get_db(db_index) {
        Some(d) => d,
        None => return RespValue::Error("ERR invalid database".to_string()),
    };

    let expire_at_ms = current_timestamp_ms() + (seconds as u64 * 1000);
    if db_instance.set_expiry(key, expire_at_ms) {
        RespValue::Integer(1)
    } else {
        RespValue::Integer(0)
    }
}

/// EXPIREAT key timestamp
pub async fn expireat(db: &Arc<Database>, db_index: usize, args: Vec<Vec<u8>>) -> RespValue {
    if args.len() != 2 {
        return RespValue::Error(
            "ERR wrong number of arguments for 'expireat' command".to_string(),
        );
    }

    let key = match std::str::from_utf8(&args[0]) {
        Ok(s) => s,
        Err(_) => return RespValue::Error("ERR invalid key".to_string()),
    };

    let timestamp = match std::str::from_utf8(&args[1]) {
        Ok(s) => match s.parse::<i64>() {
            Ok(n) => n,
            Err(_) => return RespValue::Error("ERR value is not an integer".to_string()),
        },
        Err(_) => return RespValue::Error("ERR value is not an integer".to_string()),
    };

    let db_instance = match db.get_db(db_index) {
        Some(d) => d,
        None => return RespValue::Error("ERR invalid database".to_string()),
    };

    let expire_at_ms = (timestamp as u64) * 1000;
    if db_instance.set_expiry(key, expire_at_ms) {
        RespValue::Integer(1)
    } else {
        RespValue::Integer(0)
    }
}

/// PEXPIRE key milliseconds
pub async fn pexpire(db: &Arc<Database>, db_index: usize, args: Vec<Vec<u8>>) -> RespValue {
    if args.len() != 2 {
        return RespValue::Error(
            "ERR wrong number of arguments for 'pexpire' command".to_string(),
        );
    }

    let key = match std::str::from_utf8(&args[0]) {
        Ok(s) => s,
        Err(_) => return RespValue::Error("ERR invalid key".to_string()),
    };

    let milliseconds = match std::str::from_utf8(&args[1]) {
        Ok(s) => match s.parse::<i64>() {
            Ok(n) => n,
            Err(_) => return RespValue::Error("ERR value is not an integer".to_string()),
        },
        Err(_) => return RespValue::Error("ERR value is not an integer".to_string()),
    };

    let db_instance = match db.get_db(db_index) {
        Some(d) => d,
        None => return RespValue::Error("ERR invalid database".to_string()),
    };

    let expire_at_ms = current_timestamp_ms() + milliseconds as u64;
    if db_instance.set_expiry(key, expire_at_ms) {
        RespValue::Integer(1)
    } else {
        RespValue::Integer(0)
    }
}

/// PEXPIREAT key milliseconds-timestamp
pub async fn pexpireat(db: &Arc<Database>, db_index: usize, args: Vec<Vec<u8>>) -> RespValue {
    if args.len() != 2 {
        return RespValue::Error(
            "ERR wrong number of arguments for 'pexpireat' command".to_string(),
        );
    }

    let key = match std::str::from_utf8(&args[0]) {
        Ok(s) => s,
        Err(_) => return RespValue::Error("ERR invalid key".to_string()),
    };

    let timestamp_ms = match std::str::from_utf8(&args[1]) {
        Ok(s) => match s.parse::<u64>() {
            Ok(n) => n,
            Err(_) => return RespValue::Error("ERR value is not an integer".to_string()),
        },
        Err(_) => return RespValue::Error("ERR value is not an integer".to_string()),
    };

    let db_instance = match db.get_db(db_index) {
        Some(d) => d,
        None => return RespValue::Error("ERR invalid database".to_string()),
    };

    if db_instance.set_expiry(key, timestamp_ms) {
        RespValue::Integer(1)
    } else {
        RespValue::Integer(0)
    }
}

/// TTL key
pub async fn ttl(db: &Arc<Database>, db_index: usize, args: Vec<Vec<u8>>) -> RespValue {
    if args.len() != 1 {
        return RespValue::Error("ERR wrong number of arguments for 'ttl' command".to_string());
    }

    let key = match std::str::from_utf8(&args[0]) {
        Ok(s) => s,
        Err(_) => return RespValue::Error("ERR invalid key".to_string()),
    };

    let db_instance = match db.get_db(db_index) {
        Some(d) => d,
        None => return RespValue::Error("ERR invalid database".to_string()),
    };

    let ttl_ms = db_instance.get_ttl_ms(key);
    if ttl_ms == -2 {
        RespValue::Integer(-2) // Key doesn't exist
    } else if ttl_ms == -1 {
        RespValue::Integer(-1) // Key exists but has no expiration
    } else {
        RespValue::Integer((ttl_ms / 1000) as i64) // Convert to seconds
    }
}

/// PTTL key
pub async fn pttl(db: &Arc<Database>, db_index: usize, args: Vec<Vec<u8>>) -> RespValue {
    if args.len() != 1 {
        return RespValue::Error("ERR wrong number of arguments for 'pttl' command".to_string());
    }

    let key = match std::str::from_utf8(&args[0]) {
        Ok(s) => s,
        Err(_) => return RespValue::Error("ERR invalid key".to_string()),
    };

    let db_instance = match db.get_db(db_index) {
        Some(d) => d,
        None => return RespValue::Error("ERR invalid database".to_string()),
    };

    let ttl_ms = db_instance.get_ttl_ms(key);
    RespValue::Integer(ttl_ms)
}

/// PERSIST key
pub async fn persist(db: &Arc<Database>, db_index: usize, args: Vec<Vec<u8>>) -> RespValue {
    if args.len() != 1 {
        return RespValue::Error("ERR wrong number of arguments for 'persist' command".to_string());
    }

    let key = match std::str::from_utf8(&args[0]) {
        Ok(s) => s,
        Err(_) => return RespValue::Error("ERR invalid key".to_string()),
    };

    let db_instance = match db.get_db(db_index) {
        Some(d) => d,
        None => return RespValue::Error("ERR invalid database".to_string()),
    };

    if db_instance.persist(key) {
        RespValue::Integer(1)
    } else {
        RespValue::Integer(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::types::RedisValue;
    use bytes::Bytes;

    #[tokio::test]
    async fn test_expire_ttl() {
        let db = Arc::new(Database::new(16));
        let db_instance = db.get_db(0).unwrap();

        // Set a key
        db_instance.set("mykey".to_string(), RedisValue::String(Bytes::from("value")));

        // Set expiration
        let result = expire(&db, 0, vec![b"mykey".to_vec(), b"10".to_vec()]).await;
        assert_eq!(result, RespValue::Integer(1));

        // Check TTL
        let result = ttl(&db, 0, vec![b"mykey".to_vec()]).await;
        if let RespValue::Integer(ttl_val) = result {
            assert!(ttl_val >= 9 && ttl_val <= 10);
        } else {
            panic!("Expected integer TTL");
        }

        // Check PTTL
        let result = pttl(&db, 0, vec![b"mykey".to_vec()]).await;
        if let RespValue::Integer(pttl_val) = result {
            assert!(pttl_val >= 9000 && pttl_val <= 10000);
        } else {
            panic!("Expected integer PTTL");
        }
    }

    #[tokio::test]
    async fn test_persist() {
        let db = Arc::new(Database::new(16));
        let db_instance = db.get_db(0).unwrap();

        // Set a key with expiration
        db_instance.set("mykey".to_string(), RedisValue::String(Bytes::from("value")));
        expire(&db, 0, vec![b"mykey".to_vec(), b"100".to_vec()]).await;

        // Remove expiration
        let result = persist(&db, 0, vec![b"mykey".to_vec()]).await;
        assert_eq!(result, RespValue::Integer(1));

        // Check TTL is now -1
        let result = ttl(&db, 0, vec![b"mykey".to_vec()]).await;
        assert_eq!(result, RespValue::Integer(-1));
    }

    #[tokio::test]
    async fn test_ttl_nonexistent_key() {
        let db = Arc::new(Database::new(16));

        let result = ttl(&db, 0, vec![b"nonexistent".to_vec()]).await;
        assert_eq!(result, RespValue::Integer(-2));
    }

    #[tokio::test]
    async fn test_pexpire() {
        let db = Arc::new(Database::new(16));
        let db_instance = db.get_db(0).unwrap();

        db_instance.set("mykey".to_string(), RedisValue::String(Bytes::from("value")));

        let result = pexpire(&db, 0, vec![b"mykey".to_vec(), b"5000".to_vec()]).await;
        assert_eq!(result, RespValue::Integer(1));

        let result = pttl(&db, 0, vec![b"mykey".to_vec()]).await;
        if let RespValue::Integer(pttl_val) = result {
            assert!(pttl_val >= 4900 && pttl_val <= 5000);
        } else {
            panic!("Expected integer PTTL");
        }
    }
}
