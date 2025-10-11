# Sync Implementation Guide

**Purpose**: Primary guide for implementing the Spacedrive sync system
**Status**: Active Development
**Last Updated**: October 9, 2025

---

## Executive Summary

The Spacedrive sync system is a well-architected, hybrid synchronization solution combining state-based and log-based approaches. The architectural design is comprehensive and implementation-ready. This guide synthesizes the detailed architecture from [`/docs/core/sync.md`](/Users/jamespine/Projects/spacedrive/docs/core/sync.md) with the current implementation status from [`SYNC_IMPLEMENTATION_ROADMAP.md`](./SYNC_IMPLEMENTATION_ROADMAP.md).

**Current Risk Profile**: LOW - The task is primarily one of executing a well-defined plan rather than solving architectural unknowns.

**Key Documents**:
- **Architecture Reference**: [`/docs/core/sync.md`](/Users/jamespine/Projects/spacedrive/docs/core/sync.md) - Your primary guide for "how to build"
- **Implementation Status**: [`SYNC_IMPLEMENTATION_ROADMAP.md`](./SYNC_IMPLEMENTATION_ROADMAP.md) - Current progress and priorities
- **File Organization**: [`FILE_ORGANIZATION.md`](./FILE_ORGANIZATION.md) - Where everything lives

---

## Core Architectural Principles

These principles from `sync.md` are non-negotiable. All implementation work must adhere to them.

### 1. All Data Modifications Must Go Through TransactionManager

**The Rule**: Every create, update, or delete operation on library data **must** be routed through the `TransactionManager`. Direct database writes are not permitted.

**Why**: This is the cornerstone that guarantees all changes are captured for synchronization.

**Implementation Pattern**:

```rust
// CORRECT: Use TransactionManager
let mut tx = library.transaction_manager().begin().await?;
tx.commit_device_owned("location", location_uuid, location_json).await?;

// WRONG: Direct database write
location::Entity::insert(active_model).exec(db).await?;
```

**Where This Applies**:
- Job System (file operations that create/update entries)
- Operations in `src/ops/` (all CRUD actions)
- File Watcher (entry creation/updates)

**Action Required**: Audit all database write operations and migrate them to use `TransactionManager`.

---

### 2. Use the Correct Sync Strategy Per Model

The architecture uses two distinct synchronization strategies. You must implement the correct one for each model.

#### Strategy A: State-Based Sync (Device-Owned Data)

**Used For**: Data that belongs to a specific device and cannot have conflicts between devices.

**Models**: `Location`, `Entry`, `Volume`, `Device`

**How It Works**:
1. Device makes a change locally
2. Updates its local database immediately
3. Broadcasts the new state to all connected peers
4. Peers receive and apply the state directly (no conflict resolution needed)

**Implementation**:
```rust
impl Syncable for location::Model {
    fn sync_strategy() -> SyncStrategy {
        SyncStrategy::StateBased
    }

    async fn apply_state_change(
        data: serde_json::Value,
        db: &DatabaseConnection,
    ) -> Result<(), DbErr> {
        let location: Self = serde_json::from_value(data)?;

        // Simple upsert - last write wins
        let active = ActiveModel {
            uuid: Set(location.uuid),
            device_id: Set(location.device_id),
            path: Set(location.path),
            // ... other fields
        };

        active.insert(db).await?;
        Ok(())
    }
}
```

#### Strategy B: Log-Based Sync (Shared Resources)

**Used For**: Data that can be edited by multiple devices concurrently and needs conflict resolution.

**Models**: `Tag`, `Album`, `Collection`, `UserMetadata`

**How It Works**:
1. Device makes a change locally
2. Generates an HLC timestamp
3. Writes change to local `sync.db` (append-only log)
4. Updates main database
5. Broadcasts the log entry to all connected peers
6. Peers receive, compare HLC timestamps, and resolve conflicts

**Implementation**:
```rust
impl Syncable for tag::Model {
    fn sync_strategy() -> SyncStrategy {
        SyncStrategy::LogBased
    }

    async fn apply_shared_change(
        entry: SharedChangeEntry,
        db: &DatabaseConnection,
    ) -> Result<(), DbErr> {
        let new_tag: Self = serde_json::from_value(entry.data)?;

        // Load current state
        let current = tag::Entity::find_by_id(new_tag.uuid)
            .one(db)
            .await?;

        match current {
            None => {
                // New record, just insert
                new_tag.into_active_model().insert(db).await?;
            }
            Some(existing) => {
                // Conflict! Use HLC to determine winner
                if entry.hlc > existing.last_modified_hlc {
                    // Incoming change is newer, apply it
                    new_tag.into_active_model().update(db).await?;
                } else {
                    // Local change is newer, ignore incoming
                    debug!("Ignoring older change for tag {}", new_tag.uuid);
                }
            }
        }

        Ok(())
    }
}
```

**Reference Table** (from `sync.md`):

| Model | Strategy | Reason |
|-------|----------|--------|
| Location | State-Based | Device-owned, no conflicts |
| Entry | State-Based | Device-owned, no conflicts |
| Volume | State-Based | Device-owned, no conflicts |
| Device | State-Based | Device-owned, no conflicts |
| Tag | Log-Based | Shared, needs conflict resolution |
| Album | Log-Based | Shared, needs conflict resolution |
| Collection | Log-Based | Shared, needs conflict resolution |
| UserMetadata | Hybrid | Per-device + shared fields |

**Action Required**: Implement `Syncable` trait for all models using the correct strategy.

---

### 3. Implement Proper Error Handling and Recovery

The `sync.md` document specifies recovery procedures for common failure scenarios. You must implement these.

#### Network Timeout Handling

```rust
pub async fn send_sync_message(
    &self,
    target: Uuid,
    message: SyncMessage,
) -> Result<()> {
    match tokio::time::timeout(
        Duration::from_secs(30),
        self.do_send(target, message.clone())
    ).await {
        Ok(Ok(())) => Ok(()),
        Ok(Err(e)) => {
            // Network error - enqueue for retry
            self.retry_queue.enqueue(target, message).await;
            Err(e)
        }
        Err(_) => {
            // Timeout - enqueue for retry
            warn!("Send timeout for {}, will retry", target);
            self.retry_queue.enqueue(target, message).await;
            Err(anyhow!("Send timeout"))
        }
    }
}
```

#### Database Constraint Violations

```rust
async fn apply_change(&self, change: SyncMessage) -> Result<()> {
    match self.do_apply(change.clone()).await {
        Ok(()) => Ok(()),
        Err(DbErr::RecordNotFound(_)) => {
            // Missing foreign key - request dependency
            self.request_missing_record(change.dependency_id).await?;
            // Buffer this change to retry after dependency arrives
            self.buffer_queue.push(change).await;
            Ok(())
        }
        Err(e) => Err(e.into()),
    }
}
```

#### Partial Sync Resume

```rust
pub async fn resume_sync(&self, peer_id: Uuid) -> Result<()> {
    // Load last successfully processed HLC from local state
    let watermark = self.get_watermark(peer_id).await?;

    // Request all changes since watermark
    self.request_shared_changes(peer_id, watermark).await?;

    Ok(())
}
```

**Action Required**: Implement these recovery patterns in `PeerSync` and `SyncProtocolHandler`.

---

### 4. Build Comprehensive Tests

The `sync.md` document provides test templates. Use these as your starting point.

#### Required Integration Tests

1. **Two-Peer State-Based Sync**:
   - Device A creates a location
   - Device B receives and applies it
   - Verify both databases match

2. **Two-Peer Log-Based Sync with Conflict**:
   - Device A and B both edit the same tag concurrently
   - Exchange changes
   - Verify both converge to the same state (HLC winner)

3. **Network Partition Recovery**:
   - Device A and B both make changes while disconnected
   - Reconnect
   - Verify full synchronization

4. **Partial Sync Resume**:
   - Start syncing 1000 changes
   - Disconnect at change 500
   - Reconnect
   - Verify resume from change 501

**Test Infrastructure** (from `sync.md`):

```rust
// Mock network for testing
pub struct MockNetworkTransport {
    sent_messages: Arc<Mutex<Vec<(Uuid, SyncMessage)>>>,
}

impl NetworkTransport for MockNetworkTransport {
    async fn send_sync_message(
        &self,
        target: Uuid,
        message: SyncMessage,
    ) -> Result<()> {
        self.sent_messages.lock().await.push((target, message));
        Ok(())
    }
}

// Test helper to create isolated test device
async fn create_test_device(name: &str) -> TestDevice {
    let db = create_test_database().await;
    let sync_db_path = format!("/tmp/sync_test_{}.db", name);
    let peer_log = PeerLog::new(&sync_db_path).await.unwrap();

    TestDevice {
        id: Uuid::new_v4(),
        name: name.to_string(),
        db,
        peer_log,
        sync: /* ... */,
    }
}
```

**Action Required**: Write integration tests in `core/tests/sync_integration_test.rs` using the patterns from `sync.md`.

---

## Current Implementation Status

See [`SYNC_IMPLEMENTATION_ROADMAP.md`](./SYNC_IMPLEMENTATION_ROADMAP.md) for detailed status. Key phases:

- **Phase 1: Core Infrastructure** - COMPLETE
- **Phase 2: Network Integration** - IN PROGRESS (80%)
- **Phase 3: Model Integration** - IN PROGRESS (29%)
- **Phase 4: End-to-End Testing** - NOT STARTED

---

## Priority 1: Critical Path Items

These must be completed for the sync system to function end-to-end.

### 1. Implement SyncProtocolHandler (CRITICAL)

**File**: `core/src/service/network/protocol/sync/handler.rs`

**Current State**: Stubbed with warnings

**What It Must Do**:
- Receive incoming `SyncMessage` from network layer
- Route to appropriate handler based on message type
- Call `PeerSync` methods to process the message
- Return response messages when needed

**Implementation Template**:

```rust
impl SyncProtocolHandler {
    pub async fn handle_sync_message(
        &self,
        from_device: Uuid,
        message: SyncMessage,
    ) -> Result<Option<SyncMessage>> {
        let library = self.get_library(message.library_id())?;
        let peer_sync = library.sync_service().peer_sync();

        match message {
            SyncMessage::StateChange { model_type, record_uuid, data, .. } => {
                peer_sync.on_state_change_received(
                    model_type,
                    record_uuid,
                    data
                ).await?;
                Ok(None) // No response needed
            }

            SyncMessage::SharedChange { entry, .. } => {
                peer_sync.on_shared_change_received(entry).await?;
                Ok(None)
            }

            SyncMessage::StateRequest { model_type, record_uuid, .. } => {
                let response = self.handle_state_request(
                    model_type,
                    record_uuid
                ).await?;
                Ok(Some(SyncMessage::StateResponse { /* ... */ }))
            }

            SyncMessage::SharedChangeRequest { from_hlc, .. } => {
                let entries = peer_sync.get_changes_since(from_hlc).await?;
                Ok(Some(SyncMessage::SharedChangeResponse { entries }))
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

**Estimated Effort**: 4-6 hours

---

### 2. Enable Auto-Broadcast in TransactionManager

**File**: `core/src/infra/sync/transaction.rs`

**Current State**: Transactions commit but don't trigger sync

**What It Must Do**:
- After successful database commit, automatically broadcast changes
- Use `PeerSync::broadcast_state_change()` for device-owned data
- Use `PeerSync::broadcast_shared_change()` for shared data

**Implementation**:

```rust
impl TransactionManager {
    pub async fn commit_device_owned(
        &mut self,
        model_type: &str,
        record_uuid: Uuid,
        data: serde_json::Value,
    ) -> Result<()> {
        // 1. Commit to database
        self.commit_to_db(model_type, record_uuid, &data).await?;

        // 2. Broadcast to peers
        self.peer_sync.broadcast_state_change(StateChangeMessage {
            model_type: model_type.to_string(),
            record_uuid,
            data,
        }).await?;

        Ok(())
    }

    pub async fn commit_shared(
        &mut self,
        model_type: &str,
        record_uuid: Uuid,
        data: serde_json::Value,
    ) -> Result<()> {
        // 1. Generate HLC timestamp
        let hlc = self.hlc.generate_timestamp().await;

        // 2. Write to sync.db (append-only log)
        let entry = SharedChangeEntry {
            hlc,
            device_id: self.device_id,
            model_type: model_type.to_string(),
            record_uuid,
            data: data.clone(),
            timestamp: Utc::now(),
        };

        self.peer_log.append(entry.clone()).await?;

        // 3. Commit to main database
        self.commit_to_db(model_type, record_uuid, &data).await?;

        // 4. Broadcast to peers
        self.peer_sync.broadcast_shared_change(entry).await?;

        Ok(())
    }
}
```

**Estimated Effort**: 3-4 hours

---

### 3. Fix Broadcast Error Handling

**File**: `core/src/service/sync/peer.rs` (lines 186-209)

**Current Problem**: Sequential sends, silent failures with `.unwrap_or_default()`

**Required Changes**:

1. **Use parallel sends** (not sequential):
```rust
use futures::future::join_all;

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
```

2. **Handle errors properly** (not `.unwrap_or_default()`):
```rust
let connected_partners = self
    .network
    .get_connected_sync_partners()
    .await
    .map_err(|e| {
        warn!("Failed to get connected partners: {}", e);
        e
    })?; // Propagate error instead of hiding it
```

3. **Enqueue failures for retry**:
```rust
for (partner_uuid, err) in failures {
    warn!(partner = %partner_uuid, error = %err, "Send failed, will retry");
    self.retry_queue.enqueue(partner_uuid, message.clone()).await;
}
```

**Estimated Effort**: 2-3 hours

---

## Implementation Checklist

Use this checklist to track progress on the critical path.

### Network Integration
- [ ] Implement `SyncProtocolHandler::handle_sync_message()`
- [ ] Handle each message type (StateChange, SharedChange, etc.)
- [ ] Wire up to `NetworkingService` message router
- [ ] Add proper error handling and logging
- [ ] Write unit tests for each message type

### Transaction Manager
- [ ] Implement auto-broadcast in `commit_device_owned()`
- [ ] Implement auto-broadcast in `commit_shared()`
- [ ] Generate HLC timestamps for shared changes
- [ ] Write to `sync.db` before broadcasting
- [ ] Add transaction rollback on broadcast failure

### Broadcast Improvements
- [ ] Convert to parallel sends using `join_all`
- [ ] Remove `.unwrap_or_default()` error hiding
- [ ] Add retry queue integration
- [ ] Add timeout handling
- [ ] Add metrics collection

### Model Integration
- [ ] Implement `Entry::apply_state_change()`
- [ ] Implement `Volume::apply_state_change()`
- [ ] Implement `Device::apply_state_change()`
- [ ] Implement `Album::apply_shared_change()`
- [ ] Implement `Collection::apply_shared_change()`
- [ ] Implement `UserMetadata::apply_mixed()`
- [ ] Register all models in `registry.rs`

### Testing
- [ ] Set up test infrastructure (mock network, test DB)
- [ ] Write two-peer state-based sync test
- [ ] Write two-peer log-based sync test with conflict
- [ ] Write network partition recovery test
- [ ] Write partial sync resume test
- [ ] Run tests in CI

---

## Common Pitfalls to Avoid

### 1. Direct Database Writes
**Don't**: Write directly to the database in operations/jobs
**Do**: Always use `TransactionManager`

### 2. Wrong Sync Strategy
**Don't**: Use log-based sync for device-owned data
**Do**: Check the model classification table in this guide

### 3. Silent Error Handling
**Don't**: Use `.unwrap_or_default()` or similar patterns
**Do**: Properly propagate errors and log them

### 4. Missing HLC Timestamps
**Don't**: Forget to generate HLC for shared changes
**Do**: Always generate HLC before writing to `sync.db`

### 5. Ignoring Conflicts
**Don't**: Last-write-wins for shared data
**Do**: Use HLC comparison for conflict resolution

---

## Questions and Support

If you encounter architectural questions during implementation:

1. **First**, check [`/docs/core/sync.md`](/Users/jamespine/Projects/spacedrive/docs/core/sync.md) for detailed patterns
2. **Second**, check this guide for implementation examples
3. **Third**, check the [`SYNC_IMPLEMENTATION_ROADMAP.md`](./SYNC_IMPLEMENTATION_ROADMAP.md) for status updates
4. **Finally**, document any unresolved questions and architectural decisions

---

## Success Criteria

### MVP (End-to-End Sync Working)
- [ ] All message types handled correctly
- [ ] At least 2 models fully syncing (one state-based, one log-based)
- [ ] Basic integration test passing
- [ ] No data loss in normal operation

### Production Ready
- [ ] All 7 models syncing correctly
- [ ] Comprehensive integration tests passing
- [ ] Error handling and retry mechanisms working
- [ ] < 1% message loss rate
- [ ] < 100ms broadcast latency for 10 peers

---

**Remember**: The architecture is solid. Focus on execution, not redesign. When in doubt, follow the patterns in `sync.md`.


