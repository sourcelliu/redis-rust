// Redis value types

use bytes::Bytes;
use std::collections::{BTreeMap, HashMap, HashSet, LinkedList};

/// Sorted Set member with score
#[derive(Debug, Clone, PartialEq)]
pub struct ZSetMember {
    pub member: Bytes,
    pub score: f64,
}

impl Eq for ZSetMember {}

impl PartialOrd for ZSetMember {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for ZSetMember {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        // First compare by score, then by member lexicographically
        match self.score.partial_cmp(&other.score) {
            Some(std::cmp::Ordering::Equal) => self.member.cmp(&other.member),
            Some(ord) => ord,
            None => std::cmp::Ordering::Equal, // Handle NaN case
        }
    }
}

/// Sorted Set implementation using BTreeMap for range queries
/// and HashMap for O(1) member->score lookups
#[derive(Debug, Clone)]
pub struct ZSet {
    // BTreeMap: (score, member) -> () for ordered access
    pub scores: BTreeMap<(ordered_float::OrderedFloat<f64>, Bytes), ()>,
    // HashMap: member -> score for quick score lookups
    pub members: HashMap<Bytes, f64>,
}

impl ZSet {
    pub fn new() -> Self {
        Self {
            scores: BTreeMap::new(),
            members: HashMap::new(),
        }
    }

    pub fn len(&self) -> usize {
        self.members.len()
    }

    pub fn is_empty(&self) -> bool {
        self.members.is_empty()
    }
}

/// Stream ID: timestamp-sequence (e.g., "1526919030474-0")
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct StreamId {
    pub timestamp: u64,
    pub sequence: u64,
}

impl StreamId {
    pub fn new(timestamp: u64, sequence: u64) -> Self {
        Self { timestamp, sequence }
    }

    pub fn from_string(s: &str) -> Option<Self> {
        let parts: Vec<&str> = s.split('-').collect();
        if parts.len() != 2 {
            return None;
        }
        let timestamp = parts[0].parse().ok()?;
        let sequence = parts[1].parse().ok()?;
        Some(Self::new(timestamp, sequence))
    }

    pub fn to_string(&self) -> String {
        format!("{}-{}", self.timestamp, self.sequence)
    }
}

/// Stream entry: ID + field-value pairs
#[derive(Debug, Clone)]
pub struct StreamEntry {
    pub id: StreamId,
    pub fields: HashMap<Bytes, Bytes>,
}

/// Stream data structure
#[derive(Debug, Clone)]
pub struct Stream {
    pub entries: BTreeMap<StreamId, StreamEntry>,
    pub last_id: StreamId,
}

impl Stream {
    pub fn new() -> Self {
        Self {
            entries: BTreeMap::new(),
            last_id: StreamId::new(0, 0),
        }
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

#[derive(Debug, Clone)]
pub enum RedisValue {
    String(Bytes),
    List(LinkedList<Bytes>),
    Set(HashSet<Bytes>),
    Hash(HashMap<Bytes, Bytes>),
    ZSet(ZSet),
    Stream(Stream),
}

impl RedisValue {
    pub fn type_name(&self) -> &str {
        match self {
            RedisValue::String(_) => "string",
            RedisValue::List(_) => "list",
            RedisValue::Set(_) => "set",
            RedisValue::Hash(_) => "hash",
            RedisValue::ZSet(_) => "zset",
            RedisValue::Stream(_) => "stream",
        }
    }

    pub fn as_string(&self) -> Option<&Bytes> {
        match self {
            RedisValue::String(s) => Some(s),
            _ => None,
        }
    }

    pub fn as_string_mut(&mut self) -> Option<&mut Bytes> {
        match self {
            RedisValue::String(s) => Some(s),
            _ => None,
        }
    }

    pub fn into_string(self) -> Option<Bytes> {
        match self {
            RedisValue::String(s) => Some(s),
            _ => None,
        }
    }
}

// Implement Hash and Eq for HashSet and HashMap
impl PartialEq for RedisValue {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (RedisValue::String(a), RedisValue::String(b)) => a == b,
            (RedisValue::List(a), RedisValue::List(b)) => {
                a.iter().eq(b.iter())
            }
            (RedisValue::Set(a), RedisValue::Set(b)) => a == b,
            (RedisValue::Hash(a), RedisValue::Hash(b)) => a == b,
            (RedisValue::ZSet(a), RedisValue::ZSet(b)) => a.members == b.members,
            _ => false,
        }
    }
}

impl Eq for RedisValue {}
