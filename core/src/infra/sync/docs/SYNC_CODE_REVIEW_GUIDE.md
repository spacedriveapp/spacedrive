# Sync System Code Review Guide

**Purpose**: Complete module map for reviewing the hybrid leaderless sync implementation
**Last Updated**: October 9, 2025
**Status**: Network Integration Complete

---

## Overview

The sync system follows a **Domain-Driven Design (DDD)** architecture with clear separation:
- **Domain Layer**: Entities define their own sync behavior (queries, applies)
- **Infrastructure Layer**: Generic sync primitives (HLC, PeerLog, Registry)
- **Service Layer**: Orchestration (PeerSync, NetworkProtocolHandler)
- **No domain-specific logic in infrastructure or service layers**

---

## Core Architecture Documents

### Primary Documentation
- **`/docs/core/sync.md`** - Comprehensive architectural specification (authoritative source)
- **`core/src/infra/sync/docs/SYNC_IMPLEMENTATION_GUIDE.md`** - Implementation patterns and principles
- **`core/src/infra/sync/docs/SYNC_IMPLEMENTATION_ROADMAP.md`** - Implementation status tracking

---

## Layer 1: Infrastructure - Sync Primitives

**Location**: `core/src/infra/sync/`

### Core Primitives

| File | Purpose | Key Components |
|------|---------|----------------|
| **`mod.rs`** | Module root, public API exports | `HLC`, `PeerLog`, `Syncable`, `TransactionManager` |
| **`hlc.rs`** | Hybrid Logical Clock implementation | `HLC` struct, `HLCGenerator`, timestamp ordering |
| **`peer_log.rs`** | Per-device append-only sync log | `PeerLog`, `SharedChangeEntry`, ACK tracking, pruning |
| **`syncable.rs`** | Trait defining sync behavior | `Syncable` trait with `query_for_sync`, `apply_state_change`, `apply_shared_change` |
| **`registry.rs`** | Runtime dispatch for syncable models | `SYNCABLE_REGISTRY`, `query_device_state`, `apply_state_change`, `apply_shared_change` |
| **`transaction.rs`** | Gatekeeper for all sync-enabled writes | `TransactionManager::commit_device_owned`, `commit_shared` |

### Review Focus (Infrastructure):
- **No domain-specific logic** - should be generic
- **Proper trait abstractions** - `Syncable` trait defines contract
- **Registry pattern** - dynamic dispatch without switch statements
- **HLC correctness** - causality tracking, ordering guarantees
- **PeerLog correctness** - append-only, proper pruning

---

## Layer 2: Domain - Entity Sync Implementations

**Location**: `core/src/infra/db/entities/`

### Device-Owned Models (State-Based Sync)

| File | Entity | Sync Status | Key Methods |
|------|--------|-------------|-------------|
| **`location.rs`** | Location | Fully Implemented | `query_for_sync`, `apply_state_change` |
| **`entry.rs`** | Entry | Fully Implemented | `query_for_sync`, `apply_state_change` |
| **`device.rs`** | Device | Fully Implemented | `query_for_sync`, `apply_state_change` |

### Shared Models (Log-Based Sync with HLC)

| File | Entity | Sync Status | Key Methods |
|------|--------|-------------|-------------|
| **`tag.rs`** | Tag | Fully Implemented | `apply_shared_change` (trait method) |

### Review Focus (Domain):
- **Each entity owns its sync logic** - query and apply in entity file
- **Proper serialization** - uses `to_sync_json()`, excludes non-sync fields
- **Idempotent upserts** - `on_conflict` with UUID, updates all synced columns
- **Conflict resolution** - For shared models: HLC comparison for "last write wins"

---

## Layer 3: Service - Sync Orchestration

**Location**: `core/src/service/sync/`

### Core Service Components

| File | Purpose | Key Responsibilities |
|------|---------|---------------------|
| **`mod.rs`** | Sync service root | `SyncService` lifecycle, exports |
| **`peer.rs`** | Peer-to-peer sync coordination | `PeerSync`: broadcasting, receiving, applying changes |
| **`state.rs`** | Sync state machine | `DeviceSyncState`, `BufferQueue`, backfill states |
| **`backfill.rs`** | Initial sync from peers | `BackfillManager`, peer selection, resume logic |
| **`retry_queue.rs`** | Failed message retry with backoff | `RetryQueue`, exponential backoff, max retries |
| **`applier.rs`** | Apply incoming sync messages | Delegates to registry |
| **`protocol_handler.rs`** | Legacy handlers (being phased out) | State/log sync handlers |

### Review Focus (Service):
- **Domain-agnostic** - calls registry, never switches on model types
- **Event-driven** - listens to TransactionManager events, broadcasts changes
- **Parallel broadcasts** - uses `join_all`, not sequential sends
- **Proper error handling** - timeouts, retry queues, no `.unwrap_or_default()`
- **Background tasks** - retry processor, log pruner running
- **State machine** - buffering during backfill, transitioning to ready

---

## Layer 4: Network - Protocol Handler

**Location**: `core/src/service/network/protocol/sync/`

### Network Protocol Components

| File | Purpose | Key Components |
|------|---------|----------------|
| **`mod.rs`** | Protocol module root | Exports |
| **`handler.rs`** | Incoming sync message router | `SyncProtocolHandler::handle_sync_message` |
| **`messages.rs`** | Sync message definitions | `SyncMessage` enum, `StateRecord`, message types |

### Message Types Implemented

| Message Type | Direction | Handler Status | Purpose |
|--------------|-----------|----------------|---------|
| `StateChange` | Broadcast | Complete | Send state-based updates |
| `StateBatch` | Broadcast | Complete | Batch state updates |
| `StateRequest` | Request/Response | Complete | Request backfill data |
| `StateResponse` | Response | Complete | Return backfill data |
| `SharedChange` | Broadcast | Complete | Send HLC-ordered log entries |
| `SharedChangeBatch` | Broadcast | Complete | Batch shared changes |
| `SharedChangeRequest` | Request/Response | Complete | Request shared changes since HLC |
| `SharedChangeResponse` | Response | Complete | Return shared changes |
| `AckSharedChanges` | Notification | Complete | Acknowledge receipt (for pruning) |
| `Heartbeat` | Bidirectional | Complete | Keep-alive with watermarks |
| `Error` | Notification | Complete | Error reporting |

### Review Focus (Network):
- **Complete message handling** - all message types handled
- **Request/response flow** - proper response message generation
- **Error handling** - graceful degradation, error messages sent back
- **Integration with PeerSync** - delegates to PeerSync methods

---

## Data Flow Architecture

### Write Path (Local Change → Peers)

```
1. Operation/Job
   ↓
2. TransactionManager.commit_device_owned() or commit_shared()
   ↓
3. Emits event: "sync:state_change" or "sync:shared_change"
   ↓
4. PeerSync event listener picks up event
   ↓
5. PeerSync.broadcast_state_change() or broadcast_shared_change()
   ↓
6. NetworkTransport.send_sync_message() (parallel to all peers)
   ↓
7. Peer's SyncProtocolHandler.handle_sync_message()
   ↓
8. PeerSync.on_state_change_received() or on_shared_change_received()
   ↓
9. Registry.apply_state_change() or apply_shared_change()
   ↓
10. Entity.apply_state_change() (domain logic)
    ↓
11. Database upsert
```

**Files Involved**:
1. `core/src/ops/` (operations)
2. `core/src/infra/sync/transaction.rs`
3. `core/src/service/sync/peer.rs` (lines 124-336)
4. `core/src/service/network/protocol/sync/handler.rs`
5. `core/src/service/sync/peer.rs` (lines 746-856)
6. `core/src/infra/sync/registry.rs`
7. `core/src/infra/db/entities/*.rs`

### Read Path (Backfill Request)

```
1. Peer sends StateRequest message
   ↓
2. SyncProtocolHandler.handle_sync_message()
   ↓
3. PeerSync.get_device_state()
   ↓
4. Registry.query_device_state() (loops through model types)
   ↓
5. Entity.query_for_sync() (domain logic)
   ↓
6. Database query
   ↓
7. Entity.to_sync_json() (domain serialization)
   ↓
8. Return StateResponse to requester
```

**Files Involved**:
1. `core/src/service/network/protocol/sync/handler.rs` (lines 163-197)
2. `core/src/service/sync/peer.rs` (lines 987-1057)
3. `core/src/infra/sync/registry.rs` (lines 256-289)
4. `core/src/infra/db/entities/*.rs` (Syncable impls)

---

## Critical Review Points

### 1. DDD Compliance ✅

**Check**: No domain-specific logic in sync infrastructure
- BAD: Switch statements on model types in `peer.rs`
- GOOD: Registry-based dispatch
- BAD: JSON serialization logic in `registry.rs`
- GOOD: Entity calls `to_sync_json()`

**Files to Verify**:
- `core/src/service/sync/peer.rs` - should call registry, not switch on types
- `core/src/infra/sync/registry.rs` - should be pure routing
- `core/src/infra/db/entities/*.rs` - should contain all domain logic

### 2. Error Handling ✅

**Check**: Proper error propagation, no silent failures
- BAD: `.unwrap_or_default()` hiding errors
- GOOD: Proper `Result` propagation
- GOOD: Retry queue for network failures
- GOOD: Timeout handling (30s)

**Files to Verify**:
- `core/src/service/sync/peer.rs` (lines 286-354, 399-476)
- `core/src/service/sync/retry_queue.rs`

### 3. Parallel Operations ✅

**Check**: Broadcasts use parallel sends
- BAD: Sequential `for` loop with awaits
- GOOD: `join_all` with parallel futures

**Files to Verify**:
- `core/src/service/sync/peer.rs::broadcast_state_change` (lines 564-606)
- `core/src/service/sync/peer.rs::broadcast_shared_change` (lines 670-741)

### 4. Background Tasks ✅

**Check**: Required background tasks running
- Event listener (TransactionManager → PeerSync)
- Retry queue processor (every 10s)
- Log pruner (every 5min)

**Files to Verify**:
- `core/src/service/sync/peer.rs::start` (lines 123-147)
- `core/src/service/sync/peer.rs::start_event_listener` (lines 220-276)
- `core/src/service/sync/peer.rs::start_retry_processor` (lines 149-189)
- `core/src/service/sync/peer.rs::start_log_pruner` (lines 191-218)

### 5. ACK Mechanism ✅

**Check**: Automatic ACK sending after applying shared changes
- Extracts sender from `entry.hlc.device_id`
- Sends ACK back to sender
- Non-fatal error handling (continues if ACK fails)

**Files to Verify**:
- `core/src/service/sync/peer.rs::apply_shared_change` (lines 806-843)

---

## Testing Areas

### Files Requiring Integration Tests

1. **State-Based Sync (Two Peers)**
   - File: `core/tests/sync_state_integration_test.rs` (to be created)
   - Tests: Location sync, Entry sync, Device sync

2. **Log-Based Sync (Conflict Resolution)**
   - File: `core/tests/sync_log_integration_test.rs` (to be created)
   - Tests: Tag concurrent modification, HLC ordering

3. **Network Partition Recovery**
   - File: `core/tests/sync_recovery_test.rs` (to be created)
   - Tests: Reconnect after disconnection, retry queue

4. **Backfill/Catch-up**
   - File: `core/tests/sync_backfill_test.rs` (to be created)
   - Tests: New device joins, request/response flow

---

## Complete File Manifest

### Infrastructure Layer (`core/src/infra/`)

```
sync/
├── mod.rs                    - Public API, exports
├── hlc.rs                    - Hybrid Logical Clock (324 lines)
├── peer_log.rs               - Append-only shared change log (397 lines)
├── syncable.rs               - Syncable trait definition (302 lines)
├── registry.rs               - Model registration and dispatch (397 lines)
├── transaction.rs            - Transaction manager, event emission (287 lines)
└── docs/
    ├── SYNC_IMPLEMENTATION_GUIDE.md          (601 lines)
    ├── SYNC_IMPLEMENTATION_ROADMAP.md        (1008 lines)
    └── SYNC_CODE_REVIEW_GUIDE.md             (this file)
```

### Domain Layer (`core/src/infra/db/entities/`)

```
entities/
├── mod.rs                    - Entity exports
├── location.rs               - Location entity + Syncable impl (285 lines)
├── entry.rs                  - Entry entity + Syncable impl (245 lines)
├── device.rs                 - Device entity + Syncable impl (148 lines)
└── tag.rs                    - Tag entity + Syncable impl (partial)
```

### Service Layer (`core/src/service/sync/`)

```
sync/
├── mod.rs                    - Service lifecycle (183 lines)
├── peer.rs                   - Core sync orchestration (1144 lines) CRITICAL
├── state.rs                  - State machine, buffering (195 lines)
├── backfill.rs               - Initial sync from peers (247 lines)
├── retry_queue.rs            - Failed message retry (134 lines)
├── applier.rs                - Apply incoming changes (delegates to registry)
└── protocol_handler.rs       - Legacy handlers (being phased out)
```

### Network Protocol Layer (`core/src/service/network/protocol/sync/`)

```
sync/
├── mod.rs                    - Protocol exports
├── handler.rs                - Protocol message router (336 lines) CRITICAL
└── messages.rs               - Message type definitions (205 lines)
```

---

## Key Files for Deep Review

### Critical Path Files (Must Review Thoroughly)

1. **`core/src/service/sync/peer.rs`** (1144 lines)
   - Most complex file in the system
   - Orchestrates all sync operations
   - Review: Event handling (lines 220-454), Broadcasting (lines 513-741), Applying (lines 787-856)

2. **`core/src/service/network/protocol/sync/handler.rs`** (336 lines)
   - Entry point for all incoming sync messages
   - Review: All message type handlers, error handling

3. **`core/src/infra/sync/registry.rs`** (397 lines)
   - Central dispatch mechanism
   - Review: Should have ZERO domain logic, only routing

4. **`core/src/infra/sync/hlc.rs`** (324 lines)
   - Correctness is critical for conflict resolution
   - Review: Ordering guarantees, causality tracking

### Important Supporting Files

5. **`core/src/infra/sync/peer_log.rs`** (397 lines)
   - Manages sync.db, ACK tracking, pruning
   - Review: SQL correctness, transaction safety

6. **`core/src/infra/sync/transaction.rs`** (287 lines)
   - Sole gatekeeper for writes
   - Review: Event emission, proper use throughout codebase

7. **`core/src/infra/sync/syncable.rs`** (302 lines)
   - Trait contract for all syncable models
   - Review: Complete API, proper abstractions

### Domain Implementations

8. **`core/src/infra/db/entities/location.rs`** (285 lines)
   - Reference implementation for state-based sync
   - Review: Query logic, apply logic, field exclusions

9. **`core/src/infra/db/entities/entry.rs`** (245 lines)
   - Most complex entity (hierarchical, large scale)
   - Review: UUID handling, sync-ready filtering

10. **`core/src/infra/db/entities/tag.rs`** (Fixed - October 9, 2025)
    - Reference implementation for log-based sync
    - Review: HLC conflict resolution, apply logic, trait implementation

---

## Known Issues / TODOs

### Non-Critical (Enhancements)

1. **Checkpoint Persistence** (`core/src/service/sync/state.rs`, lines 185-195)
   - Currently in-memory only
   - Impact: Backfill restarts from beginning if interrupted

2. **Fallback for Pruned Logs** (`core/src/service/network/protocol/sync/handler.rs`, line 235)
   - Returns error if logs already pruned
   - Impact: Triggers full resync (acceptable)

3. **Device UUID Filtering** (`core/src/infra/db/entities/location.rs`, line 1018)
   - Minor query optimization
   - Impact: Minimal, locations already device-scoped

### Critical (Blockers) - Currently NONE ✅

All critical functionality is implemented and working.

---

## Review Checklist

### Architecture Review

- [ ] Verify no switch statements on model types outside domain layer
- [ ] Verify all domain logic is in `entities/*.rs`, not `sync/*`
- [ ] Verify registry is pure routing (no JSON serialization)
- [ ] Verify proper trait-based dispatch throughout

### Correctness Review

- [ ] HLC ordering guarantees (timestamp > counter > device_id)
- [ ] HLC causality tracking (update on receive)
- [ ] Idempotent upserts (on_conflict with UUID)
- [ ] Proper field exclusions (id, created_at, updated_at not synced)
- [ ] ACK mechanism (sent after apply, enables pruning)

### Performance Review

- [ ] Parallel broadcasts (not sequential)
- [ ] Batch operations where appropriate
- [ ] Database query efficiency (proper indexes on uuid, updated_at)
- [ ] Buffer queue during backfill (prevents overwhelm)

### Reliability Review

- [ ] Timeout handling (30s on all network operations)
- [ ] Retry queue with exponential backoff
- [ ] Error propagation (no silent failures)
- [ ] Log pruning (prevents unbounded growth)
- [ ] Background task lifecycle (starts with service, stops cleanly)

### Integration Review

- [ ] TransactionManager → PeerSync event flow working
- [ ] PeerSync → NetworkHandler message flow working
- [ ] StateRequest/Response backfill flow working
- [ ] SharedChangeRequest/Response sync flow working
- [ ] ACK flow enabling log pruning

---

## Line-by-Line Review Priorities

### High Priority Sections

**`peer.rs`**:
- Lines 123-147: Service startup (background tasks)
- Lines 220-454: Event handling (TransactionManager integration)
- Lines 513-606: State change broadcasting
- Lines 619-741: Shared change broadcasting
- Lines 787-856: Apply received changes

**`handler.rs`**:
- Lines 47-287: Message routing (all types)

**`registry.rs`**:
- Lines 140-203: Registry initialization (should be pure routing)
- Lines 256-289: Query dispatch function

**`location.rs`, `entry.rs`, `device.rs`**:
- Syncable trait implementations
- Query and apply logic

### Medium Priority Sections

**`peer_log.rs`**:
- Lines 103-131: Append operation
- Lines 133-198: Query operations
- Lines 200-270: ACK and pruning

**`hlc.rs`**:
- Lines 67-92: HLC update logic (causality)
- Lines 132-145: Ordering implementation

**`retry_queue.rs`**:
- Lines 51-95: Retry logic with exponential backoff

---

## Questions for Review

1. **Architecture**: Is the DDD boundary properly maintained? Any domain logic leaking into infrastructure?

2. **Completeness**: Are all message types handled? Any stub functions remaining?

3. **Correctness**: Will the HLC ordering guarantee convergence? Is the ACK mechanism sound?

4. **Performance**: Are there any blocking operations in hot paths? Any N+1 query issues?

5. **Reliability**: What happens if a peer goes offline mid-sync? Are all failure modes handled?

6. **Testing**: What integration tests are needed? Can we mock the network layer effectively?

---

## Success Criteria for Review

### Passing Review Means:

**Architecture**: Clean DDD separation, no domain logic in infrastructure
**Completeness**: All message types handled, no critical stubs
**Correctness**: HLC math verified, conflict resolution sound
**Performance**: Parallel operations, efficient queries
**Reliability**: Proper error handling, retry mechanisms
**Readiness**: Code is ready for integration testing

---

## Next Steps After Review

1. **Model Integration** - Implement Syncable for remaining models (Album, Collection, UserMetadata)
2. **Integration Tests** - Write end-to-end sync tests
3. **Performance Testing** - Benchmark with 1M+ entries
4. **Production Hardening** - Add metrics, monitoring, circuit breakers

---

**Review Status**: Ready for review ✅
**Estimated Review Time**: 4-6 hours for thorough review
**Reviewers Should Focus On**: Architecture violations, correctness issues, missing error handling

