# Redis-Rust Architecture Design

## System Architecture

### High-Level Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                      Client Connections                      │
│              (Redis Protocol - RESP2/RESP3)                 │
└────────────────────────┬────────────────────────────────────┘
                         │
┌────────────────────────▼────────────────────────────────────┐
│                   Network Layer (Tokio)                      │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐     │
│  │ TCP Listener │  │  Connection  │  │   Protocol   │     │
│  │              │  │   Manager    │  │    Parser    │     │
│  └──────────────┘  └──────────────┘  └──────────────┘     │
└────────────────────────┬────────────────────────────────────┘
                         │
┌────────────────────────▼────────────────────────────────────┐
│                   Command Processor                          │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐     │
│  │   Command    │  │  Transaction │  │   Pipeline   │     │
│  │  Dispatcher  │  │    Engine    │  │    Handler   │     │
│  └──────────────┘  └──────────────┘  └──────────────┘     │
└────────────────────────┬────────────────────────────────────┘
                         │
┌────────────────────────▼────────────────────────────────────┐
│                    Storage Engine                            │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐     │
│  │   Database   │  │    Memory    │  │  Expiration  │     │
│  │   (16 DBs)   │  │   Manager    │  │    Engine    │     │
│  └──────────────┘  └──────────────┘  └──────────────┘     │
│                                                              │
│  ┌─────────────────────────────────────────────────────┐   │
│  │              Data Structures                         │   │
│  │  String │ Hash │ List │ Set │ ZSet │ Stream │ ...  │   │
│  └─────────────────────────────────────────────────────┘   │
└────────────────────────┬────────────────────────────────────┘
                         │
        ┌────────────────┼────────────────┐
        │                │                │
┌───────▼──────┐  ┌──────▼─────┐  ┌──────▼──────┐
│ Persistence  │  │ Replication│  │   Cluster   │
│              │  │            │  │             │
│ ┌──────────┐ │  │ ┌────────┐ │  │ ┌─────────┐ │
│ │   RDB    │ │  │ │ Master │ │  │ │  Slots  │ │
│ └──────────┘ │  │ └────────┘ │  │ └─────────┘ │
│ ┌──────────┐ │  │ ┌────────┐ │  │ ┌─────────┐ │
│ │   AOF    │ │  │ │ Replica│ │  │ │  Gossip │ │
│ └──────────┘ │  │ └────────┘ │  │ └─────────┘ │
└──────────────┘  └────────────┘  └─────────────┘
```

## Core Components

### 1. Network Layer

#### TCP Server (Tokio-based)
```rust
// Async TCP listener handling multiple connections
pub struct RedisServer {
    listener: TcpListener,
    db: Arc<Database>,
    config: Arc<ServerConfig>,
}

// Connection handler per client
pub struct Connection {
    stream: BufStream<TcpStream>,
    db: Arc<Database>,
    state: ConnectionState,
}
```

**Key Features:**
- Async I/O with Tokio runtime
- Connection pooling and management
- Graceful shutdown
- Keep-alive support
- Rate limiting per connection

#### RESP Protocol Parser
```rust
pub enum RespValue {
    SimpleString(String),
    Error(String),
    Integer(i64),
    BulkString(Option<Bytes>),
    Array(Option<Vec<RespValue>>),
    // RESP3 extensions
    Null,
    Boolean(bool),
    Double(f64),
    Map(HashMap<RespValue, RespValue>),
}

pub struct RespParser {
    buffer: BytesMut,
}
```

**Features:**
- Support RESP2 and RESP3 protocols
- Streaming parser for large payloads
- Zero-copy where possible
- Error recovery

### 2. Storage Engine

#### Database Structure
```rust
pub struct Database {
    // 16 logical databases (0-15)
    dbs: [DbInstance; 16],
    config: Arc<DbConfig>,
}

pub struct DbInstance {
    // Main key-value storage
    data: Arc<DashMap<String, RedisObject>>,
    // Expiration tracking
    expires: Arc<DashMap<String, Instant>>,
    // Blocking operations
    blocking_keys: Arc<DashMap<String, Vec<BlockedClient>>>,
}

pub struct RedisObject {
    value: RedisValue,
    lru: u32,        // LRU clock
    refcount: u32,   // Reference count
}

pub enum RedisValue {
    String(Bytes),
    List(LinkedList<Bytes>),
    Set(HashSet<Bytes>),
    ZSet(SkipList<Bytes, f64>),
    Hash(HashMap<Bytes, Bytes>),
    Stream(Stream),
}
```

**Key Design Decisions:**
- Use `DashMap` for lock-free concurrent access
- Separate expiration tracking for efficiency
- Copy-on-write for strings
- Skip list for sorted sets (O(log n) operations)

#### Memory Management
```rust
pub struct MemoryManager {
    max_memory: usize,
    used_memory: AtomicUsize,
    eviction_policy: EvictionPolicy,
}

pub enum EvictionPolicy {
    NoEviction,
    AllKeysLRU,
    AllKeysLFU,
    VolatileLRU,
    VolatileLFU,
    AllKeysRandom,
    VolatileRandom,
    VolatileTTL,
}
```

**Features:**
- Configurable eviction policies
- Background eviction tasks
- Memory usage tracking per object
- OOM prevention

### 3. Command Processing

#### Command Registry
```rust
pub struct CommandRegistry {
    commands: HashMap<&'static str, CommandHandler>,
}

pub struct CommandHandler {
    name: &'static str,
    arity: i32,              // Number of arguments (-n means >= n)
    flags: CommandFlags,
    first_key: usize,        // Position of first key
    last_key: usize,         // Position of last key
    step: usize,             // Key position step
    handler: CommandFn,
}

bitflags! {
    pub struct CommandFlags: u32 {
        const WRITE = 1 << 0;
        const READONLY = 1 << 1;
        const ADMIN = 1 << 2;
        const PUBSUB = 1 << 3;
        const BLOCKING = 1 << 4;
        const FAST = 1 << 5;
    }
}

type CommandFn = fn(&mut Context, Vec<Bytes>) -> Result<RespValue>;
```

#### Transaction Support
```rust
pub struct Transaction {
    commands: Vec<Command>,
    watched_keys: HashMap<String, u64>,  // Key -> version
    state: TransactionState,
}

pub enum TransactionState {
    Idle,
    Queued,
    Executing,
    Aborted,
}
```

**WATCH Implementation:**
- Track version numbers for watched keys
- Invalidate transaction on modification
- Optimistic locking pattern

### 4. Data Structure Implementations

#### String
- Simple byte array with encoding optimization
- Encodings: raw, int, embstr (embedded string for small strings)

#### List
```rust
pub enum List {
    // Quick list: list of compressed nodes
    QuickList {
        nodes: LinkedList<QuickListNode>,
        len: usize,
    },
}

pub struct QuickListNode {
    data: Vec<Bytes>,  // Or compressed ziplist
    compressed: bool,
}
```

#### Hash
```rust
pub enum Hash {
    // Small hashes use ziplist for memory efficiency
    ZipList(ZipList),
    // Large hashes use hash table
    HashMap(HashMap<Bytes, Bytes>),
}
```

**Optimization:** Automatic promotion from ziplist to hashmap based on size

#### Sorted Set (ZSet)
```rust
pub struct ZSet {
    // Skip list for range queries
    skip_list: SkipList<Bytes, f64>,
    // Hash table for O(1) score lookup
    dict: HashMap<Bytes, f64>,
}

pub struct SkipList<K, V> {
    head: Link<K, V>,
    tail: Link<K, V>,
    level: usize,
    length: usize,
}
```

**Key Features:**
- Skip list with configurable max level (32)
- Dual indexing for fast score and rank queries
- Support for lexicographical ordering

#### Stream
```rust
pub struct Stream {
    entries: BTreeMap<StreamId, StreamEntry>,
    groups: HashMap<String, ConsumerGroup>,
    max_deleted_id: StreamId,
    length: usize,
}

pub struct StreamId {
    ms: u64,      // Milliseconds
    seq: u64,     // Sequence number
}

pub struct ConsumerGroup {
    name: String,
    last_id: StreamId,
    pending: HashMap<StreamId, PendingEntry>,
    consumers: HashMap<String, Consumer>,
}
```

### 5. Persistence

#### RDB Format
```rust
pub struct RdbWriter {
    writer: BufWriter<File>,
    checksum: u64,
}

pub struct RdbReader {
    reader: BufReader<File>,
    checksum: u64,
}

// RDB file format:
// REDIS<version><databases><EOF><checksum>
```

**Features:**
- Binary format with checksums
- Compression (LZF algorithm)
- Background save with fork (Unix) or copy-on-write
- Incremental snapshots

#### AOF (Append-Only File)
```rust
pub struct AofWriter {
    file: Arc<Mutex<BufWriter<File>>>,
    buffer: Vec<u8>,
    fsync_policy: FsyncPolicy,
}

pub enum FsyncPolicy {
    Always,      // fsync after every write
    EverySecond, // fsync every second
    No,          // Let OS decide
}
```

**Features:**
- Command logging in RESP format
- Background rewriting to compact file
- Automatic AOF rewrite triggers
- Hybrid RDB+AOF format

### 6. Replication

#### Master-Replica Architecture
```rust
pub struct ReplicationMaster {
    replicas: Vec<ReplicaInfo>,
    repl_backlog: Arc<RwLock<ReplBacklog>>,
    repl_offset: AtomicU64,
}

pub struct ReplicaInfo {
    id: String,
    addr: SocketAddr,
    state: ReplicaState,
    offset: u64,
}

pub struct ReplicationReplica {
    master_addr: SocketAddr,
    state: ReplicaState,
    offset: u64,
    cached_master: Option<Connection>,
}

pub enum ReplicaState {
    Connecting,
    Connected,
    Syncing,
    Online,
}
```

**Replication Protocol:**
1. PSYNC command with replication ID and offset
2. Full sync (RDB) or partial sync (backlog)
3. Streaming command replication
4. Periodic heartbeats

#### Replication Backlog
```rust
pub struct ReplBacklog {
    buffer: VecDeque<u8>,
    size: usize,
    offset: u64,
    repl_id: String,
}
```

### 7. Clustering

#### Cluster Architecture
```rust
pub struct ClusterNode {
    id: String,
    addr: SocketAddr,
    flags: NodeFlags,
    master: Option<String>,  // Master node ID if replica
    slots: BitVec,           // 16384 bits for slot ownership
    epoch: u64,
}

pub struct ClusterState {
    myself: Arc<ClusterNode>,
    nodes: HashMap<String, Arc<ClusterNode>>,
    slots_to_nodes: [Option<String>; 16384],
    state: ClusterHealthState,
}

pub enum ClusterHealthState {
    Ok,
    Fail,
    Partial,  // Some slots uncovered
}
```

**Hash Slot Calculation:**
```rust
fn hash_slot(key: &[u8]) -> u16 {
    // Extract hash tag if present {tag}
    let hash_key = extract_hash_tag(key).unwrap_or(key);
    crc16(hash_key) % 16384
}
```

**Cluster Bus Protocol:**
- Gossip protocol for node discovery
- Failure detection with PFAIL/FAIL states
- Automatic failover with voting
- Slot migration support

### 8. Pub/Sub System

```rust
pub struct PubSubManager {
    // Channel -> Subscribers
    channels: Arc<DashMap<String, HashSet<ClientId>>>,
    // Pattern -> Subscribers
    patterns: Arc<DashMap<String, HashSet<ClientId>>>,
}

impl PubSubManager {
    pub async fn publish(&self, channel: &str, message: &[u8]) -> usize;
    pub async fn subscribe(&self, client: ClientId, channel: String);
    pub async fn psubscribe(&self, client: ClientId, pattern: String);
}
```

### 9. Scripting Engine (Lua)

```rust
pub struct ScriptEngine {
    lua: Arc<Mutex<Lua>>,
    scripts: Arc<DashMap<String, String>>,  // SHA -> script
}

impl ScriptEngine {
    pub async fn eval(&self, script: &str, keys: Vec<String>,
                     args: Vec<Bytes>) -> Result<RespValue>;

    pub async fn evalsha(&self, sha: &str, keys: Vec<String>,
                        args: Vec<Bytes>) -> Result<RespValue>;
}
```

**Sandboxing:**
- Disable dangerous Lua functions
- Execution timeout
- Deterministic random number generator
- Redis command calls via `redis.call()` and `redis.pcall()`

## Concurrency Model

### Thread Pool Architecture
```
┌─────────────────────────────────────┐
│      Tokio Runtime (Multi-thread)   │
│                                     │
│  ┌───────┐ ┌───────┐ ┌───────┐    │
│  │ Worker│ │ Worker│ │ Worker│ ...│
│  │Thread │ │Thread │ │Thread │    │
│  └───────┘ └───────┘ └───────┘    │
└─────────────────────────────────────┘
         │         │         │
         └────┬────┴────┬────┘
              │         │
    ┌─────────▼──┐  ┌──▼──────────┐
    │ Network I/O│  │  Background  │
    │  Handlers  │  │    Tasks     │
    └────────────┘  └──────────────┘
                          │
              ┌───────────┼────────────┐
              │           │            │
        ┌─────▼──┐  ┌────▼───┐  ┌────▼────┐
        │ BGSAVE │  │  AOF   │  │ Eviction│
        │        │  │ Rewrite│  │         │
        └────────┘  └────────┘  └─────────┘
```

**Key Principles:**
- Single-writer principle for each key
- Lock-free reads where possible
- Background tasks for expensive operations
- Async I/O for all network operations

## Configuration

```rust
pub struct ServerConfig {
    // Network
    pub bind: String,
    pub port: u16,
    pub tcp_backlog: u32,
    pub timeout: Duration,

    // Memory
    pub max_memory: usize,
    pub eviction_policy: EvictionPolicy,

    // Persistence
    pub save_intervals: Vec<(Duration, u64)>,  // (time, changes)
    pub aof_enabled: bool,
    pub aof_fsync: FsyncPolicy,

    // Replication
    pub replicaof: Option<(String, u16)>,
    pub replica_read_only: bool,

    // Cluster
    pub cluster_enabled: bool,
    pub cluster_node_timeout: Duration,

    // Limits
    pub max_clients: usize,
    pub max_db_count: usize,
}
```

## Performance Considerations

1. **Zero-copy operations**: Use `Bytes` crate for buffer sharing
2. **Lock-free structures**: DashMap for concurrent access
3. **Memory pooling**: Reuse allocations for frequent objects
4. **Pipeline optimization**: Batch command execution
5. **Lazy deletion**: Background cleanup of large objects
6. **Async everywhere**: Non-blocking I/O operations
7. **SIMD optimizations**: For checksum and hashing operations

## Error Handling Strategy

```rust
pub enum RedisError {
    // Protocol errors
    ParseError(String),
    InvalidCommand(String),
    WrongType,
    WrongArity,

    // Runtime errors
    OutOfMemory,
    IoError(io::Error),
    ClusterDown,
    CrossSlot,

    // User errors
    NoSuchKey,
    Timeout,
    ReadOnlyReplica,
}

pub type Result<T> = std::result::Result<T, RedisError>;
```

All errors are converted to RESP error messages for client responses.
