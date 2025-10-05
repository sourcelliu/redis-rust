# Redis-Rust Test Plan

## Testing Strategy Overview

This document outlines the comprehensive testing strategy for the Redis-Rust implementation, including unit tests, integration tests, end-to-end tests, and performance benchmarks.

## Testing Pyramid

```
         /\
        /  \  E2E Tests (10%)
       /────\
      /      \  Integration Tests (30%)
     /────────\
    /          \  Unit Tests (60%)
   /────────────\
```

## 1. Unit Tests

### Coverage Target: 90%+

#### 1.1 Protocol Layer Tests
**Location**: `src/protocol/tests.rs`

Test Cases:
- **RESP Parser**
  - Parse simple strings: `+OK\r\n`
  - Parse errors: `-ERR unknown command\r\n`
  - Parse integers: `:1000\r\n`
  - Parse bulk strings: `$6\r\nfoobar\r\n`
  - Parse null bulk strings: `$-1\r\n`
  - Parse arrays: `*2\r\n$3\r\nfoo\r\n$3\r\nbar\r\n`
  - Parse nested arrays
  - Parse RESP3 types (null, boolean, double, map)
  - Handle incomplete data (streaming)
  - Handle malformed input (error recovery)
  - Large payload handling (>1MB)

- **RESP Serializer**
  - Serialize all RESP types correctly
  - Handle special characters
  - Binary-safe serialization
  - Zero-copy optimization verification

#### 1.2 Data Structure Tests
**Location**: `src/storage/types/tests.rs`

Test Cases for Each Data Type:

- **String**
  - SET, GET basic operations
  - APPEND, STRLEN
  - INCR, DECR with integers
  - INCRBYFLOAT with floats
  - GETRANGE, SETRANGE
  - Encoding transitions (embstr -> raw)

- **List**
  - LPUSH, RPUSH, LPOP, RPOP
  - LINDEX, LLEN, LRANGE
  - LINSERT, LSET, LTRIM
  - Edge cases: empty list, single element
  - Large lists (>10k elements)

- **Hash**
  - HSET, HGET, HDEL, HEXISTS
  - HGETALL, HKEYS, HVALS
  - HINCRBY, HINCRBYFLOAT
  - Encoding transitions (ziplist -> hashtable)
  - Large hashes

- **Set**
  - SADD, SREM, SMEMBERS
  - SINTER, SUNION, SDIFF
  - SISMEMBER, SCARD
  - SPOP, SRANDMEMBER
  - Set operations with multiple sets

- **Sorted Set**
  - ZADD, ZREM, ZSCORE
  - ZRANGE, ZREVRANGE
  - ZRANGEBYSCORE, ZCOUNT
  - ZINCRBY, ZRANK
  - Lexicographical ordering (ZRANGEBYLEX)
  - Skip list implementation correctness

- **Stream**
  - XADD with auto ID generation
  - XREAD, XRANGE, XREVRANGE
  - XGROUP CREATE, XREADGROUP
  - Consumer groups and pending entries
  - Stream trimming (XTRIM)

#### 1.3 Memory Management Tests
**Location**: `src/storage/memory/tests.rs`

Test Cases:
- Memory usage tracking accuracy
- Eviction policies (LRU, LFU, Random)
- MaxMemory enforcement
- Reference counting correctness
- Memory fragmentation measurement
- Lazy free mechanism

#### 1.4 Expiration Tests
**Location**: `src/storage/expiration/tests.rs`

Test Cases:
- EXPIRE, EXPIREAT, TTL, PTTL
- Passive expiration (on access)
- Active expiration (periodic cleanup)
- Expiration in transactions
- Expiration persistence (RDB/AOF)
- Edge case: expiration in the past

#### 1.5 Transaction Tests
**Location**: `src/commands/transaction/tests.rs`

Test Cases:
- MULTI, EXEC basic flow
- DISCARD transaction
- WATCH key changes detection
- WATCH with multiple keys
- WATCH timeout
- Transaction with errors
- Nested transactions (should fail)

#### 1.6 Command Handler Tests
**Location**: `src/commands/*/tests.rs`

For each command:
- Correct execution with valid inputs
- Error handling for invalid inputs
- Argument validation (arity checking)
- Type checking (WRONGTYPE errors)
- Edge cases and boundary conditions

### Unit Test Implementation Example

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_string() {
        let input = b"+OK\r\n";
        let result = RespParser::parse(input).unwrap();
        assert_eq!(result, RespValue::SimpleString("OK".to_string()));
    }

    #[tokio::test]
    async fn test_set_get() {
        let db = Database::new();
        db.set("key", "value").await.unwrap();
        let result = db.get("key").await.unwrap();
        assert_eq!(result, Some("value".as_bytes()));
    }

    #[test]
    fn test_lru_eviction() {
        let mut mm = MemoryManager::new(1024, EvictionPolicy::AllKeysLRU);
        // Fill memory
        for i in 0..100 {
            mm.add_object(format!("key{}", i), vec![0u8; 20]);
        }
        // Trigger eviction
        mm.add_object("new_key", vec![0u8; 500]);
        // Verify LRU keys were evicted
        assert!(mm.get("key0").is_none());
    }
}
```

## 2. Integration Tests

### Coverage Target: 80%+

**Location**: `tests/integration/`

#### 2.1 Client-Server Integration
**File**: `tests/integration/client_server.rs`

Test Cases:
- Multiple concurrent clients
- Client timeout handling
- Connection pooling
- Pipeline commands execution
- Blocking commands (BLPOP, BRPOP)
- Client authentication (if implemented)

#### 2.2 Persistence Integration
**File**: `tests/integration/persistence.rs`

Test Cases:
- **RDB Tests**
  - Save and load RDB file
  - Background save (BGSAVE)
  - RDB compression
  - Large dataset save/load
  - Corrupted RDB file handling

- **AOF Tests**
  - Command logging correctness
  - AOF rewrite (BGREWRITEAOF)
  - AOF loading on startup
  - Hybrid RDB-AOF persistence
  - AOF corruption recovery

- **Combined Tests**
  - RDB + AOF together
  - Failover scenarios
  - Data integrity after restart

#### 2.3 Replication Integration
**File**: `tests/integration/replication.rs`

Test Cases:
- Master-replica setup
- Full synchronization
- Partial resynchronization
- Command propagation
- Replica-to-master promotion
- Multiple replicas
- Chained replication
- Replica read operations

#### 2.4 Cluster Integration
**File**: `tests/integration/cluster.rs`

Test Cases:
- Cluster formation (3 masters, 3 replicas)
- Slot distribution
- Cluster resharding
- Node failure detection
- Automatic failover
- Cross-slot operations handling
- Cluster topology changes

#### 2.5 Pub/Sub Integration
**File**: `tests/integration/pubsub.rs`

Test Cases:
- Subscribe and publish
- Pattern subscriptions (PSUBSCRIBE)
- Multiple subscribers
- Unsubscribe behavior
- Pub/Sub with persistence
- Pub/Sub in cluster mode

#### 2.6 Lua Scripting Integration
**File**: `tests/integration/scripting.rs`

Test Cases:
- EVAL command execution
- EVALSHA with script caching
- Scripts with keys and args
- Redis command calls from Lua
- Script atomicity
- Script timeout
- Scripting errors handling

### Integration Test Example

```rust
#[tokio::test]
async fn test_master_replica_sync() {
    // Start master
    let master = RedisServer::new("127.0.0.1:6379").start().await;

    // Start replica
    let replica = RedisServer::new("127.0.0.1:6380")
        .replica_of("127.0.0.1:6379")
        .start().await;

    // Wait for sync
    tokio::time::sleep(Duration::from_secs(1)).await;

    // Write to master
    master.set("key", "value").await.unwrap();

    // Read from replica
    let value = replica.get("key").await.unwrap();
    assert_eq!(value, Some("value".as_bytes()));

    // Verify replication offset
    let master_offset = master.repl_offset().await;
    let replica_offset = replica.repl_offset().await;
    assert_eq!(master_offset, replica_offset);
}
```

## 3. End-to-End Tests

### Coverage Target: 70%+

**Location**: `tests/e2e/`

#### 3.1 Redis Test Suite Translation
**Strategy**: Translate Redis TCL tests to Rust

**Redis Test Files to Translate** (from `redis/tests/`):
- `tests/unit/type/string.tcl` → `tests/e2e/string_commands.rs`
- `tests/unit/type/list.tcl` → `tests/e2e/list_commands.rs`
- `tests/unit/type/set.tcl` → `tests/e2e/set_commands.rs`
- `tests/unit/type/hash.tcl` → `tests/e2e/hash_commands.rs`
- `tests/unit/type/zset.tcl` → `tests/e2e/zset_commands.rs`
- `tests/unit/type/stream.tcl` → `tests/e2e/stream_commands.rs`
- `tests/unit/expire.tcl` → `tests/e2e/expiration.rs`
- `tests/unit/multi.tcl` → `tests/e2e/transactions.rs`
- `tests/unit/pubsub.tcl` → `tests/e2e/pubsub.rs`
- `tests/unit/scripting.tcl` → `tests/e2e/scripting.rs`
- `tests/unit/aofrw.tcl` → `tests/e2e/aof_rewrite.rs`
- `tests/integration/replication.tcl` → `tests/e2e/replication.rs`
- `tests/cluster/tests/` → `tests/e2e/cluster/`

#### 3.2 E2E Test Framework

```rust
// tests/e2e/common/mod.rs
pub struct TestRedisServer {
    process: Child,
    port: u16,
    client: redis::Client,
}

impl TestRedisServer {
    pub async fn start() -> Self { /* ... */ }
    pub async fn stop(self) { /* ... */ }
    pub async fn restart(&mut self) { /* ... */ }
    pub fn client(&self) -> &redis::Client { /* ... */ }
}

pub struct TestCluster {
    nodes: Vec<TestRedisServer>,
}

impl TestCluster {
    pub async fn create(masters: usize, replicas: usize) -> Self { /* ... */ }
    pub async fn fail_node(&mut self, index: usize) { /* ... */ }
    pub async fn recover_node(&mut self, index: usize) { /* ... */ }
}
```

#### 3.3 E2E Test Categories

**3.3.1 Command Correctness** (`tests/e2e/commands/`)
- Test all commands with various inputs
- Verify exact output matches Redis behavior
- Test command combinations
- Error message compatibility

**3.3.2 Data Persistence** (`tests/e2e/persistence/`)
- RDB save/load correctness
- AOF replay correctness
- Data integrity after crashes (simulated)
- Persistence performance tests

**3.3.3 Replication** (`tests/e2e/replication/`)
- Master-replica consistency
- Failover scenarios
- Network partition handling
- Replication lag monitoring

**3.3.4 Clustering** (`tests/e2e/cluster/`)
- Cluster operations
- Slot migration
- Multi-key operations
- Cluster reconfiguration

**3.3.5 Client Compatibility** (`tests/e2e/clients/`)
- Test with official Redis clients:
  - redis-cli
  - redis-py (Python)
  - redis-rb (Ruby)
  - node-redis (Node.js)
  - go-redis (Go)
  - redis-rs (Rust)

### E2E Test Example

```rust
// tests/e2e/string_commands.rs
#[tokio::test]
async fn test_string_operations() {
    let server = TestRedisServer::start().await;
    let mut conn = server.client().get_async_connection().await.unwrap();

    // Test from Redis test suite: tests/unit/type/string.tcl

    // SET and GET
    redis::cmd("SET").arg("mykey").arg("Hello").query_async(&mut conn).await.unwrap();
    let value: String = redis::cmd("GET").arg("mykey").query_async(&mut conn).await.unwrap();
    assert_eq!(value, "Hello");

    // APPEND
    let len: i32 = redis::cmd("APPEND").arg("mykey").arg(" World").query_async(&mut conn).await.unwrap();
    assert_eq!(len, 11);

    let value: String = redis::cmd("GET").arg("mykey").query_async(&mut conn).await.unwrap();
    assert_eq!(value, "Hello World");

    // GETRANGE
    let substr: String = redis::cmd("GETRANGE").arg("mykey").arg(0).arg(4).query_async(&mut conn).await.unwrap();
    assert_eq!(substr, "Hello");

    server.stop().await;
}
```

## 4. Performance Tests (Benchmarks)

**Location**: `benches/`

#### 4.1 Micro Benchmarks
**File**: `benches/micro.rs`

Using Criterion.rs:
```rust
fn bench_set(c: &mut Criterion) {
    c.bench_function("SET command", |b| {
        b.iter(|| {
            // Benchmark SET command
        });
    });
}

fn bench_get(c: &mut Criterion) {
    c.bench_function("GET command", |b| {
        b.iter(|| {
            // Benchmark GET command
        });
    });
}

criterion_group!(benches, bench_set, bench_get, bench_lpush, bench_zadd);
criterion_main!(benches);
```

Benchmarks:
- Individual command execution time
- Protocol parsing/serialization speed
- Data structure operations (list push/pop, zset add/range, etc.)
- Memory allocation patterns

#### 4.2 Throughput Benchmarks
**File**: `benches/throughput.rs`

Test Scenarios:
- Requests per second (QPS) for different commands
- Pipeline vs non-pipeline performance
- Concurrent client throughput
- Mixed workload performance

#### 4.3 Latency Benchmarks
**File**: `benches/latency.rs`

Metrics:
- P50, P95, P99, P999 latencies
- Command latency distribution
- Impact of persistence on latency
- Replication lag measurement

#### 4.4 Memory Benchmarks
**File**: `benches/memory.rs`

Test Cases:
- Memory overhead per key
- Memory usage for different data types
- Eviction performance
- Memory fragmentation over time

#### 4.5 Comparison with Redis
**File**: `benches/comparison.rs`

Side-by-side benchmarks:
- Same workload on Redis and Redis-Rust
- Performance gap analysis
- Identify optimization opportunities

### Benchmark Example

```rust
use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};

fn bench_commands(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let server = rt.block_on(async { RedisServer::start().await });

    let mut group = c.benchmark_group("commands");

    for size in [10, 100, 1000, 10000].iter() {
        group.bench_with_input(BenchmarkId::new("SET", size), size, |b, &size| {
            b.iter(|| {
                rt.block_on(async {
                    server.set(black_box("key"), black_box("x".repeat(size))).await
                });
            });
        });
    }

    group.finish();
}

criterion_group!(benches, bench_commands);
criterion_main!(benches);
```

## 5. Property-Based Testing

**Location**: `tests/property/`

Using `proptest`:

```rust
use proptest::prelude::*;

proptest! {
    #[test]
    fn test_set_get_roundtrip(key in "\\PC*", value in "\\PC*") {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let db = Database::new();
            db.set(&key, &value).await.unwrap();
            let result = db.get(&key).await.unwrap();
            prop_assert_eq!(result, Some(value.as_bytes()));
        });
    }

    #[test]
    fn test_list_operations(ops in prop::collection::vec(list_operation(), 0..100)) {
        // Test random sequence of list operations
        // Verify consistency
    }
}
```

## 6. Fuzzing

**Location**: `fuzz/`

Using `cargo-fuzz`:

```rust
// fuzz/fuzz_targets/protocol_parser.rs
#![no_main]
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let _ = RespParser::parse(data);
});

// fuzz/fuzz_targets/commands.rs
fuzz_target!(|data: Vec<Vec<u8>>| {
    if let Ok(cmd) = Command::parse(data) {
        let _ = execute_command(cmd);
    }
});
```

Fuzzing Targets:
- Protocol parser (malformed RESP)
- Command execution (random inputs)
- RDB/AOF file parsing (corrupted files)
- Cluster protocol messages

## 7. Stress Testing

**Location**: `tests/stress/`

#### 7.1 Load Testing
- Sustained high QPS (100k+ requests/sec)
- Memory pressure testing
- Long-running stability tests (24h+)

#### 7.2 Chaos Testing
- Random node failures
- Network partitions
- Disk I/O failures
- Clock skew

#### 7.3 Resource Exhaustion
- Max connections
- Max memory
- Max keys
- File descriptor limits

## 8. Test Execution

### CI/CD Pipeline

```yaml
# .github/workflows/test.yml
test:
  - name: Unit Tests
    run: cargo test --lib

  - name: Integration Tests
    run: cargo test --test '*'

  - name: E2E Tests
    run: cargo test --test 'e2e_*'

  - name: Benchmarks
    run: cargo bench --no-run

  - name: Code Coverage
    run: cargo tarpaulin --out Lcov
```

### Test Execution Commands

```bash
# Run all tests
cargo test

# Run unit tests only
cargo test --lib

# Run integration tests
cargo test --test integration

# Run E2E tests
cargo test --test e2e

# Run specific test
cargo test test_name

# Run with output
cargo test -- --nocapture

# Run benchmarks
cargo bench

# Code coverage
cargo tarpaulin --out Html

# Fuzzing
cargo +nightly fuzz run protocol_parser
```

## 9. Test Metrics

### Coverage Goals
- Line coverage: >85%
- Branch coverage: >80%
- Critical path coverage: 100%

### Quality Metrics
- Zero memory leaks (valgrind/ASAN)
- Zero data races (thread sanitizer)
- Zero undefined behavior (MSAN)
- <1% test flakiness

### Performance Targets
- Throughput: >80% of native Redis
- Latency P99: <2x native Redis
- Memory overhead: <20% vs native Redis

## 10. Test Documentation

Each test should include:
- **Purpose**: What is being tested
- **Setup**: Initial state and prerequisites
- **Execution**: Steps performed
- **Verification**: Expected outcomes
- **Cleanup**: Resource cleanup

Example:
```rust
/// Tests that the SET command correctly stores a string value
/// and returns OK response.
///
/// Setup: Clean database
/// Execution: SET key value
/// Verification: Response is OK, GET returns value
/// Cleanup: Database is dropped
#[tokio::test]
async fn test_set_command() {
    // Test implementation
}
```

## 11. Redis TCL Test Translation Guide

### Translation Mapping

Redis TCL → Rust:
```tcl
# Redis TCL
start_server {tags {"string"}} {
    test "SET and GET" {
        r set mykey "Hello"
        assert_equal "Hello" [r get mykey]
    }
}
```

```rust
// Rust equivalent
#[tokio::test]
async fn test_set_and_get() {
    let server = TestRedisServer::start().await;
    let mut conn = server.client().get_async_connection().await.unwrap();

    redis::cmd("SET").arg("mykey").arg("Hello").query_async(&mut conn).await.unwrap();
    let value: String = redis::cmd("GET").arg("mykey").query_async(&mut conn).await.unwrap();
    assert_eq!(value, "Hello");

    server.stop().await;
}
```

### Test Files to Translate (Priority Order)

1. **High Priority** (Core functionality):
   - `unit/type/string.tcl`
   - `unit/type/list.tcl`
   - `unit/type/set.tcl`
   - `unit/type/hash.tcl`
   - `unit/type/zset.tcl`
   - `unit/expire.tcl`
   - `unit/multi.tcl`

2. **Medium Priority** (Advanced features):
   - `unit/type/stream.tcl`
   - `unit/pubsub.tcl`
   - `unit/scripting.tcl`
   - `unit/aofrw.tcl`
   - `unit/dump.tcl`

3. **Lower Priority** (Cluster/Advanced):
   - `integration/replication.tcl`
   - `cluster/tests/*.tcl`
   - `unit/acl.tcl`

## Summary

This comprehensive test plan ensures:
- ✅ High code quality and reliability
- ✅ Protocol compatibility with Redis
- ✅ Performance validation
- ✅ Regression prevention
- ✅ Continuous integration
- ✅ Production readiness

Total estimated test count: **2000+ tests**
- Unit: ~1000 tests
- Integration: ~500 tests
- E2E: ~400 tests
- Benchmarks: ~100 scenarios
