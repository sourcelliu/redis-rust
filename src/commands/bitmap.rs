// Bitmap commands for Redis-Rust

use crate::protocol::RespValue;
use crate::storage::db::Database;
use crate::storage::RedisValue;
use bytes::Bytes;
use std::sync::Arc;

/// SETBIT key offset value
/// Sets or clears the bit at offset in the string value stored at key
pub async fn setbit(db: &Arc<Database>, db_index: usize, args: Vec<Vec<u8>>) -> RespValue {
    if args.len() != 3 {
        return RespValue::Error(
            "ERR wrong number of arguments for 'setbit' command".to_string(),
        );
    }

    let key = match std::str::from_utf8(&args[0]) {
        Ok(s) => s,
        Err(_) => return RespValue::Error("ERR invalid key".to_string()),
    };

    let offset = match std::str::from_utf8(&args[1]) {
        Ok(s) => match s.parse::<usize>() {
            Ok(n) => n,
            Err(_) => return RespValue::Error("ERR bit offset is not an integer or out of range".to_string()),
        },
        Err(_) => return RespValue::Error("ERR bit offset is not an integer or out of range".to_string()),
    };

    let value = match std::str::from_utf8(&args[2]) {
        Ok(s) => match s {
            "0" => 0u8,
            "1" => 1u8,
            _ => return RespValue::Error("ERR bit is not an integer or out of range".to_string()),
        },
        Err(_) => return RespValue::Error("ERR bit is not an integer or out of range".to_string()),
    };

    // Redis limits offset to 2^32-1 (512MB)
    if offset >= (1 << 32) {
        return RespValue::Error("ERR bit offset is not an integer or out of range".to_string());
    }

    let db_instance = db.get_db(db_index).unwrap();

    // Get or create string value
    let mut bytes = match db_instance.get(key) {
        Some(val) => match val.as_string() {
            Some(s) => s.to_vec(),
            None => return RespValue::Error("WRONGTYPE Operation against a key holding the wrong kind of value".to_string()),
        },
        None => Vec::new(),
    };

    // Calculate byte position and bit position within byte
    let byte_offset = offset / 8;
    let bit_offset = 7 - (offset % 8); // Redis uses big-endian bit ordering

    // Expand string if needed
    if byte_offset >= bytes.len() {
        bytes.resize(byte_offset + 1, 0);
    }

    // Get old bit value
    let old_value = (bytes[byte_offset] >> bit_offset) & 1;

    // Set or clear the bit
    if value == 1 {
        bytes[byte_offset] |= 1 << bit_offset;
    } else {
        bytes[byte_offset] &= !(1 << bit_offset);
    }

    // Store back
    db_instance.set(key.to_string(), RedisValue::String(Bytes::from(bytes)));

    RespValue::Integer(old_value as i64)
}

/// GETBIT key offset
/// Returns the bit value at offset in the string value stored at key
pub async fn getbit(db: &Arc<Database>, db_index: usize, args: Vec<Vec<u8>>) -> RespValue {
    if args.len() != 2 {
        return RespValue::Error(
            "ERR wrong number of arguments for 'getbit' command".to_string(),
        );
    }

    let key = match std::str::from_utf8(&args[0]) {
        Ok(s) => s,
        Err(_) => return RespValue::Error("ERR invalid key".to_string()),
    };

    let offset = match std::str::from_utf8(&args[1]) {
        Ok(s) => match s.parse::<usize>() {
            Ok(n) => n,
            Err(_) => return RespValue::Error("ERR bit offset is not an integer or out of range".to_string()),
        },
        Err(_) => return RespValue::Error("ERR bit offset is not an integer or out of range".to_string()),
    };

    if offset >= (1 << 32) {
        return RespValue::Error("ERR bit offset is not an integer or out of range".to_string());
    }

    let db_instance = db.get_db(db_index).unwrap();

    match db_instance.get(key) {
        Some(val) => match val.as_string() {
            Some(s) => {
                let byte_offset = offset / 8;
                let bit_offset = 7 - (offset % 8);

                if byte_offset >= s.len() {
                    // Beyond string length, return 0
                    return RespValue::Integer(0);
                }

                let bit_value = (s[byte_offset] >> bit_offset) & 1;
                RespValue::Integer(bit_value as i64)
            }
            None => RespValue::Error("WRONGTYPE Operation against a key holding the wrong kind of value".to_string()),
        },
        None => RespValue::Integer(0), // Key doesn't exist, return 0
    }
}

/// BITCOUNT key [start end [BYTE|BIT]]
/// Count set bits in a string
pub async fn bitcount(db: &Arc<Database>, db_index: usize, args: Vec<Vec<u8>>) -> RespValue {
    if args.is_empty() {
        return RespValue::Error(
            "ERR wrong number of arguments for 'bitcount' command".to_string(),
        );
    }

    let key = match std::str::from_utf8(&args[0]) {
        Ok(s) => s,
        Err(_) => return RespValue::Error("ERR invalid key".to_string()),
    };

    let db_instance = db.get_db(db_index).unwrap();

    let value = match db_instance.get(key) {
        Some(val) => match val.as_string() {
            Some(s) => s.to_vec(),
            None => return RespValue::Error("WRONGTYPE Operation against a key holding the wrong kind of value".to_string()),
        },
        None => return RespValue::Integer(0), // Empty string has 0 bits set
    };

    // Parse start/end range if provided (simplified version - byte mode only)
    let (start, end) = if args.len() >= 3 {
        let start_val = match std::str::from_utf8(&args[1]) {
            Ok(s) => match s.parse::<i64>() {
                Ok(n) => n,
                Err(_) => return RespValue::Error("ERR value is not an integer or out of range".to_string()),
            },
            Err(_) => return RespValue::Error("ERR value is not an integer or out of range".to_string()),
        };

        let end_val = match std::str::from_utf8(&args[2]) {
            Ok(s) => match s.parse::<i64>() {
                Ok(n) => n,
                Err(_) => return RespValue::Error("ERR value is not an integer or out of range".to_string()),
            },
            Err(_) => return RespValue::Error("ERR value is not an integer or out of range".to_string()),
        };

        // Convert negative indices
        let len = value.len() as i64;
        let start = if start_val < 0 { (len + start_val).max(0) } else { start_val.min(len) };
        let end = if end_val < 0 { (len + end_val).max(-1) } else { end_val.min(len - 1) };

        (start as usize, end as usize)
    } else {
        (0, value.len().saturating_sub(1))
    };

    // Count bits in range
    let mut count = 0u64;
    for i in start..=end.min(value.len().saturating_sub(1)) {
        count += value[i].count_ones() as u64;
    }

    RespValue::Integer(count as i64)
}

/// BITPOS key bit [start [end [BYTE|BIT]]]
/// Find first bit set or clear in a string
pub async fn bitpos(db: &Arc<Database>, db_index: usize, args: Vec<Vec<u8>>) -> RespValue {
    if args.len() < 2 {
        return RespValue::Error(
            "ERR wrong number of arguments for 'bitpos' command".to_string(),
        );
    }

    let key = match std::str::from_utf8(&args[0]) {
        Ok(s) => s,
        Err(_) => return RespValue::Error("ERR invalid key".to_string()),
    };

    let bit = match std::str::from_utf8(&args[1]) {
        Ok(s) => match s {
            "0" => 0u8,
            "1" => 1u8,
            _ => return RespValue::Error("ERR bit should be 0 or 1".to_string()),
        },
        Err(_) => return RespValue::Error("ERR bit should be 0 or 1".to_string()),
    };

    let db_instance = db.get_db(db_index).unwrap();

    let value = match db_instance.get(key) {
        Some(val) => match val.as_string() {
            Some(s) => s.to_vec(),
            None => return RespValue::Error("WRONGTYPE Operation against a key holding the wrong kind of value".to_string()),
        },
        None => return RespValue::Integer(-1), // Key doesn't exist
    };

    if value.is_empty() {
        return RespValue::Integer(-1);
    }

    // Parse start/end range (simplified - byte mode only)
    let (start, end) = if args.len() >= 4 {
        let start_val = match std::str::from_utf8(&args[2]) {
            Ok(s) => match s.parse::<i64>() {
                Ok(n) => n,
                Err(_) => return RespValue::Error("ERR value is not an integer or out of range".to_string()),
            },
            Err(_) => return RespValue::Error("ERR value is not an integer or out of range".to_string()),
        };

        let end_val = match std::str::from_utf8(&args[3]) {
            Ok(s) => match s.parse::<i64>() {
                Ok(n) => n,
                Err(_) => return RespValue::Error("ERR value is not an integer or out of range".to_string()),
            },
            Err(_) => return RespValue::Error("ERR value is not an integer or out of range".to_string()),
        };

        let len = value.len() as i64;
        let start = if start_val < 0 { (len + start_val).max(0) } else { start_val.min(len) };
        let end = if end_val < 0 { (len + end_val).max(-1) } else { end_val.min(len - 1) };

        (start as usize, end as usize)
    } else if args.len() == 3 {
        let start_val = match std::str::from_utf8(&args[2]) {
            Ok(s) => match s.parse::<i64>() {
                Ok(n) => n,
                Err(_) => return RespValue::Error("ERR value is not an integer or out of range".to_string()),
            },
            Err(_) => return RespValue::Error("ERR value is not an integer or out of range".to_string()),
        };

        let len = value.len() as i64;
        let start = if start_val < 0 { (len + start_val).max(0) } else { start_val.min(len) };

        (start as usize, value.len().saturating_sub(1))
    } else {
        (0, value.len().saturating_sub(1))
    };

    // Find first occurrence of bit
    for byte_idx in start..=end.min(value.len().saturating_sub(1)) {
        let byte = value[byte_idx];

        for bit_idx in 0..8 {
            let bit_value = (byte >> (7 - bit_idx)) & 1;
            if bit_value == bit {
                return RespValue::Integer((byte_idx * 8 + bit_idx) as i64);
            }
        }
    }

    RespValue::Integer(-1) // Not found
}

/// BITOP operation destkey key [key ...]
/// Perform bitwise operations between strings
pub async fn bitop(db: &Arc<Database>, db_index: usize, args: Vec<Vec<u8>>) -> RespValue {
    if args.len() < 3 {
        return RespValue::Error(
            "ERR wrong number of arguments for 'bitop' command".to_string(),
        );
    }

    let operation = match std::str::from_utf8(&args[0]) {
        Ok(s) => s.to_uppercase(),
        Err(_) => return RespValue::Error("ERR invalid operation".to_string()),
    };

    let destkey = match std::str::from_utf8(&args[1]) {
        Ok(s) => s,
        Err(_) => return RespValue::Error("ERR invalid destination key".to_string()),
    };

    let db_instance = db.get_db(db_index).unwrap();

    // Get all source strings
    let mut strings: Vec<Vec<u8>> = Vec::new();
    let mut max_len = 0;

    for key_bytes in &args[2..] {
        let key = match std::str::from_utf8(key_bytes) {
            Ok(s) => s,
            Err(_) => return RespValue::Error("ERR invalid key".to_string()),
        };

        match db_instance.get(key) {
            Some(val) => match val.as_string() {
                Some(s) => {
                    let vec = s.to_vec();
                    max_len = max_len.max(vec.len());
                    strings.push(vec);
                }
                None => return RespValue::Error("WRONGTYPE Operation against a key holding the wrong kind of value".to_string()),
            },
            None => {
                strings.push(Vec::new());
            }
        }
    }

    // Perform operation
    let result = match operation.as_str() {
        "AND" => {
            if strings.is_empty() {
                return RespValue::Error("ERR BITOP AND requires at least one source key".to_string());
            }
            let mut result = vec![0xFFu8; max_len];
            for s in &strings {
                for (i, &byte) in s.iter().enumerate() {
                    result[i] &= byte;
                }
                // AND with 0 for positions beyond this string's length
                for i in s.len()..max_len {
                    result[i] = 0;
                }
            }
            result
        }
        "OR" => {
            let mut result = vec![0u8; max_len];
            for s in &strings {
                for (i, &byte) in s.iter().enumerate() {
                    result[i] |= byte;
                }
            }
            result
        }
        "XOR" => {
            let mut result = vec![0u8; max_len];
            for s in &strings {
                for (i, &byte) in s.iter().enumerate() {
                    result[i] ^= byte;
                }
            }
            result
        }
        "NOT" => {
            if strings.len() != 1 {
                return RespValue::Error("ERR BITOP NOT requires exactly one source key".to_string());
            }
            strings[0].iter().map(|&b| !b).collect()
        }
        _ => return RespValue::Error("ERR unknown BITOP operation".to_string()),
    };

    let result_len = result.len();

    // Store result
    if result_len > 0 {
        db_instance.set(destkey.to_string(), RedisValue::String(Bytes::from(result)));
    } else {
        db_instance.delete(destkey);
    }

    RespValue::Integer(result_len as i64)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::db::Database;

    #[tokio::test]
    async fn test_setbit_getbit() {
        let db = Arc::new(Database::new(16));

        // SETBIT key 7 1
        let result = setbit(&db, 0, vec![b"key".to_vec(), b"7".to_vec(), b"1".to_vec()]).await;
        assert_eq!(result, RespValue::Integer(0)); // Old value was 0

        // GETBIT key 7
        let result = getbit(&db, 0, vec![b"key".to_vec(), b"7".to_vec()]).await;
        assert_eq!(result, RespValue::Integer(1));

        // GETBIT key 100
        let result = getbit(&db, 0, vec![b"key".to_vec(), b"100".to_vec()]).await;
        assert_eq!(result, RespValue::Integer(0));
    }

    #[tokio::test]
    async fn test_bitcount() {
        let db = Arc::new(Database::new(16));

        // Set some bits
        setbit(&db, 0, vec![b"key".to_vec(), b"0".to_vec(), b"1".to_vec()]).await;
        setbit(&db, 0, vec![b"key".to_vec(), b"1".to_vec(), b"1".to_vec()]).await;
        setbit(&db, 0, vec![b"key".to_vec(), b"2".to_vec(), b"1".to_vec()]).await;

        // BITCOUNT key
        let result = bitcount(&db, 0, vec![b"key".to_vec()]).await;
        assert_eq!(result, RespValue::Integer(3));
    }
}
