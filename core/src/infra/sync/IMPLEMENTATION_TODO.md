# Leaderless Sync - Implementation TODO Map

**Status**: Architecture Complete ✅, Implementation Stubs Remaining
**Last Updated**: 2025-10-08
**Build**: Compiles successfully

---

## 🎯 Critical Path (Must Have for MVP)

### 1. Network Message Integration (HIGH PRIORITY)

**Location**: `service/sync/peer.rs`

**Current State**: Methods exist but don't send to network
```rust
// Line 135, 182
// TODO: Send to all sync_partners via network protocol
```

**What's Needed**:
```rust
impl PeerSync {
    async fn broadcast_state_change(&self, change: StateChangeMessage) {
        // Get sync_partners from database
        let partners = query_sync_partners(self.library_id).await?;

        // Send to each via NetworkingService
        for partner in partners {
            networking.send_message(
                partner.remote_device_id,
                "sync",
                SyncMessage::StateChange { ... }
            ).await?;
        }
    }

    async fn broadcast_shared_change(&self, entry: SharedChangeEntry) {
        // Same pattern for SharedChange messages
    }
}
```

**Dependencies**:
- Access to NetworkingService in PeerSync
- `sync_partners` table querying
- Message serialization already done ✅

**Effort**: ~100 lines
**Complexity**: Medium

---

### 2. Model-Specific Apply Functions (HIGH PRIORITY)

**Location**: `service/sync/protocol_handler.rs` + individual model files

**Current State**: Generic registry dispatch, but models don't implement apply yet
```rust
// Line 260, 237
// TODO: Deserialize and upsert based on model_type
// TODO: Deserialize and merge based on model_type
```

**What's Needed** (per model):

**Example for Location (Device-Owned)**:
```rust
// In infra/db/entities/location.rs
impl Model {
    pub async fn apply_state_change(
        data: serde_json::Value,
        db: &DatabaseConnection
    ) -> Result<()> {
        let location: Self = serde_json::from_value(data)?;

        // Idempotent upsert
        location::ActiveModel::from(location)
            .insert_or_update(db)
            .await?;

        Ok(())
    }
}
```

**Example for Tag (Shared, with conflict resolution)**:
```rust
// In infra/db/entities/tag.rs
impl Model {
    pub async fn apply_shared_change(
        entry: SharedChangeEntry,
        db: &DatabaseConnection
    ) -> Result<()> {
        let tag: Self = serde_json::from_value(entry.data)?;

        // Semantic tags use random UUIDs, not deterministic
        // Tags with same canonical_name are allowed (polymorphic naming)

        // Check if this exact tag UUID exists
        if let Some(existing) = find_by_uuid(tag.uuid, db).await? {
            // Update existing tag (last-writer-wins for properties)
            update_tag(existing, tag, entry.hlc, db).await?;
        } else {
            // Insert new tag (preserves all tags via union merge)
            tag.insert(db).await?;
        }

        Ok(())
    }
}
```

**Models to Implement**:
- ✅ location (device-owned, state-based) - **IMPLEMENTED**
- ⚠️ entry (device-owned, state-based)
- ⚠️ volume (device-owned, state-based)
- ⚠️ device (special: each broadcasts its own)
- ✅ tag (shared, log-based, union merge) - **IMPLEMENTED**
- ⚠️ album (shared, log-based, union merge)
- ⚠️ user_metadata (shared, log-based, LWW)

**Effort**: ~50 lines per model (~350 lines total)
**Complexity**: Medium-High (need conflict resolution)

**Status**: 2/7 models implemented (location, tag)

---

### 3. Registry Function Pointers (MEDIUM PRIORITY)

**Location**: `infra/sync/registry.rs`

**Current State**: Registry tracks metadata but can't call apply functions
```rust
// Line 28-29
// TODO: Function pointers for serialize/deserialize/apply
// Will be implemented when we wire up full sync
```

**What's Needed**:
```rust
type ApplyFn = fn(serde_json::Value, &DatabaseConnection) -> BoxFuture<'static, Result<()>>;

pub struct SyncableModelRegistration {
    pub model_type: &'static str,
    pub table_name: &'static str,
    pub is_device_owned: bool,

    // NEW:
    pub apply_fn: ApplyFn,  // Polymorphic dispatch!
}

// Then in apply_sync_entry:
pub async fn apply_sync_entry(
    model_type: &str,
    data: serde_json::Value,
) -> Result<()> {
    let registry = SYNCABLE_REGISTRY.read().unwrap();
    let registration = registry.get(model_type)
        .ok_or_else(|| anyhow!("Unknown model: {}", model_type))?;

    // Call the registered function!
    (registration.apply_fn)(data, db).await
}
```

**Effort**: ~50 lines in registry, ~20 lines per model registration
**Complexity**: Medium (async function pointers are tricky)

---

### 4. TransactionManager HLC Integration (MEDIUM PRIORITY)

**Location**: `infra/sync/transaction.rs`

**Current State**: Methods exist but are stubs
```rust
// Line 118-150
// TODO: Implement
// 1. Verify model.is_device_owned()
// 2. Generate HLC
// 3. Write to peer_log
// 4. Emit event for broadcast
```

**What's Needed**:
```rust
pub async fn commit_device_owned<M>(
    &self,
    library: &Library,
    model: M,
) -> Result<M>
where
    M: Syncable,
{
    // 1. Verify classification
    if !model.is_device_owned() {
        return Err(TxError::InvalidModel("Expected device-owned".into()));
    }

    // 2. Write to database (no log!)
    let saved = model.insert(library.db()).await?;

    // 3. Emit event → PeerSync picks up and broadcasts
    self.event_bus.emit(Event::StateChanged {
        library_id: library.id(),
        model_type: M::SYNC_MODEL,
        record_uuid: saved.sync_id(),
        device_id: model.device_id().unwrap(),
        data: saved.to_sync_json()?,
    });

    Ok(saved)
}

pub async fn commit_shared<M>(
    &self,
    library: &Library,
    model: M,
) -> Result<M>
where
    M: Syncable,
{
    // 1. Verify classification
    if model.is_device_owned() {
        return Err(TxError::InvalidModel("Expected shared".into()));
    }

    // 2. Get PeerSync to generate HLC and write to peer_log
    let peer_sync = library.sync_service()
        .ok_or(TxError::SyncLog("Sync service not initialized".into()))?
        .peer_sync();

    // 3. Atomic: DB write + peer_log write
    let saved = library.db().transaction(|txn| async {
        let saved = model.insert(txn).await?;

        // Write to peer_log happens inside PeerSync
        peer_sync.broadcast_shared_change(
            M::SYNC_MODEL.to_string(),
            saved.sync_id(),
            ChangeType::Insert,
            saved.to_sync_json()?,
        ).await?;

        Ok(saved)
    }).await?;

    Ok(saved)
}
```

**Effort**: ~150 lines
**Complexity**: Medium (need Library parameter, transaction integration)

---

## 🔌 Network Integration (Must Have)

### 5. Protocol Handler Wiring (HIGH PRIORITY)

**Location**: `service/network/protocol/sync/handler.rs`

**Current State**: Completely stubbed out
```rust
// All methods return "not yet implemented"
warn!("SyncProtocolHandler called but protocol not yet implemented");
```

**What's Needed**:
```rust
impl ProtocolHandler for SyncProtocolHandler {
    async fn handle_stream(&self, send, recv, remote_node_id) {
        // 1. Read SyncMessage from recv
        let message: SyncMessage = read_message(recv).await?;

        // 2. Route to StateSyncHandler or LogSyncHandler
        match message {
            SyncMessage::StateChange { .. } => {
                state_handler.handle_state_change(...).await?
            }
            SyncMessage::SharedChange { entry } => {
                log_handler.handle_shared_change(entry).await?
            }
            // ... other messages
        }

        // 3. Write response to send (if needed)
    }
}
```

**Effort**: ~200 lines
**Complexity**: Medium (stream handling, message routing)

---

### 6. PeerSync Background Tasks (MEDIUM PRIORITY)

**Location**: `service/sync/peer.rs`

**Current State**: Loop exists but tasks not implemented
```rust
// Line 92-96
// TODO: Start background tasks for:
// - Listening to network messages
// - Processing buffer queue
// - Pruning sync log
// - Heartbeat to peers
// - Reconnect to offline peers
```

**What's Needed**:
```rust
pub async fn start(&self) -> Result<()> {
    // Spawn task 1: Process buffer queue
    tokio::spawn({
        let peer_sync = self.clone();
        async move {
            loop {
                tokio::time::sleep(Duration::from_secs(1)).await;
                peer_sync.process_buffer_queue().await;
            }
        }
    });

    // Spawn task 2: Prune sync log periodically
    tokio::spawn({
        let peer_log = self.peer_log.clone();
        async move {
            loop {
                tokio::time::sleep(Duration::from_mins(5)).await;
                peer_log.prune_acked().await;
            }
        }
    });

    // Spawn task 3: Heartbeat to peers
    // Spawn task 4: Reconnect attempts
}
```

**Effort**: ~150 lines
**Complexity**: Medium

---

## 📊 State Management & Persistence

### 7. Checkpoint Persistence (LOW PRIORITY)

**Location**: `service/sync/state.rs`

**Current State**: Checkpoint save/load are stubs
```rust
// Line 185-194
pub async fn save(&self) -> Result<()> {
    // TODO: Persist to disk for crash recovery
}

pub async fn load() -> Result<Option<Self>> {
    // TODO: Load from disk
}
```

**What's Needed**:
```rust
// Save to library_path/sync_checkpoint.json
let checkpoint_path = library_path.join("sync_checkpoint.json");
let json = serde_json::to_string_pretty(self)?;
tokio::fs::write(checkpoint_path, json).await?;
```

**Effort**: ~30 lines
**Complexity**: Low

---

### 8. Backfill Network Requests (MEDIUM PRIORITY)

**Location**: `service/sync/backfill.rs`

**Current State**: Methods exist but return empty responses
```rust
// Line 202-240
// TODO: Send StateRequest via network
// TODO: Send SharedChangeRequest via network
```

**What's Needed**:
```rust
async fn request_state_batch(...) -> Result<SyncMessage> {
    // Serialize request
    let request = SyncMessage::StateRequest { ... };

    // Send via networking service
    let response = networking.send_request(
        peer,
        "sync",
        request
    ).await?;

    // Deserialize response
    let message: SyncMessage = serde_json::from_slice(&response)?;
    Ok(message)
}
```

**Effort**: ~80 lines
**Complexity**: Medium

---

## 🔍 Data Serialization (Quality of Life)

### 9. Row to JSON Serialization (MEDIUM PRIORITY)

**Location**: `service/sync/protocol_handler.rs`

**Current State**: Returns empty JSON
```rust
// Line 145-151
// TODO: Proper serialization per model type via registry
data: serde_json::json!({})  // Placeholder
```

**What's Needed**:
```rust
// Use Syncable trait's to_sync_json() method
// Need to fetch actual model from row and serialize it
let model = Model::from_query_result(row)?;
let data = model.to_sync_json()?;
```

**Effort**: ~50 lines (generic row -> model conversion)
**Complexity**: Medium

---

### 10. Shared State Fallback Query (LOW PRIORITY)

**Location**: `service/sync/protocol_handler.rs`

**Current State**: Returns empty JSON
```rust
// Line 246-250
// TODO: Query via registry instead of hardcoding
async fn get_current_shared_state() -> Result<serde_json::Value> {
    Ok(serde_json::json!({
        "tags": [],
        "albums": [],
        "user_metadata": [],
    }))
}
```

**What's Needed**:
```rust
async fn get_current_shared_state() -> Result<serde_json::Value> {
    // Query all shared models from registry
    let shared_models = registry.iter()
        .filter(|(_, reg)| !reg.is_device_owned)
        .map(|(name, _)| name);

    let mut state = serde_json::Map::new();
    for model_type in shared_models {
        let records = query_all_records(model_type).await?;
        state.insert(model_type.clone(), serde_json::to_value(records)?);
    }

    Ok(serde_json::Value::Object(state))
}
```

**Effort**: ~60 lines
**Complexity**: Medium

---

## 🔄 Periodic Tasks (Nice to Have)

### 11. SyncService Periodic Tasks (MEDIUM PRIORITY)

**Location**: `service/sync/mod.rs`

**Current State**: Loop exists but empty
```rust
// Line 92-99
// TODO: Implement periodic tasks:
// - Process buffer queue
// - Prune sync log
// - Heartbeat to peers
// - Reconnect to offline peers
```

**What's Needed**:
```rust
async fn run_sync_loop(peer_sync: Arc<PeerSync>, ...) {
    let mut prune_interval = tokio::time::interval(Duration::from_secs(300)); // 5 min
    let mut heartbeat_interval = tokio::time::interval(Duration::from_secs(30));

    loop {
        tokio::select! {
            _ = prune_interval.tick() => {
                peer_sync.peer_log.prune_acked().await?;
            }
            _ = heartbeat_interval.tick() => {
                send_heartbeat_to_peers().await?;
            }
            _ = shutdown_rx.recv() => break,
        }
    }
}
```

**Effort**: ~80 lines
**Complexity**: Low-Medium

---

### 12. Peer Reconnection Logic (LOW PRIORITY)

**Location**: `service/sync/backfill.rs`

**Current State**: Placeholder
```rust
// Line 251-253
// TODO: Save checkpoint, select new peer, resume
```

**What's Needed**:
```rust
async fn on_peer_disconnected(&self, peer_id: Uuid) {
    if self.is_backfilling_from(peer_id) {
        // Save current progress
        self.save_checkpoint().await?;

        // Select new peer
        let peers = get_available_peers().await?;
        let new_peer = select_backfill_peer(peers)?;

        // Resume from checkpoint
        self.resume_backfill(new_peer, self.load_checkpoint().await?).await?;
    }
}
```

**Effort**: ~60 lines
**Complexity**: Medium

---

## 📈 Optimizations (Can Wait)

### 13. Backfill Progress Calculation (LOW PRIORITY)

**Location**: `service/sync/backfill.rs`

**Current State**: Hardcoded 0.5
```rust
// Line 157
current_checkpoint.update(chk, 0.5); // TODO: Calculate actual progress
```

**What's Needed**:
```rust
let total_expected = estimate_total_records(model_type).await?;
let progress = records_received as f32 / total_expected as f32;
current_checkpoint.update(chk, progress);
```

**Effort**: ~30 lines
**Complexity**: Low

---

### 14. Checkpointing in State Queries (LOW PRIORITY)

**Location**: `service/sync/protocol_handler.rs`

**Current State**: Always None
```rust
// Line 104-105
checkpoint: None, // TODO: Implement checkpointing
has_more: false,  // TODO: Implement pagination
```

**What's Needed**:
```rust
// Track offset in query
let offset = parse_checkpoint(checkpoint)?;
query.push_str(&format!(" OFFSET {}", offset));

// Return new checkpoint if more data
let new_checkpoint = if total_count > batch_size {
    Some(format!("offset-{}", offset + batch_size))
} else {
    None
};
```

**Effort**: ~40 lines
**Complexity**: Low

---

### 15. Multi-Peer Backfill (LOW PRIORITY)

**Location**: `service/sync/backfill.rs`

**Current State**: Only backfills from primary peer
```rust
// Line 102-104
// TODO: Get list of all peers, not just primary
// For now, just backfill from primary peer
```

**What's Needed**:
```rust
async fn backfill_device_owned_state(&self) {
    // Get ALL peers
    let peers = get_all_sync_partners().await?;

    // Backfill each peer's device-owned data
    for peer in peers {
        self.backfill_peer_state(
            peer.id,
            vec!["location", "entry", "volume"],
            None
        ).await?;
    }
}
```

**Effort**: ~40 lines
**Complexity**: Low

---

### 16. Shared State Fallback Application (LOW PRIORITY)

**Location**: `service/sync/backfill.rs`

**Current State**: Logged but not applied
```rust
// Line 193
// TODO: Deserialize and insert tags, albums, etc.
```

**What's Needed**:
```rust
if let Some(state) = current_state {
    // Deserialize shared models from state
    if let Some(tags) = state.get("tags") {
        let tags: Vec<tag::Model> = serde_json::from_value(tags)?;
        for tag in tags {
            tag.insert_or_ignore(db).await?;
        }
    }
    // Same for albums, user_metadata
}
```

**Effort**: ~40 lines
**Complexity**: Low

---

## 🧹 Cleanup (Optional)

### 17. Remove Old Stubbed Methods (LOW PRIORITY)

**Location**: `infra/sync/transaction.rs`

**Current State**: Old methods still exist for compatibility
```rust
// Line 153-204
// OLD METHODS (STUBBED - Will be replaced)
pub async fn log_change_stubbed(...)
pub async fn log_batch_stubbed(...)
pub async fn log_bulk_stubbed(...)
```

**Action**: Delete these once new methods are fully implemented

**Effort**: Delete ~50 lines
**Complexity**: Trivial (just verify nothing calls them)

---

### 18. Remove SyncApplier Stub (LOW PRIORITY)

**Location**: `service/sync/applier.rs`

**Current State**: Entire file is a stub
```rust
// STUB - Being replaced with PeerSync
```

**Action**: Can delete this file once registry apply is working

**Effort**: Delete file
**Complexity**: Trivial

---

## 📋 Summary by Priority

### 🔴 Critical Path (Must Implement):
1. **Network Message Integration** (~100 lines) - Send/receive messages
2. **Model Apply Functions** (~350 lines) - 7 models × 50 lines each
3. **Registry Function Pointers** (~100 lines) - Polymorphic dispatch
4. **TransactionManager Integration** (~150 lines) - HLC + peer_log

**Total Critical**: ~700 lines, 1-2 days of focused work

### 🟡 Important (Should Implement):
5. **Protocol Handler Wiring** (~200 lines) - Message routing
6. **PeerSync Background Tasks** (~150 lines) - Periodic operations
7. **Backfill Network Requests** (~80 lines) - Actual network calls

**Total Important**: ~430 lines, 1 day

### 🟢 Nice to Have (Can Wait):
8-16. Various optimizations and completions

**Total Nice**: ~290 lines, 0.5 days

---

## 🎯 Recommended Implementation Order

### Phase 1: Get Basic Sync Working (Critical)
1. Model apply functions (start with location, tag)
2. Registry function pointers
3. Network message integration
4. TransactionManager integration

**Result**: Can sync locations and tags between 2 devices!

### Phase 2: Protocol & Background (Important)
5. Protocol handler wiring
6. PeerSync background tasks
7. Backfill network requests

**Result**: Full sync including backfill works!

### Phase 3: Polish (Nice to Have)
8-16. Optimizations, edge cases, cleanup

**Result**: Production-ready!

---

## 📊 Current Implementation Status

| Component | Architecture | Implementation | Status |
|-----------|--------------|----------------|--------|
| HLC | ✅ | ✅ | Complete |
| PeerLog | ✅ | ✅ | Complete |
| State Machine | ✅ | ✅ | Complete |
| PeerSync | ✅ | 🟡 | Needs network integration |
| Protocol Messages | ✅ | ✅ | Complete |
| Protocol Handlers | ✅ | 🟡 | Needs wiring |
| Backfill | ✅ | 🟡 | Needs network calls |
| TransactionManager | ✅ | 🟡 | Needs HLC integration |
| Registry Dispatch | ✅ | 🟡 | Needs function pointers |
| Model Apply | ✅ | ⚠️ | Needs per-model impl |

**Legend**: ✅ Done | 🟡 Partial | ⚠️ Not Started

---

## 🚀 Estimated Total Remaining Work

**Critical Path**: ~700 lines, 1-2 days
**Full Implementation**: ~1,420 lines, 3-4 days
**Current Progress**: Architecture complete, ~40% implementation done

The hardest part (architecture) is DONE! What remains is straightforward implementation following the established patterns.

