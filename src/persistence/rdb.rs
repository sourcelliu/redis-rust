// RDB (Redis Database) persistence implementation
// Binary format for snapshots

use crate::storage::db::{Database, DbInstance};
use crate::storage::types::RedisValue;
use anyhow::{Context, Result};
use bytes::Bytes;
use std::collections::{HashMap, HashSet, LinkedList};
use std::fs::File;
use std::io::{BufReader, BufWriter, Read, Write};
use std::path::Path;
use std::sync::Arc;

// RDB file format constants
const RDB_VERSION: u32 = 1;
const RDB_MAGIC: &[u8] = b"REDIS";

// Opcodes for different value types
const OPCODE_STRING: u8 = 0;
const OPCODE_LIST: u8 = 1;
const OPCODE_SET: u8 = 2;
const OPCODE_HASH: u8 = 3;
const OPCODE_ZSET: u8 = 4;
const OPCODE_EXPIRY: u8 = 253;
const OPCODE_DB_SELECT: u8 = 254;
const OPCODE_EOF: u8 = 255;

pub struct RdbSerializer;

impl RdbSerializer {
    /// Save database to RDB file
    pub async fn save(db: &Arc<Database>, path: impl AsRef<Path>) -> Result<()> {
        let file = File::create(path).context("Failed to create RDB file")?;
        let mut writer = BufWriter::new(file);

        // Write magic string and version
        writer.write_all(RDB_MAGIC)?;
        writer.write_all(&RDB_VERSION.to_le_bytes())?;

        // Save each database
        for db_index in 0..16 {
            let db_instance = match db.get_db(db_index) {
                Some(d) => d,
                None => continue,
            };

            // Skip empty databases
            if db_instance.is_empty() {
                continue;
            }

            // Write database selector
            writer.write_all(&[OPCODE_DB_SELECT])?;
            writer.write_all(&(db_index as u32).to_le_bytes())?;

            // Get all keys
            let keys = db_instance.keys("*");

            // Write each key-value pair
            for key in keys {
                Self::save_key_value(&mut writer, db_instance, &key)?;
            }
        }

        // Write EOF marker
        writer.write_all(&[OPCODE_EOF])?;
        writer.flush()?;

        Ok(())
    }

    fn save_key_value(
        writer: &mut BufWriter<File>,
        db_instance: &DbInstance,
        key: &str,
    ) -> Result<()> {
        // Check if key has expiration
        let ttl_ms = db_instance.get_ttl_ms(key);
        if ttl_ms == -2 {
            // Key doesn't exist or already expired
            return Ok(());
        }

        // Write expiration if present
        if ttl_ms > 0 {
            writer.write_all(&[OPCODE_EXPIRY])?;
            let expire_at_ms = crate::storage::db::current_timestamp_ms() + ttl_ms as u64;
            writer.write_all(&expire_at_ms.to_le_bytes())?;
        }

        // Get value
        let value = match db_instance.get(key) {
            Some(v) => v,
            None => return Ok(()), // Already expired
        };

        // Write key
        Self::write_string(writer, key.as_bytes())?;

        // Write value based on type
        match value {
            RedisValue::String(bytes) => {
                writer.write_all(&[OPCODE_STRING])?;
                Self::write_bytes(writer, &bytes)?;
            }
            RedisValue::List(list) => {
                writer.write_all(&[OPCODE_LIST])?;
                Self::write_list(writer, &list)?;
            }
            RedisValue::Set(set) => {
                writer.write_all(&[OPCODE_SET])?;
                Self::write_set(writer, &set)?;
            }
            RedisValue::Hash(hash) => {
                writer.write_all(&[OPCODE_HASH])?;
                Self::write_hash(writer, &hash)?;
            }
            RedisValue::ZSet(zset) => {
                writer.write_all(&[OPCODE_ZSET])?;
                Self::write_zset(writer, &zset)?;
            }
            RedisValue::Stream(_stream) => {
                // Skip stream serialization for RDB (not yet fully supported)
                // In a full implementation, we would add OPCODE_STREAM and write_stream
                return Ok(());
            }
        }

        Ok(())
    }

    fn write_string(writer: &mut BufWriter<File>, s: &[u8]) -> Result<()> {
        let len = s.len() as u32;
        writer.write_all(&len.to_le_bytes())?;
        writer.write_all(s)?;
        Ok(())
    }

    fn write_bytes(writer: &mut BufWriter<File>, bytes: &Bytes) -> Result<()> {
        let len = bytes.len() as u32;
        writer.write_all(&len.to_le_bytes())?;
        writer.write_all(bytes)?;
        Ok(())
    }

    fn write_list(writer: &mut BufWriter<File>, list: &LinkedList<Bytes>) -> Result<()> {
        let len = list.len() as u32;
        writer.write_all(&len.to_le_bytes())?;
        for item in list {
            Self::write_bytes(writer, item)?;
        }
        Ok(())
    }

    fn write_set(writer: &mut BufWriter<File>, set: &HashSet<Bytes>) -> Result<()> {
        let len = set.len() as u32;
        writer.write_all(&len.to_le_bytes())?;
        for item in set {
            Self::write_bytes(writer, item)?;
        }
        Ok(())
    }

    fn write_hash(writer: &mut BufWriter<File>, hash: &HashMap<Bytes, Bytes>) -> Result<()> {
        let len = hash.len() as u32;
        writer.write_all(&len.to_le_bytes())?;
        for (key, value) in hash {
            Self::write_bytes(writer, key)?;
            Self::write_bytes(writer, value)?;
        }
        Ok(())
    }

    fn write_zset(
        writer: &mut BufWriter<File>,
        zset: &crate::storage::types::ZSet,
    ) -> Result<()> {
        let len = zset.len() as u32;
        writer.write_all(&len.to_le_bytes())?;
        for (member, score) in &zset.members {
            Self::write_bytes(writer, member)?;
            writer.write_all(&score.to_le_bytes())?;
        }
        Ok(())
    }
}

pub struct RdbDeserializer;

impl RdbDeserializer {
    /// Load database from RDB file
    pub async fn load(db: &Arc<Database>, path: impl AsRef<Path>) -> Result<()> {
        let file = File::open(path).context("Failed to open RDB file")?;
        let mut reader = BufReader::new(file);

        // Read and verify magic string
        let mut magic = [0u8; 5];
        reader.read_exact(&mut magic)?;
        if &magic != RDB_MAGIC {
            anyhow::bail!("Invalid RDB file: bad magic string");
        }

        // Read version
        let mut version_bytes = [0u8; 4];
        reader.read_exact(&mut version_bytes)?;
        let version = u32::from_le_bytes(version_bytes);
        if version != RDB_VERSION {
            anyhow::bail!("Unsupported RDB version: {}", version);
        }

        let mut current_db: usize = 0;
        let mut expiry_ms: Option<u64> = None;

        loop {
            // Read opcode
            let mut opcode = [0u8; 1];
            if reader.read_exact(&mut opcode).is_err() {
                break;
            }

            match opcode[0] {
                OPCODE_EOF => break,
                OPCODE_DB_SELECT => {
                    let mut db_index_bytes = [0u8; 4];
                    reader.read_exact(&mut db_index_bytes)?;
                    current_db = u32::from_le_bytes(db_index_bytes) as usize;
                }
                OPCODE_EXPIRY => {
                    let mut expire_bytes = [0u8; 8];
                    reader.read_exact(&mut expire_bytes)?;
                    expiry_ms = Some(u64::from_le_bytes(expire_bytes));
                }
                _ => {
                    // Read key
                    let key = Self::read_string(&mut reader)?;

                    // Read value based on preceding opcode (which we stored)
                    let value = match opcode[0] {
                        OPCODE_STRING => {
                            let bytes = Self::read_bytes(&mut reader)?;
                            RedisValue::String(bytes)
                        }
                        OPCODE_LIST => {
                            let list = Self::read_list(&mut reader)?;
                            RedisValue::List(list)
                        }
                        OPCODE_SET => {
                            let set = Self::read_set(&mut reader)?;
                            RedisValue::Set(set)
                        }
                        OPCODE_HASH => {
                            let hash = Self::read_hash(&mut reader)?;
                            RedisValue::Hash(hash)
                        }
                        OPCODE_ZSET => {
                            let zset = Self::read_zset(&mut reader)?;
                            RedisValue::ZSet(zset)
                        }
                        _ => anyhow::bail!("Unknown value type opcode: {}", opcode[0]),
                    };

                    // Store in database
                    let db_instance = db
                        .get_db(current_db)
                        .context("Invalid database index in RDB")?;

                    if let Some(expire_at_ms) = expiry_ms.take() {
                        db_instance.set_with_expiry(key, value, expire_at_ms);
                    } else {
                        db_instance.set(key, value);
                    }
                }
            }
        }

        Ok(())
    }

    fn read_string(reader: &mut BufReader<File>) -> Result<String> {
        let mut len_bytes = [0u8; 4];
        reader.read_exact(&mut len_bytes)?;
        let len = u32::from_le_bytes(len_bytes) as usize;

        let mut buf = vec![0u8; len];
        reader.read_exact(&mut buf)?;

        String::from_utf8(buf).context("Invalid UTF-8 in key")
    }

    fn read_bytes(reader: &mut BufReader<File>) -> Result<Bytes> {
        let mut len_bytes = [0u8; 4];
        reader.read_exact(&mut len_bytes)?;
        let len = u32::from_le_bytes(len_bytes) as usize;

        let mut buf = vec![0u8; len];
        reader.read_exact(&mut buf)?;
        Ok(Bytes::from(buf))
    }

    fn read_list(reader: &mut BufReader<File>) -> Result<LinkedList<Bytes>> {
        let mut len_bytes = [0u8; 4];
        reader.read_exact(&mut len_bytes)?;
        let len = u32::from_le_bytes(len_bytes) as usize;

        let mut list = LinkedList::new();
        for _ in 0..len {
            list.push_back(Self::read_bytes(reader)?);
        }
        Ok(list)
    }

    fn read_set(reader: &mut BufReader<File>) -> Result<HashSet<Bytes>> {
        let mut len_bytes = [0u8; 4];
        reader.read_exact(&mut len_bytes)?;
        let len = u32::from_le_bytes(len_bytes) as usize;

        let mut set = HashSet::new();
        for _ in 0..len {
            set.insert(Self::read_bytes(reader)?);
        }
        Ok(set)
    }

    fn read_hash(reader: &mut BufReader<File>) -> Result<HashMap<Bytes, Bytes>> {
        let mut len_bytes = [0u8; 4];
        reader.read_exact(&mut len_bytes)?;
        let len = u32::from_le_bytes(len_bytes) as usize;

        let mut hash = HashMap::new();
        for _ in 0..len {
            let key = Self::read_bytes(reader)?;
            let value = Self::read_bytes(reader)?;
            hash.insert(key, value);
        }
        Ok(hash)
    }

    fn read_zset(reader: &mut BufReader<File>) -> Result<crate::storage::types::ZSet> {
        let mut len_bytes = [0u8; 4];
        reader.read_exact(&mut len_bytes)?;
        let len = u32::from_le_bytes(len_bytes) as usize;

        let mut zset = crate::storage::types::ZSet::new();
        for _ in 0..len {
            let member = Self::read_bytes(reader)?;
            let mut score_bytes = [0u8; 8];
            reader.read_exact(&mut score_bytes)?;
            let score = f64::from_le_bytes(score_bytes);

            zset.members.insert(member.clone(), score);
            zset.scores.insert(
                (ordered_float::OrderedFloat(score), member),
                (),
            );
        }
        Ok(zset)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    #[tokio::test]
    async fn test_rdb_save_load() {
        let db = Arc::new(Database::new(16));
        let db_instance = db.get_db(0).unwrap();

        // Add some test data
        db_instance.set(
            "string_key".to_string(),
            RedisValue::String(Bytes::from("test_value")),
        );

        let mut list = LinkedList::new();
        list.push_back(Bytes::from("item1"));
        list.push_back(Bytes::from("item2"));
        db_instance.set("list_key".to_string(), RedisValue::List(list));

        // Save to temporary file
        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path();

        RdbSerializer::save(&db, path).await.unwrap();

        // Load into new database
        let db2 = Arc::new(Database::new(16));
        RdbDeserializer::load(&db2, path).await.unwrap();

        // Verify data
        let db2_instance = db2.get_db(0).unwrap();
        assert!(db2_instance.exists("string_key"));
        assert!(db2_instance.exists("list_key"));

        let value = db2_instance.get("string_key").unwrap();
        match value {
            RedisValue::String(s) => assert_eq!(s, Bytes::from("test_value")),
            _ => panic!("Wrong type"),
        }
    }
}
