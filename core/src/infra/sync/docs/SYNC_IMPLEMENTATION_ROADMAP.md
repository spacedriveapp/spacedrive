# Sync System Implementation Roadmap

**Status**: In Active Development
**Architecture**: Leaderless Hybrid (State-based + Log-based with HLC)
**Last Updated**: October 9, 2025

---

> **📖 START HERE**: If you're implementing the sync system, read [`SYNC_IMPLEMENTATION_GUIDE.md`](./SYNC_IMPLEMENTATION_GUIDE.md) first.
> That guide synthesizes the architecture from `/docs/core/sync.md` with the status from this roadmap,
> providing clear, actionable guidance for implementation.

---

## 📊 Executive Summary

**Current Grade**: 7.5/10 - Solid foundation with clear architectural vision, mid-migration from leader-based to leaderless architecture.

**Completion Status**: ~75% (25/34 files fully implemented)

---

## 🎯 Critical Path to MVP

### ✅ **Phase 1: Core Infrastructure** (COMPLETE)

- [x] HLC implementation (`hlc.rs`)
- [x] PeerLog for per-device sync.db (`peer_log.rs`)
- [x] Syncable trait (`syncable.rs`)
- [x] Registry for model dispatch (`registry.rs`)
- [x] NetworkTransport trait (`transport.rs`)
- [x] Transaction manager structure (`transaction.rs`)

### 🚧 **Phase 2: Network Integration** (IN PROGRESS - 80%)

- [x] NetworkTransport implementation (`network/transports/sync.rs`)
- [x] PeerSync with broadcast capabilities (`service/sync/peer.rs`)
- [x] SyncMessage types (`network/protocol/sync/messages.rs`)
- [ ] **CRITICAL**: SyncProtocolHandler inbound handling (`network/protocol/sync/handler.rs`)
- [ ] **CRITICAL**: TransactionManager auto-broadcast (`infra/sync/transaction.rs`)

### ⏳ **Phase 3: Model Integration** (IN PROGRESS - 29%)

- [x] Location apply function (`db/entities/location.rs`)
- [x] Tag apply function + Syncable impl (`db/entities/tag.rs`)
- [ ] Entry apply function (`db/entities/entry.rs`)
- [ ] Volume apply function (`db/entities/volume.rs`)
- [ ] Device apply function (`db/entities/device.rs`)
- [ ] Collection/Album apply function (`db/entities/collection.rs`)
- [ ] UserMetadata apply function (`db/entities/user_metadata.rs`)

### ⏳ **Phase 4: End-to-End Testing** (NOT STARTED)

- [ ] Integration tests for state-based sync
- [ ] Integration tests for log-based sync
- [ ] Conflict resolution tests
- [ ] Backfill/catch-up tests
- [ ] Network partition tests

---

## 🔥 Priority 1: Immediate Actions (This Week)

### 1.1 Clean Up Migration Artifacts

**Goal**: Remove confusion from legacy code

**Files to Delete**:
- [ ] `service/sync/applier.rs` - Legacy stub, no longer used
- [ ] `service/network/protocol/sync/transport.rs` - Duplicate, wrong location
- [ ] `service/network/core/sync_transport.rs` - Moved to transports/

**Command**:
```bash
cd /Users/jamespine/Projects/spacedrive/core
rm -f src/service/sync/applier.rs
rm -f src/service/network/protocol/sync/transport.rs
rm -f src/service/network/core/sync_transport.rs
```

**Then update module imports**:
- [ ] Remove `pub mod applier;` from `service/sync/mod.rs`
- [ ] Remove `pub use applier::SyncApplier;` from `service/sync/mod.rs`

---

### 1.2 Implement SyncProtocolHandler (CRITICAL)

**File**: `core/src/service/network/protocol/sync/handler.rs`

**Current State**: Stubbed with warnings

**Required Implementation**:

```rust
impl SyncProtocolHandler {
    /// Handle incoming sync message from peer
    async fn handle_sync_message(
        &self,
        from_device: Uuid,
        message: SyncMessage,
    ) -> Result<Option<SyncMessage>> {
        // Get PeerSync from library
        let library = self.get_library(message.library_id())?;
        let peer_sync = library.sync_service().peer_sync();

        match message {
            SyncMessage::StateChange { .. } => {
                peer_sync.on_state_change_received(change).await?;
                Ok(None) // No response needed
            }
            SyncMessage::SharedChange { entry, .. } => {
                peer_sync.on_shared_change_received(entry).await?;
                Ok(None)
            }
            SyncMessage::StateRequest { .. } => {
                // Query DB and return StateResponse
                let response = self.handle_state_request(request).await?;
                Ok(Some(response))
            }
            SyncMessage::SharedChangeRequest { .. } => {
                // Query PeerLog and return SharedChangeResponse
                let response = self.handle_shared_request(request).await?;
                Ok(Some(response))
            }
            SyncMessage::AckSharedChanges { from_device, up_to_hlc, .. } => {
                peer_sync.on_ack_received(from_device, up_to_hlc).await?;
                Ok(None)
            }
            // ... handle other message types
        }
    }
}
```

**Checklist**:
- [ ] Wire up to `NetworkingService` message router
- [ ] Implement each message type handler
- [ ] Add proper error handling and logging
- [ ] Write unit tests for each message type

**Estimated Effort**: 4-6 hours

---

### 1.3 Fix Broadcast Error Handling

**File**: `core/src/service/sync/peer.rs`

**Issue**: Sequential sends, silent failures, no retry

**Current Code** (lines 186-209):
```rust
for partner_uuid in connected_partners {
    match self.network.send_sync_message(partner_uuid, message.clone()).await {
        Ok(()) => { success_count += 1; }
        Err(e) => {
            error_count += 1;
            warn!("Failed to send"); // Continues
        }
    }
}
```

**Improved Implementation**:
```rust
use futures::future::join_all;

// Parallel sends
let send_futures: Vec<_> = connected_partners
    .iter()
    .map(|&partner| {
        let network = self.network.clone();
        let msg = message.clone();
        async move {
            (partner, network.send_sync_message(partner, msg).await)
        }
    })
    .collect();

let results = join_all(send_futures).await;

// Structured error handling
let (successes, failures): (Vec<_>, Vec<_>) = results
    .into_iter()
    .partition(|(_, result)| result.is_ok());

// Enqueue failures for retry
for (partner_uuid, err) in failures {
    warn!(partner = %partner_uuid, error = %err, "Send failed, will retry");
    // TODO: Add to retry queue
}

// Fail if no one received the message
if successes.is_empty() && !failures.is_empty() {
    return Err(anyhow!("Failed to broadcast to any partner"));
}
```

**Checklist**:
- [ ] Add `futures` dependency to Cargo.toml
- [ ] Implement parallel broadcast in `broadcast_state_change()`
- [ ] Implement parallel broadcast in `broadcast_shared_change()`
- [ ] Add retry queue structure (see Priority 2)

**Estimated Effort**: 2-3 hours

---

### 1.4 Fix Silent Error Handling

**File**: `core/src/service/sync/peer.rs` (lines 156-161)

**Issue**: `.unwrap_or_default()` hides network errors

**Replace**:
```rust
let connected_partners = self
    .network
    .get_connected_sync_partners()
    .await
    .unwrap_or_default(); // ❌ Hides errors
```

**With**:
```rust
let connected_partners = self
    .network
    .get_connected_sync_partners()
    .await
    .map_err(|e| {
        warn!("Failed to get connected partners: {}", e);
        e
    })?;
```

**Checklist**:
- [ ] Fix in `broadcast_state_change()` (line 157)
- [ ] Fix in `broadcast_shared_change()` (line 258)
- [ ] Audit entire codebase for similar patterns

**Estimated Effort**: 30 minutes

---

## 🎯 Priority 2: Short-Term Improvements (This Month)

### 2.1 Implement Retry Queue

**New File**: `core/src/service/sync/retry_queue.rs`

**Purpose**: Automatically retry failed broadcasts with exponential backoff

```rust
use std::collections::VecDeque;
use tokio::sync::Mutex;
use chrono::{DateTime, Utc, Duration};

pub struct RetryQueue {
    queue: Arc<Mutex<VecDeque<FailedSend>>>,
}

struct FailedSend {
    target: Uuid,
    message: SyncMessage,
    attempts: u8,
    next_retry: DateTime<Utc>,
}

impl RetryQueue {
    /// Enqueue a failed send with exponential backoff
    /// Backoff: 1s, 2s, 4s, 8s, 16s, then move to DLQ
    pub async fn enqueue(&self, target: Uuid, message: SyncMessage) {
        let mut queue = self.queue.lock().await;
        queue.push_back(FailedSend {
            target,
            message,
            attempts: 0,
            next_retry: Utc::now() + Duration::seconds(1),
        });
    }

    /// Process retries (called by background task)
    pub async fn process_retries(&self, network: Arc<dyn NetworkTransport>) {
        let mut queue = self.queue.lock().await;
        let now = Utc::now();

        // Try to send all messages due for retry
        let mut i = 0;
        while i < queue.len() {
            let failed = &mut queue[i];

            if failed.next_retry > now {
                i += 1;
                continue;
            }

            match network.send_sync_message(failed.target, failed.message.clone()).await {
                Ok(()) => {
                    // Success! Remove from queue
                    queue.remove(i);
                }
                Err(e) => {
                    failed.attempts += 1;

                    if failed.attempts >= 5 {
                        // Move to DLQ
                        warn!(target = %failed.target, "Max retries exceeded, moving to DLQ");
                        // TODO: Persist to dead letter queue
                        queue.remove(i);
                    } else {
                        // Exponential backoff: 2^attempts seconds
                        let backoff_secs = 1 << failed.attempts;
                        failed.next_retry = Utc::now() + Duration::seconds(backoff_secs);
                        i += 1;
                    }
                }
            }
        }
    }
}
```

**Integration Points**:
- [ ] Add `retry_queue: Arc<RetryQueue>` to `PeerSync`
- [ ] Call `retry_queue.enqueue()` on broadcast failures
- [ ] Add background task in `SyncService::run_sync_loop()` to call `process_retries()`

**Estimated Effort**: 4-6 hours

---

### 2.2 Add Message Envelope Pattern

**File**: `core/src/service/network/protocol/sync/messages.rs`

**Issue**: 11 message types with duplicated fields (library_id, device_id, timestamp)

**Refactor to Envelope Pattern**:

```rust
/// Envelope for all sync messages
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncEnvelope {
    /// Protocol version (for future compatibility)
    pub version: u8,

    /// Library this message pertains to
    pub library_id: Uuid,

    /// Sending device ID
    pub device_id: Uuid,

    /// Message timestamp
    pub timestamp: DateTime<Utc>,

    /// Actual payload
    pub payload: SyncPayload,
}

/// Sync message payloads (simplified)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SyncPayload {
    // State-based (device-owned)
    StateChange {
        model_type: String,
        record_uuid: Uuid,
        data: serde_json::Value,
    },
    StateBatch {
        model_type: String,
        records: Vec<StateRecord>,
    },

    // Log-based (shared)
    SharedChange(SharedChangeEntry),
    SharedChangeBatch(Vec<SharedChangeEntry>),

    // Requests
    StateRequest(StateRequestParams),
    SharedChangeRequest(SharedChangeRequestParams),

    // Responses
    StateResponse(StateResponseData),
    SharedChangeResponse(SharedChangeResponseData),

    // Control
    AckSharedChanges { up_to_hlc: HLC },
    Heartbeat {
        state_watermark: Option<DateTime<Utc>>,
        shared_watermark: Option<HLC>,
    },
    Error { message: String },
}

impl SyncEnvelope {
    pub fn new(library_id: Uuid, device_id: Uuid, payload: SyncPayload) -> Self {
        Self {
            version: 1,
            library_id,
            device_id,
            timestamp: Utc::now(),
            payload,
        }
    }
}
```

**Migration Strategy**:
1. [ ] Add `SyncEnvelope` alongside existing `SyncMessage`
2. [ ] Update serialization to wrap/unwrap envelope
3. [ ] Support both formats during transition (check version field)
4. [ ] Migrate all senders to use envelope
5. [ ] Remove old `SyncMessage` enum

**Estimated Effort**: 6-8 hours

---

### 2.3 Complete Model Apply Functions

**Status**: 2/7 models implemented

**Remaining Models**:

#### Entry (`core/src/infra/db/entities/entry.rs`)
```rust
impl Model {
    pub async fn apply_state_change(
        data: serde_json::Value,
        db: &DatabaseConnection,
    ) -> Result<(), DbErr> {
        let entry: Self = serde_json::from_value(data)
            .map_err(|e| DbErr::Custom(format!("Deserialization failed: {}", e)))?;

        // Upsert entry
        let active = ActiveModel {
            uuid: Set(entry.uuid),
            location_id: Set(entry.location_id),
            device_id: Set(entry.device_id),
            path: Set(entry.path),
            // ... other fields
        };

        active.insert(db).await?;
        Ok(())
    }
}
```

**Checklist**:
- [ ] Implement `Entry::apply_state_change()`
- [ ] Implement `Volume::apply_state_change()`
- [ ] Implement `Device::apply_state_change()`
- [ ] Implement `Collection::apply_shared_change()` (with conflict resolution)
- [ ] Implement `UserMetadata::apply_mixed()` (hybrid strategy)
- [ ] Register all models in `registry.rs`
- [ ] Add tests for each apply function

**Estimated Effort**: 8-12 hours (2-3 hours per model)

---

### 2.4 Write Integration Tests

**New File**: `core/tests/sync_integration_test.rs`

**Test Scenarios**:

```rust
#[tokio::test]
async fn test_state_based_sync_flow() {
    // Setup: Two devices with mock network
    let device_a = create_test_device("A").await;
    let device_b = create_test_device("B").await;
    let network = Arc::new(MockNetworkTransport::new());

    // 1. Device A creates a location
    let location = create_location(device_a.id, "/test");
    device_a.sync.broadcast_state_change(location.into()).await.unwrap();

    // 2. Verify message was sent
    let messages = network.get_sent_messages();
    assert_eq!(messages.len(), 1);

    // 3. Device B receives and applies
    let (target, msg) = &messages[0];
    assert_eq!(*target, device_b.id);
    device_b.sync.on_state_change_received(msg.into()).await.unwrap();

    // 4. Verify location exists on device B
    let location_b = device_b.db.find_location(location.uuid).await.unwrap();
    assert_eq!(location_b.path, "/test");
}

#[tokio::test]
async fn test_log_based_sync_with_conflict() {
    // Setup: Two devices editing same tag
    let device_a = create_test_device("A").await;
    let device_b = create_test_device("B").await;

    // 1. Both devices create same tag (deterministic UUID)
    let tag_uuid = deterministic_system_tag_uuid("Important");

    // Device A: name="Important", color="red"
    device_a.create_tag(tag_uuid, "Important", "red").await;

    // Device B: name="Important", color="blue" (concurrent)
    device_b.create_tag(tag_uuid, "Important", "blue").await;

    // 2. Exchange changes
    let change_a = device_a.peer_log.get_latest().await.unwrap();
    let change_b = device_b.peer_log.get_latest().await.unwrap();

    device_a.sync.on_shared_change_received(change_b).await.unwrap();
    device_b.sync.on_shared_change_received(change_a).await.unwrap();

    // 3. Both devices should converge (HLC determines winner)
    let tag_a = device_a.db.find_tag(tag_uuid).await.unwrap();
    let tag_b = device_b.db.find_tag(tag_uuid).await.unwrap();

    assert_eq!(tag_a.color, tag_b.color); // Convergence!
}

#[tokio::test]
async fn test_backfill_with_buffering() {
    // Test that changes during backfill are buffered and applied later
    todo!("Implement backfill test");
}

#[tokio::test]
async fn test_network_partition_recovery() {
    // Test that devices sync correctly after network partition
    todo!("Implement partition recovery test");
}
```

**Checklist**:
- [ ] Set up test infrastructure (mock network, test databases)
- [ ] Write state-based sync test
- [ ] Write log-based sync test with conflicts
- [ ] Write backfill test
- [ ] Write partition recovery test
- [ ] Run tests in CI

**Estimated Effort**: 12-16 hours

---

## 🏗️ Priority 3: Architectural Refactors (This Quarter)

### 3.1 Eliminate Circular Dependency

**Issue**: Library → SyncService → needs NetworkingService, NetworkingService → needs Library

**Current Workaround**: `Arc<dyn NetworkTransport>` trait abstraction

**Better Solution**: Extract shared context

**New File**: `core/src/library/context.rs`

```rust
/// Shared context for all library services
///
/// Breaks circular dependencies by extracting shared state into a context object
/// that both SyncService and NetworkingService depend on.
pub struct LibraryContext {
    /// Library ID
    pub id: Uuid,

    /// This device's ID
    pub device_id: Uuid,

    /// Database connection
    pub db: Arc<DatabaseConnection>,

    /// Event bus for cross-cutting events
    pub event_bus: Arc<EventBus>,

    /// Device registry (UUID ↔ NodeId mapping)
    pub device_registry: Arc<RwLock<DeviceRegistry>>,

    /// Library path (for sync.db location)
    pub path: PathBuf,
}

impl LibraryContext {
    pub fn new(library: &Library, device_id: Uuid) -> Self {
        Self {
            id: library.id(),
            device_id,
            db: library.db().conn().clone(),
            event_bus: library.event_bus().clone(),
            device_registry: Arc::new(RwLock::new(DeviceRegistry::new())),
            path: library.path().to_path_buf(),
        }
    }
}
```

**Refactored Services**:

```rust
// SyncService no longer needs NetworkTransport trait
pub struct SyncService {
    context: Arc<LibraryContext>,
    network: Arc<NetworkingService>, // Direct reference
    peer_sync: Arc<PeerSync>,
}

// NetworkingService uses context
pub struct NetworkingService {
    context: Arc<LibraryContext>,
    endpoint: Option<Endpoint>,
}

impl NetworkingService {
    /// Send sync message using context's device registry
    pub async fn send_sync_message(
        &self,
        target_device: Uuid,
        message: SyncMessage,
    ) -> Result<()> {
        let node_id = self.context.device_registry
            .read()
            .await
            .get_node_id_for_device(target_device)?;

        // Send via endpoint...
    }
}
```

**Migration Steps**:
1. [ ] Create `LibraryContext` struct
2. [ ] Update `Library` to create and store context
3. [ ] Refactor `NetworkingService` to use context
4. [ ] Remove `NetworkTransport` trait
5. [ ] Update `SyncService` and `PeerSync` to use direct `NetworkingService` reference
6. [ ] Update all initialization code
7. [ ] Run full test suite

**Estimated Effort**: 16-24 hours

---

### 3.2 Simplify Registry Pattern

**Issue**: Function pointer registry is complex and hard to debug

**Current Implementation**: `StateApplyFn` and `SharedApplyFn` function pointer types

**Better Implementation**: Trait-based with auto-registration

```rust
// New trait
#[async_trait]
pub trait SyncableModel: Send + Sync + 'static {
    const MODEL_TYPE: &'static str;
    const TABLE_NAME: &'static str;
    const IS_DEVICE_OWNED: bool;

    async fn apply_state_change(
        data: serde_json::Value,
        db: &DatabaseConnection,
    ) -> Result<(), DbErr> {
        Err(DbErr::Custom("Not a device-owned model".to_string()))
    }

    async fn apply_shared_change(
        entry: SharedChangeEntry,
        db: &DatabaseConnection,
    ) -> Result<(), DbErr> {
        Err(DbErr::Custom("Not a shared model".to_string()))
    }
}

// Models implement the trait
impl SyncableModel for location::Model {
    const MODEL_TYPE: &'static str = "location";
    const TABLE_NAME: &'static str = "locations";
    const IS_DEVICE_OWNED: bool = true;

    async fn apply_state_change(
        data: serde_json::Value,
        db: &DatabaseConnection,
    ) -> Result<(), DbErr> {
        // Implementation...
    }
}

// Auto-registration using inventory
pub struct RegisteredModel {
    pub model_type: &'static str,
    pub table_name: &'static str,
    pub is_device_owned: bool,
    pub applier: &'static dyn SyncableModelApplier,
}

inventory::collect!(RegisteredModel);

// Register models
inventory::submit! {
    RegisteredModel {
        model_type: "location",
        table_name: "locations",
        is_device_owned: true,
        applier: &LocationApplier,
    }
}
```

**Migration Steps**:
1. [ ] Define `SyncableModel` trait
2. [ ] Implement trait for existing models (location, tag)
3. [ ] Set up inventory-based registration
4. [ ] Add macro to simplify registration
5. [ ] Migrate apply functions to use new registry
6. [ ] Remove old function pointer registry
7. [ ] Update all call sites

**Estimated Effort**: 12-16 hours

---

### 3.3 Add Observability Infrastructure

**Goal**: Production-ready monitoring and debugging

**Components**:

#### Metrics (`core/src/service/sync/metrics.rs`)
```rust
use prometheus::{Counter, Histogram, IntGauge};

pub struct SyncMetrics {
    // Counters
    pub state_changes_sent: Counter,
    pub state_changes_received: Counter,
    pub shared_changes_sent: Counter,
    pub shared_changes_received: Counter,
    pub broadcast_failures: Counter,

    // Gauges
    pub connected_partners: IntGauge,
    pub buffer_queue_size: IntGauge,
    pub retry_queue_size: IntGauge,

    // Histograms
    pub broadcast_duration: Histogram,
    pub apply_duration: Histogram,
}

impl SyncMetrics {
    pub fn new() -> Self {
        Self {
            state_changes_sent: Counter::new("sync_state_changes_sent", "State changes sent")
                .unwrap(),
            // ... register all metrics
        }
    }

    pub fn record_broadcast(&self, duration: Duration, success: bool) {
        self.broadcast_duration.observe(duration.as_secs_f64());
        if !success {
            self.broadcast_failures.inc();
        }
    }
}
```

#### Tracing Spans
```rust
use tracing::instrument;

#[instrument(skip(self), fields(library_id = %self.library_id, model_type = %change.model_type))]
pub async fn broadcast_state_change(&self, change: StateChangeMessage) -> Result<()> {
    let span = tracing::info_span!("broadcast", partner_count = connected_partners.len());
    let _enter = span.enter();

    // Broadcast logic...
}
```

**Checklist**:
- [ ] Add `prometheus` dependency
- [ ] Create `SyncMetrics` struct
- [ ] Instrument all critical paths
- [ ] Add tracing spans with context
- [ ] Create Grafana dashboard
- [ ] Add health check endpoint

**Estimated Effort**: 8-12 hours

---

## 📋 Current Status Matrix

| Component | Status | Priority | Effort | Owner |
|-----------|--------|----------|--------|-------|
| **Core Infrastructure** |
| HLC | ✅ Complete | - | - | - |
| PeerLog | ✅ Complete | - | - | - |
| NetworkTransport trait | ✅ Complete | - | - | - |
| TransactionManager | 🚧 Stubbed | P1 | 4h | TBD |
| **Network Layer** |
| NetworkTransport impl | ✅ Complete | - | - | - |
| SyncProtocolHandler | ❌ Stubbed | P1 (CRITICAL) | 6h | TBD |
| Message types | ✅ Complete | - | - | - |
| Envelope pattern | ❌ Not started | P2 | 8h | TBD |
| **Sync Service** |
| PeerSync | ✅ Complete | - | - | - |
| Broadcast (sequential) | ✅ Works | P1 | 2h | TBD |
| Broadcast (parallel) | ❌ Not started | P1 | 2h | TBD |
| Retry queue | ❌ Not started | P2 | 6h | TBD |
| **Models** |
| Location | ✅ Complete | - | - | - |
| Tag | ✅ Complete | - | - | - |
| Entry | ❌ Not started | P2 | 2h | TBD |
| Volume | ❌ Not started | P2 | 2h | TBD |
| Device | ❌ Not started | P2 | 2h | TBD |
| Collection | ❌ Not started | P2 | 3h | TBD |
| UserMetadata | ❌ Not started | P2 | 3h | TBD |
| **Testing** |
| Unit tests | 🚧 Partial | P2 | 8h | TBD |
| Integration tests | ❌ Not started | P2 | 16h | TBD |
| Performance tests | ❌ Not started | P3 | 12h | TBD |
| **Architecture** |
| Circular dependency | 🚧 Workaround | P3 | 24h | TBD |
| Registry pattern | 🚧 Functional | P3 | 16h | TBD |
| Observability | ❌ Not started | P3 | 12h | TBD |

**Legend**:
- ✅ Complete
- 🚧 In progress / Partial
- ❌ Not started
- P1 = Priority 1 (This week)
- P2 = Priority 2 (This month)
- P3 = Priority 3 (This quarter)

---

## 🐛 Known Issues

### Critical
1. **SyncProtocolHandler is stubbed** - Incoming messages are not processed
2. **TransactionManager doesn't auto-broadcast** - Changes don't trigger sync
3. **Sequential broadcasts** - Slow for many peers
4. **Silent error handling** - `.unwrap_or_default()` hides network issues

### Major
5. **No retry mechanism** - Failed broadcasts are lost
6. **No conflict resolution UI** - Users can't resolve conflicts manually
7. **Only 2/7 models have apply functions** - Most models can't sync
8. **No integration tests** - Untested end-to-end flows

### Minor
9. **Circular dependency workaround** - Adds unnecessary complexity
10. **Function pointer registry** - Hard to debug
11. **No observability** - Can't monitor sync health in production
12. **Clone overuse** - Potential performance issues

---

## 📚 Architecture Decisions

### ADR-001: Hybrid Sync Model
**Decision**: Use state-based sync for device-owned data, log-based with HLC for shared resources

**Rationale**:
- State-based is simpler and more efficient for data that can't conflict
- Log-based provides proper conflict resolution for shared data
- Hybrid approach gives us best of both worlds

**Status**: Implemented

### ADR-002: NetworkTransport Trait
**Decision**: Use trait abstraction to break circular dependency

**Rationale**:
- Library → Sync → Network circular dependency
- Trait allows dependency injection
- Enables testing with mocks

**Status**: Implemented (may be refactored in P3)

### ADR-003: Leaderless Architecture
**Decision**: All devices are peers, no leader election

**Rationale**:
- Simpler than leader-based approach
- More resilient (no single point of failure)
- Better for offline-first usage

**Status**: Implemented

### ADR-004: Per-Device Sync.db
**Decision**: Each device has its own sync.db for shared changes

**Rationale**:
- Allows independent pruning
- Clear ownership of log entries
- Simplifies ACK tracking

**Status**: Implemented

---

## 🎓 Learning Resources

### Distributed Sync Papers
- [Hybrid Logical Clocks](https://cse.buffalo.edu/tech-reports/2014-04.pdf)
- [Conflict-free Replicated Data Types (CRDTs)](https://hal.inria.fr/inria-00555588/document)
- [Operational Transformation](http://www.codecommit.com/blog/java/understanding-and-applying-operational-transformation)

### Related Projects
- [Automerge](https://github.com/automerge/automerge) - CRDT library
- [Yrs](https://github.com/y-crdt/y-crdt) - Rust CRDT implementation
- [Syncthing](https://github.com/syncthing/syncthing) - File sync reference

### Spacedrive Documentation
- [Daemon Architecture](/Users/jamespine/Projects/spacedrive/docs/core/daemon.md)
- [AGENTS.md](/Users/jamespine/Projects/spacedrive/core/AGENTS.md)
- [Sync Design Docs](/Users/jamespine/Projects/spacedrive/docs/core/)

---

## 📞 Questions / Discussion Points

1. **Should we support protocol versioning from day one?**
   - Pros: Future-proof, easier upgrades
   - Cons: More complexity upfront
   - **Recommendation**: Yes, add envelope pattern now (P2)

2. **How should we handle conflicts in the UI?**
   - Option A: Always auto-resolve using HLC
   - Option B: Present conflicts to user for manual resolution
   - **Recommendation**: A for MVP, B for future

3. **Should we compress messages?**
   - Large batches (1000+ entries) could benefit from zstd
   - Adds complexity and CPU overhead
   - **Recommendation**: Not for MVP, revisit in P3

4. **Should we encrypt sync messages?**
   - End-to-end encryption for privacy
   - Per-library keys
   - **Recommendation**: Not for MVP, but design with encryption in mind

---

## 📅 Timeline

### Week 1 (Current)
- [ ] Clean up legacy files
- [ ] Implement SyncProtocolHandler
- [ ] Fix broadcast error handling
- [ ] Fix silent error handling

**Goal**: Basic end-to-end sync working

### Week 2-4
- [ ] Implement retry queue
- [ ] Complete all model apply functions
- [ ] Write integration tests
- [ ] Add message envelope pattern

**Goal**: Production-ready sync

### Month 2-3
- [ ] Refactor circular dependency
- [ ] Simplify registry pattern
- [ ] Add observability
- [ ] Performance testing

**Goal**: Clean, maintainable architecture

---

## 🎯 Success Metrics

### MVP (End of Week 4)
- [ ] All message types handled correctly
- [ ] 7/7 models can sync
- [ ] Integration tests pass
- [ ] No data loss in normal operation
- [ ] Basic error handling and logging

### Production-Ready (End of Month 3)
- [ ] Zero data corruption issues
- [ ] < 1% message loss rate (with retry)
- [ ] < 100ms broadcast latency (10 peers)
- [ ] < 5s sync time for 1000 changes
- [ ] Comprehensive test coverage (>70%)
- [ ] Monitoring dashboards

---

## 📝 Notes

- Keep this document updated as work progresses
- Link to relevant PRs and commits
- Document any architectural changes or decisions
- Add new issues as they're discovered

**Last Updated**: October 9, 2025

