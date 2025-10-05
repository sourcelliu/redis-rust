# Redis Cluster Implementation Plan

## Overview
This document outlines the comprehensive plan for implementing Redis Cluster functionality in redis-rust.

## Background
Redis Cluster provides a way to run a Redis installation where data is automatically sharded across multiple Redis nodes. It provides:
- Automatic data sharding across multiple nodes
- High availability with automatic failover
- Linear scalability
- No single point of failure

## Architecture Components

### 1. Hash Slots (16384 slots)
- Every key belongs to a hash slot
- Hash slot = CRC16(key) mod 16384
- Each master node is responsible for a subset of hash slots
- Supports hash tags for multi-key operations: {tag}key

### 2. Cluster Topology
- Minimum 3 master nodes recommended
- Each master can have 0-N replica nodes
- Nodes communicate via gossip protocol
- Each node maintains cluster state

### 3. Client Redirection
- **MOVED**: Permanent redirection when slot has moved
- **ASK**: Temporary redirection during migration
- Clients should update their slot mapping on MOVED

## Implementation Phases

### Phase 1: Core Infrastructure (Phase 36)
**Goal**: Implement hash slot calculation and basic cluster state

**Tasks**:
1. Implement CRC16 algorithm for key hashing
2. Implement hash slot calculation (16384 slots)
3. Support hash tags {user}:profile
4. Create ClusterState structure
5. Implement slot-to-node mapping

**Files to Create/Modify**:
- `src/cluster/slots.rs` - Hash slot calculation
- `src/cluster/mod.rs` - Cluster state management
- Add unit tests

**Estimated Lines**: ~200 lines

**Success Criteria**:
- ✅ CRC16 correctly calculates hash slots
- ✅ Hash tags work correctly
- ✅ Slot mapping can be queried
- ✅ Unit tests pass

---

### Phase 2: Cluster Node Management (Phase 37)
**Goal**: Implement node discovery and cluster topology

**Tasks**:
1. Implement ClusterNode structure
2. Node ID generation (40 hex chars)
3. Node flags (master, slave, myself, fail, etc.)
4. Slot range assignment to nodes
5. Node state persistence

**Files to Create/Modify**:
- `src/cluster/node.rs` - Node management
- Add node addition/removal logic
- Add unit tests

**Estimated Lines**: ~250 lines

**Success Criteria**:
- ✅ Nodes can be added/removed
- ✅ Node states are tracked
- ✅ Slot ranges correctly assigned

---

### Phase 3: CLUSTER Commands (Phase 38)
**Goal**: Implement essential CLUSTER commands

**Commands to Implement**:
1. `CLUSTER NODES` - List all cluster nodes
2. `CLUSTER SLOTS` - Get cluster slot mapping
3. `CLUSTER ADDSLOTS` - Assign slots to this node
4. `CLUSTER DELSLOTS` - Remove slot assignments
5. `CLUSTER MEET` - Add a node to cluster
6. `CLUSTER FORGET` - Remove a node from cluster
7. `CLUSTER REPLICATE` - Make this node a replica
8. `CLUSTER INFO` - Get cluster state info
9. `CLUSTER MYID` - Get this node's ID
10. `CLUSTER KEYSLOT` - Get hash slot for a key

**Files to Create/Modify**:
- `src/commands/cluster.rs` - CLUSTER command handlers
- `src/commands/dispatcher.rs` - Add CLUSTER routing
- Add integration tests

**Estimated Lines**: ~400 lines

**Success Criteria**:
- ✅ All 10 commands work correctly
- ✅ CLUSTER NODES returns correct format
- ✅ CLUSTER SLOTS returns slot mapping

---

### Phase 4: Client Redirection (Phase 39)
**Goal**: Implement MOVED and ASK redirections

**Tasks**:
1. Check slot ownership before command execution
2. Return MOVED error if slot not owned
3. Implement slot migration states
4. Return ASK error during migration
5. Implement ASKING command
6. Add redirection logic to all data commands

**Files to Create/Modify**:
- `src/cluster/redirection.rs` - Redirection logic
- Modify command dispatcher to check slots
- Add ASKING command
- Add integration tests

**Estimated Lines**: ~300 lines

**Success Criteria**:
- ✅ MOVED errors returned correctly
- ✅ ASK errors during migration
- ✅ ASKING command works
- ✅ Clients can follow redirections

---

### Phase 5: Slot Migration (Phase 40)
**Goal**: Implement slot migration between nodes

**Commands**:
1. `CLUSTER SETSLOT IMPORTING` - Mark slot as importing
2. `CLUSTER SETSLOT MIGRATING` - Mark slot as migrating
3. `CLUSTER SETSLOT STABLE` - Mark migration complete
4. `CLUSTER SETSLOT NODE` - Assign slot to node
5. `CLUSTER GETKEYSINSLOT` - Get keys in a slot
6. `CLUSTER COUNTKEYSINSLOT` - Count keys in slot

**Tasks**:
1. Track importing/migrating state per slot
2. Implement key-by-key migration
3. Handle ASK redirections during migration
4. Atomic slot ownership transfer

**Files to Create/Modify**:
- `src/cluster/migration.rs` - Migration logic
- Add migration commands
- Add migration tests

**Estimated Lines**: ~350 lines

**Success Criteria**:
- ✅ Slots can be migrated between nodes
- ✅ No data loss during migration
- ✅ Clients redirected correctly

---

### Phase 6: Cluster Configuration (Phase 41)
**Goal**: Persist and restore cluster configuration

**Tasks**:
1. Save cluster state to nodes.conf
2. Load cluster state on startup
3. Cluster epoch tracking
4. Configuration updates

**Files to Create/Modify**:
- `src/cluster/config.rs` - Configuration persistence
- Add config load/save
- Add tests

**Estimated Lines**: ~200 lines

**Success Criteria**:
- ✅ Cluster state persists across restarts
- ✅ nodes.conf format compatible with Redis

---

### Phase 7: E2E Testing Suite (Phase 42)
**Goal**: Comprehensive end-to-end cluster tests

**Test Scenarios**:
1. **Basic Cluster Setup**
   - Create 3-node cluster
   - Assign slots evenly
   - Verify CLUSTER NODES
   - Verify CLUSTER SLOTS

2. **Key Distribution**
   - Insert 10,000 keys
   - Verify distributed across nodes
   - Verify hash slot correctness

3. **Client Redirection**
   - Access key from wrong node
   - Verify MOVED error
   - Follow redirection
   - Verify success

4. **Slot Migration**
   - Migrate slot 0-1000 from node1 to node2
   - Verify ASK during migration
   - Verify MOVED after completion
   - Verify no data loss

5. **Node Failure**
   - Simulate node failure
   - Verify cluster continues (if replicas exist)
   - Verify failover (future enhancement)

6. **Multi-key Operations**
   - Test MGET/MSET with hash tags
   - Verify cross-slot errors

**Files to Create**:
- `tests/cluster_e2e.rs` - Main E2E test suite
- `tests/cluster_helper.rs` - Test utilities

**Estimated Lines**: ~600 lines

**Success Criteria**:
- ✅ All E2E tests pass
- ✅ No data corruption
- ✅ Correct error handling
- ✅ Performance acceptable

---

## Future Enhancements (Post Phase 42)

### Phase 43+: Advanced Features
1. **Automatic Failover**
   - Replica promotion
   - Quorum-based decisions
   - Epoch increments

2. **Gossip Protocol**
   - Node heartbeats
   - Cluster state synchronization
   - Failure detection

3. **Pub/Sub in Cluster Mode**
   - Cluster-wide pub/sub
   - Message routing

4. **Resharding Support**
   - Online resharding
   - Automatic rebalancing

5. **Cluster Monitoring**
   - Cluster health metrics
   - Slot distribution stats
   - Migration progress

## Testing Strategy

### Unit Tests
- Test CRC16 algorithm correctness
- Test slot calculation
- Test hash tag parsing
- Test node state management
- Test slot assignment logic

### Integration Tests
- Test CLUSTER commands
- Test slot redirection
- Test migration workflow
- Test configuration persistence

### E2E Tests
- Multi-node cluster setup
- Real client interactions
- Failure scenarios
- Migration scenarios
- Performance benchmarks

## Documentation Updates

### User Documentation
1. **Getting Started with Cluster**
   - Minimum cluster setup
   - Configuration options
   - Basic operations

2. **Cluster Management Guide**
   - Adding/removing nodes
   - Slot management
   - Migration procedures

3. **Client Configuration**
   - Cluster-aware client setup
   - Handling redirections
   - Hash tags usage

### Developer Documentation
1. **Architecture Overview**
   - Cluster components
   - Data flow
   - State management

2. **API Reference**
   - All CLUSTER commands
   - Error codes
   - Return formats

## Success Metrics

### Functional Metrics
- ✅ All planned CLUSTER commands implemented
- ✅ Correct hash slot distribution
- ✅ Zero data loss during migration
- ✅ Compatible with Redis Cluster protocol

### Quality Metrics
- ✅ >95% test coverage for cluster code
- ✅ All E2E tests passing
- ✅ No memory leaks
- ✅ No race conditions

### Performance Metrics
- ✅ Slot lookup < 1μs
- ✅ Redirection overhead < 100μs
- ✅ Migration throughput > 1000 keys/sec
- ✅ Cluster info queries < 10ms

## Timeline Estimation

| Phase | Description | Estimated Time | Lines of Code |
|-------|-------------|----------------|---------------|
| 36 | Core Infrastructure | 1 session | ~200 |
| 37 | Node Management | 1 session | ~250 |
| 38 | CLUSTER Commands | 1-2 sessions | ~400 |
| 39 | Client Redirection | 1 session | ~300 |
| 40 | Slot Migration | 1-2 sessions | ~350 |
| 41 | Configuration | 1 session | ~200 |
| 42 | E2E Testing | 1-2 sessions | ~600 |
| **Total** | **Full Implementation** | **8-11 sessions** | **~2300 lines** |

## Dependencies

### External Crates (if needed)
- None - use existing dependencies (dashmap, tokio, bytes)

### Internal Dependencies
- Existing database layer
- Command dispatcher
- RESP protocol implementation
- Config management

## Risk Mitigation

### Technical Risks
1. **Complexity**: Break into small phases
2. **Testing**: Comprehensive test suite
3. **Performance**: Profile and optimize
4. **Compatibility**: Follow Redis protocol exactly

### Operational Risks
1. **Data Loss**: Extensive testing before production
2. **Split Brain**: Implement quorum properly
3. **Migration Failures**: Rollback mechanisms

## Conclusion

This plan provides a structured approach to implementing Redis Cluster in redis-rust. By following these phases systematically, we can build a robust, tested, and production-ready cluster implementation.

**Next Steps**:
1. Review and approve this plan
2. Begin Phase 36: Core Infrastructure
3. Implement and test each phase incrementally
4. Update documentation continuously
5. Run E2E tests after each phase
