// HyperLogLog commands for Redis-Rust
// HyperLogLog is a probabilistic data structure for cardinality estimation

use crate::protocol::RespValue;
use crate::storage::db::Database;
use crate::storage::types::RedisValue;
use bytes::Bytes;
use std::sync::Arc;

// HyperLogLog constants
const HLL_REGISTERS: usize = 16384; // 2^14 registers
const HLL_BITS: usize = 6; // 6 bits per register
const HLL_SIZE: usize = (HLL_REGISTERS * HLL_BITS + 7) / 8; // 12KB

/// HyperLogLog implementation using 16384 registers with 6 bits each
#[derive(Clone)]
struct HyperLogLog {
    registers: Vec<u8>,
}

impl HyperLogLog {
    fn new() -> Self {
        HyperLogLog {
            registers: vec![0; HLL_SIZE],
        }
    }

    fn from_bytes(data: &[u8]) -> Option<Self> {
        if data.len() != HLL_SIZE {
            return None;
        }
        Some(HyperLogLog {
            registers: data.to_vec(),
        })
    }

    fn to_bytes(&self) -> Vec<u8> {
        self.registers.clone()
    }

    /// Add an element to the HyperLogLog
    fn add(&mut self, element: &[u8]) -> bool {
        let hash = Self::hash(element);

        // Use first 14 bits for register index
        let index = (hash & 0x3FFF) as usize;

        // Count leading zeros in remaining bits + 1
        let remaining = hash >> 14;
        let leading_zeros = if remaining == 0 {
            50 // 64 - 14 bits
        } else {
            remaining.leading_zeros() as u8 - 14
        } + 1;

        let old_value = self.get_register(index);
        if leading_zeros > old_value {
            self.set_register(index, leading_zeros);
            return true;
        }
        false
    }

    /// Get register value (6 bits)
    fn get_register(&self, index: usize) -> u8 {
        let byte_index = (index * HLL_BITS) / 8;
        let bit_offset = (index * HLL_BITS) % 8;

        if bit_offset + HLL_BITS <= 8 {
            // Register fits in one byte
            (self.registers[byte_index] >> (8 - bit_offset - HLL_BITS)) & 0x3F
        } else {
            // Register spans two bytes
            let first_bits = 8 - bit_offset;
            let second_bits = HLL_BITS - first_bits;
            let high = (self.registers[byte_index] & ((1 << first_bits) - 1)) << second_bits;
            let low = self.registers[byte_index + 1] >> (8 - second_bits);
            high | low
        }
    }

    /// Set register value (6 bits)
    fn set_register(&mut self, index: usize, value: u8) {
        let byte_index = (index * HLL_BITS) / 8;
        let bit_offset = (index * HLL_BITS) % 8;

        if bit_offset + HLL_BITS <= 8 {
            // Register fits in one byte
            let mask = 0x3F << (8 - bit_offset - HLL_BITS);
            self.registers[byte_index] = (self.registers[byte_index] & !mask)
                | ((value << (8 - bit_offset - HLL_BITS)) & mask);
        } else {
            // Register spans two bytes
            let first_bits = 8 - bit_offset;
            let second_bits = HLL_BITS - first_bits;

            let high_mask = (1 << first_bits) - 1;
            self.registers[byte_index] = (self.registers[byte_index] & !high_mask)
                | ((value >> second_bits) & high_mask);

            let low_mask = ((1 << second_bits) - 1) << (8 - second_bits);
            self.registers[byte_index + 1] = (self.registers[byte_index + 1] & !low_mask)
                | ((value << (8 - second_bits)) & low_mask);
        }
    }

    /// Count cardinality using HyperLogLog algorithm
    fn count(&self) -> u64 {
        let mut sum = 0.0;
        let mut zeros = 0;

        for i in 0..HLL_REGISTERS {
            let val = self.get_register(i);
            if val == 0 {
                zeros += 1;
            }
            sum += 1.0 / (1u64 << val) as f64;
        }

        // Alpha constant for 16384 registers
        let alpha = 0.7213 / (1.0 + 1.079 / HLL_REGISTERS as f64);
        let estimate = alpha * (HLL_REGISTERS as f64) * (HLL_REGISTERS as f64) / sum;

        // Apply bias correction for small and large cardinalities
        if estimate <= 2.5 * HLL_REGISTERS as f64 {
            if zeros > 0 {
                // Small range correction
                (HLL_REGISTERS as f64 * (HLL_REGISTERS as f64 / zeros as f64).ln()) as u64
            } else {
                estimate as u64
            }
        } else if estimate <= (1u64 << 32) as f64 / 30.0 {
            estimate as u64
        } else {
            // Large range correction
            let two_pow_32 = (1u64 << 32) as f64;
            (-two_pow_32 * (1.0 - estimate / two_pow_32).ln()) as u64
        }
    }

    /// Merge another HyperLogLog into this one
    fn merge(&mut self, other: &HyperLogLog) {
        for i in 0..HLL_REGISTERS {
            let self_val = self.get_register(i);
            let other_val = other.get_register(i);
            if other_val > self_val {
                self.set_register(i, other_val);
            }
        }
    }

    /// Simple hash function (FNV-1a variant)
    fn hash(data: &[u8]) -> u64 {
        let mut hash: u64 = 0xcbf29ce484222325;
        for &byte in data {
            hash ^= byte as u64;
            hash = hash.wrapping_mul(0x100000001b3);
        }
        hash
    }
}

/// PFADD key element [element ...]
/// Adds elements to HyperLogLog
pub async fn pfadd(db: &Arc<Database>, db_index: usize, args: Vec<Vec<u8>>) -> RespValue {
    if args.len() < 2 {
        return RespValue::Error("ERR wrong number of arguments for 'pfadd' command".to_string());
    }

    let key = match std::str::from_utf8(&args[0]) {
        Ok(s) => s.to_string(),
        Err(_) => return RespValue::Error("ERR invalid key".to_string()),
    };

    let db_instance = match db.get_db(db_index) {
        Some(d) => d,
        None => return RespValue::Error("ERR invalid database".to_string()),
    };

    // Get existing HLL or create new one
    let mut hll = match db_instance.get(&key) {
        Some(RedisValue::String(bytes)) => {
            match HyperLogLog::from_bytes(&bytes) {
                Some(h) => h,
                None => return RespValue::Error("WRONGTYPE Key is not a valid HyperLogLog string value".to_string()),
            }
        }
        Some(_) => {
            return RespValue::Error(
                "WRONGTYPE Operation against a key holding the wrong kind of value".to_string(),
            )
        }
        None => HyperLogLog::new(),
    };

    // Add all elements
    let mut changed = false;
    for element in &args[1..] {
        if hll.add(element) {
            changed = true;
        }
    }

    // Store back
    db_instance.set(key, RedisValue::String(Bytes::from(hll.to_bytes())));

    RespValue::Integer(if changed { 1 } else { 0 })
}

/// PFCOUNT key [key ...]
/// Returns cardinality estimate
pub async fn pfcount(db: &Arc<Database>, db_index: usize, args: Vec<Vec<u8>>) -> RespValue {
    if args.is_empty() {
        return RespValue::Error("ERR wrong number of arguments for 'pfcount' command".to_string());
    }

    let db_instance = match db.get_db(db_index) {
        Some(d) => d,
        None => return RespValue::Error("ERR invalid database".to_string()),
    };

    // If single key, just count it
    if args.len() == 1 {
        let key = match std::str::from_utf8(&args[0]) {
            Ok(s) => s,
            Err(_) => return RespValue::Error("ERR invalid key".to_string()),
        };

        match db_instance.get(key) {
            Some(RedisValue::String(bytes)) => {
                match HyperLogLog::from_bytes(&bytes) {
                    Some(hll) => return RespValue::Integer(hll.count() as i64),
                    None => return RespValue::Error("WRONGTYPE Key is not a valid HyperLogLog string value".to_string()),
                }
            }
            Some(_) => {
                return RespValue::Error(
                    "WRONGTYPE Operation against a key holding the wrong kind of value".to_string(),
                )
            }
            None => return RespValue::Integer(0),
        }
    }

    // Multiple keys: merge and count
    let mut merged = HyperLogLog::new();
    for key_bytes in &args {
        let key = match std::str::from_utf8(key_bytes) {
            Ok(s) => s,
            Err(_) => continue,
        };

        if let Some(RedisValue::String(bytes)) = db_instance.get(key) {
            if let Some(hll) = HyperLogLog::from_bytes(&bytes) {
                merged.merge(&hll);
            }
        }
    }

    RespValue::Integer(merged.count() as i64)
}

/// PFMERGE destkey sourcekey [sourcekey ...]
/// Merge multiple HyperLogLogs
pub async fn pfmerge(db: &Arc<Database>, db_index: usize, args: Vec<Vec<u8>>) -> RespValue {
    if args.len() < 2 {
        return RespValue::Error("ERR wrong number of arguments for 'pfmerge' command".to_string());
    }

    let destkey = match std::str::from_utf8(&args[0]) {
        Ok(s) => s.to_string(),
        Err(_) => return RespValue::Error("ERR invalid destination key".to_string()),
    };

    let db_instance = match db.get_db(db_index) {
        Some(d) => d,
        None => return RespValue::Error("ERR invalid database".to_string()),
    };

    // Create merged HLL
    let mut merged = HyperLogLog::new();

    for key_bytes in &args[1..] {
        let key = match std::str::from_utf8(key_bytes) {
            Ok(s) => s,
            Err(_) => continue,
        };

        match db_instance.get(key) {
            Some(RedisValue::String(bytes)) => {
                match HyperLogLog::from_bytes(&bytes) {
                    Some(hll) => merged.merge(&hll),
                    None => return RespValue::Error("WRONGTYPE Key is not a valid HyperLogLog string value".to_string()),
                }
            }
            Some(_) => {
                return RespValue::Error(
                    "WRONGTYPE Operation against a key holding the wrong kind of value".to_string(),
                )
            }
            None => {} // Empty key, skip
        }
    }

    // Store result
    db_instance.set(destkey, RedisValue::String(Bytes::from(merged.to_bytes())));

    RespValue::SimpleString("OK".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_pfadd_pfcount() {
        let db = Arc::new(Database::new(16));

        // Add elements
        let result = pfadd(&db, 0, vec![
            b"hll".to_vec(),
            b"foo".to_vec(),
            b"bar".to_vec(),
            b"baz".to_vec(),
        ]).await;
        assert_eq!(result, RespValue::Integer(1));

        // Count
        let result = pfcount(&db, 0, vec![b"hll".to_vec()]).await;
        if let RespValue::Integer(count) = result {
            assert!(count >= 2 && count <= 4); // Approximate
        } else {
            panic!("Expected integer result");
        }
    }

    #[tokio::test]
    async fn test_pfmerge() {
        let db = Arc::new(Database::new(16));

        // Create two HLLs
        pfadd(&db, 0, vec![b"hll1".to_vec(), b"a".to_vec(), b"b".to_vec()]).await;
        pfadd(&db, 0, vec![b"hll2".to_vec(), b"c".to_vec(), b"d".to_vec()]).await;

        // Merge
        let result = pfmerge(&db, 0, vec![
            b"merged".to_vec(),
            b"hll1".to_vec(),
            b"hll2".to_vec(),
        ]).await;
        assert_eq!(result, RespValue::SimpleString("OK".to_string()));

        // Count merged
        let result = pfcount(&db, 0, vec![b"merged".to_vec()]).await;
        if let RespValue::Integer(count) = result {
            assert!(count >= 3 && count <= 5); // Approximate count of unique elements
        }
    }

    #[test]
    fn test_hll_registers() {
        let mut hll = HyperLogLog::new();

        // Test setting and getting registers
        hll.set_register(0, 15);
        assert_eq!(hll.get_register(0), 15);

        hll.set_register(100, 31);
        assert_eq!(hll.get_register(100), 31);

        hll.set_register(16383, 63);
        assert_eq!(hll.get_register(16383), 63);
    }
}
