# Spacedrive Sync System

**Status**: Implementation Ready
**Version**: 3.0 (Leaderless)
**Last Updated**: 2025-10-09
**Architecture**: `core/src/infra/sync/NEW_SYNC.md`

> 📋 **Implementation Tracking**:
> - [Sync Roadmap](./sync-roadmap.md) - Quick reference and status overview
> - [Detailed Roadmap](../../core/src/infra/sync/SYNC_IMPLEMENTATION_ROADMAP.md) - Comprehensive tracking with code examples
> - [Network Integration Status](../../core/src/infra/sync/NETWORK_INTEGRATION_STATUS.md) - Phase-by-phase progress
> - [File Organization](../../core/src/infra/sync/FILE_ORGANIZATION.md) - Navigate the codebase

---

## Overview

Spacedrive's Library sync system enables real-time, multi-device synchronization of library metadata using a **leaderless hybrid model**. All devices are peers—no leader election, no bottlenecks, no single point of failure.

### Core Principle

**Data ownership drives sync strategy**:
- **Device-owned data** (locations, entries): State-based replication (simple, fast, no conflicts)
- **Shared resources** (tags, albums): HLC-ordered logs (conflict resolution, small, prunable)

---

## Architecture Overview

### The Library Model

A Library (e.g., "Jamie's Library") is a shared data space replicated across devices:

```
Device A (Desktop):
  Jamie's Library.sdlibrary/
    ├── database.db  ← Replicated metadata from ALL devices
    └── sync.db      ← MY pending shared changes (HLC-based, pruned)

Device B (Laptop):
  Jamie's Library.sdlibrary/
    ├── database.db  ← Same replicated metadata
    └── sync.db      ← MY pending shared changes

Device C (Phone):
  Jamie's Library.sdlibrary/
    ├── database.db  ← Same replicated metadata
    └── sync.db      ← MY pending shared changes
```

**What syncs**: Metadata (locations, entries, tags, albums)
**What doesn't sync**: File content (stays on original device)

### No Central Leader

Every device is equal:
- ✅ Any device can make changes anytime
- ✅ Changes sync peer-to-peer
- ✅ No coordination required
- ✅ Works fully offline

---

## Data Classification

### Device-Owned Data (State-Based Sync)

Data that belongs to a specific device and can only be modified by that device.

| Model | Owner | Example |
|-------|-------|---------|
| **Location** | Device that has the filesystem | `/Users/jamie/Photos` on Device A |
| **Entry** | Device via location | `vacation.jpg` in Device A's location |
| **Volume** | Device with the physical drive | `MacBook SSD` on Device A |
| **Audit Log** | Device that performed action | "Device A created location" |

**Sync Strategy**: State broadcast
- Device writes to local database
- Broadcasts current state to all peers
- Peers apply idempotently (upsert)
- No log needed!

**Why no conflicts**: Device A can't modify Device B's filesystem. Ownership is absolute.

---

### Shared Resources (Log-Based Sync)

Data that any device can modify and needs conflict resolution.

| Model | Shared Across | Example |
|-------|--------------|---------|
| **Tag** | All devices | "Vacation" tag (supports same name in different contexts) |
| **Album** | All devices | "Summer 2024" collection with entries from multiple devices |
| **UserMetadata** | All devices (when content-scoped) | Favoriting a photo applies to the content everywhere |

**Sync Strategy**: HLC-ordered log
- Device writes to local database
- Generates HLC timestamp
- Writes to local `sync.db` log
- Broadcasts to all peers
- Peers apply in HLC order
- Log pruned when all peers ACK

**Why log needed**: Multiple devices can create/modify tags and albums independently. Need ordering for proper merge resolution.

---

## Hybrid Logical Clocks (HLC)

### What is HLC?

A distributed timestamp that provides:
- **Total ordering**: Any two HLCs can be compared
- **Causality tracking**: If A caused B, then HLC(A) < HLC(B)
- **No coordination**: Each device generates independently

### Structure

```rust
pub struct HLC {
    timestamp: u64,    // Milliseconds since epoch (physical time)
    counter: u64,      // Logical counter for same millisecond
    device_id: Uuid,   // Device that generated this
}

// Ordering: timestamp, then counter, then device_id
// Example: HLC(1000,0,A) < HLC(1000,1,B) < HLC(1001,0,C)
```

### Generation

```rust
// Generate next HLC
fn next_hlc(last: Option<HLC>, device_id: Uuid) -> HLC {
    let now = current_time_ms();

    match last {
        Some(last) if last.timestamp == now => {
            // Same millisecond, increment counter
            HLC { timestamp: now, counter: last.counter + 1, device_id }
        }
        _ => {
            // New millisecond, reset counter
            HLC { timestamp: now, counter: 0, device_id }
        }
    }
}
```

### Causality Tracking

```rust
// When receiving HLC from peer
fn update_hlc(local: &mut HLC, received: HLC) {
    // Advance to max timestamp
    local.timestamp = local.timestamp.max(received.timestamp);

    // Increment counter if same timestamp
    if local.timestamp == received.timestamp {
        local.counter = local.counter.max(received.counter) + 1;
    }
}
```

**Result**: Preserves causality without clock synchronization!

---

## Sync Protocols

### Protocol 1: State-Based (Device-Owned)

#### Broadcasting Changes

```rust
// Device A creates location
location.insert(db).await?;

// Broadcast state
broadcast(StateChange {
    model_type: "location",
    record_uuid: location.uuid,
    device_id: MY_DEVICE_ID,
    data: serde_json::to_value(&location)?,
    timestamp: Utc::now(),
});
```

#### Receiving Changes

```rust
// Peer receives state change
async fn on_state_change(change: StateChange) {
    let location: location::Model = serde_json::from_value(change.data)?;

    // Idempotent upsert
    location::ActiveModel::from(location)
        .insert_or_update(db)
        .await?;

    // Emit event
    event_bus.emit(Event::LocationSynced { ... });
}
```

**Properties**:
- ✅ Simple (just broadcast state)
- ✅ No ordering needed (idempotent)
- ✅ No log (stateless)
- ✅ Fast (~100ms latency)

---

### Protocol 2: Log-Based (Shared Resources)

#### Broadcasting Changes

```rust
// Device A creates tag
let tag = tag::ActiveModel { name: Set("Vacation"), ... };
tag.insert(db).await?;

// Generate HLC
let hlc = hlc_generator.next();

// Write to MY sync log
sync_db.append(SharedChangeEntry {
    hlc,
    model_type: "tag",
    record_uuid: tag.uuid,
    change_type: ChangeType::Insert,
    data: serde_json::to_value(&tag)?,
}).await?;

// Broadcast to peers
broadcast(SharedChange {
    hlc,
    model_type: "tag",
    record_uuid: tag.uuid,
    change_type: ChangeType::Insert,
    data: serde_json::to_value(&tag)?,
});
```

#### Receiving Changes

```rust
// Peer receives shared change
async fn on_shared_change(entry: SharedChangeEntry) {
    // Update causality
    hlc_generator.update(entry.hlc);

    // Apply to database (with conflict resolution)
    apply_with_merge(entry).await?;

    // Send ACK
    send_ack(entry.device_id, entry.hlc).await?;
}
```

#### Pruning

```rust
// After all peers ACK
async fn on_all_peers_acked(up_to_hlc: HLC) {
    // Delete from MY sync log
    sync_db.delete_where(hlc <= up_to_hlc).await?;

    // Log stays small!
}
```

**Properties**:
- ✅ Ordered (HLC)
- ✅ Conflict resolution (merge strategies)
- ✅ Small log (pruned aggressively)
- ✅ Offline-capable (queues locally)

---

## Database Schemas

### database.db (Per-Library, Replicated)

Lives in each library folder, replicated across all devices:

```sql
-- Device-owned (state replicated)
CREATE TABLE locations (
    id INTEGER PRIMARY KEY,
    uuid TEXT NOT NULL UNIQUE,
    device_id TEXT NOT NULL,  -- Owner
    path TEXT NOT NULL,
    name TEXT,
    updated_at TIMESTAMP NOT NULL,
);

CREATE TABLE entries (
    id INTEGER PRIMARY KEY,
    uuid TEXT NOT NULL UNIQUE,
    location_id INTEGER NOT NULL → device_id,
    name TEXT NOT NULL,
    size INTEGER NOT NULL,
    updated_at TIMESTAMP NOT NULL,
);

CREATE TABLE volumes (
    id INTEGER PRIMARY KEY,
    uuid TEXT NOT NULL UNIQUE,
    device_id TEXT NOT NULL,  -- Owner
    name TEXT NOT NULL,
);

-- Shared resources (log replicated)
CREATE TABLE tags (
    id INTEGER PRIMARY KEY,
    uuid TEXT NOT NULL UNIQUE,
    canonical_name TEXT NOT NULL,   -- Can be duplicated
    namespace TEXT,                 -- Context grouping
    display_name TEXT,
    formal_name TEXT,
    abbreviation TEXT,
    aliases TEXT,                   -- JSON array
    color TEXT,
    icon TEXT,
    tag_type TEXT DEFAULT 'standard',
    privacy_level TEXT DEFAULT 'normal',
    created_at TIMESTAMP NOT NULL,
    created_by_device TEXT NOT NULL,
    -- NO device_id ownership field! (shared resource)
);

CREATE TABLE albums (
    id INTEGER PRIMARY KEY,
    uuid TEXT NOT NULL UNIQUE,
    name TEXT NOT NULL,
    description TEXT,
    created_at TIMESTAMP NOT NULL,
    created_by_device TEXT NOT NULL,
    -- NO device_id field!
);

-- Junction tables for many-to-many relationships
CREATE TABLE album_entries (
    album_id INTEGER NOT NULL,
    entry_id INTEGER NOT NULL,
    added_at TIMESTAMP NOT NULL,
    added_by_device TEXT NOT NULL,
    PRIMARY KEY (album_id, entry_id),
    FOREIGN KEY (album_id) REFERENCES albums(id),
    FOREIGN KEY (entry_id) REFERENCES entries(id)
);

CREATE TABLE user_metadata_semantic_tags (
    user_metadata_id INTEGER NOT NULL,
    tag_id INTEGER NOT NULL,
    applied_context TEXT,
    confidence REAL DEFAULT 1.0,
    source TEXT DEFAULT 'user',
    created_at TIMESTAMP NOT NULL,
    device_uuid TEXT NOT NULL,
    PRIMARY KEY (user_metadata_id, tag_id),
    FOREIGN KEY (user_metadata_id) REFERENCES user_metadata(id),
    FOREIGN KEY (tag_id) REFERENCES tags(id)
);

-- Sync coordination
CREATE TABLE sync_partners (
    id INTEGER PRIMARY KEY,
    remote_device_id TEXT NOT NULL UNIQUE,
    sync_enabled BOOLEAN DEFAULT true,
    last_sync_at TIMESTAMP,
);

CREATE TABLE peer_sync_state (
    device_id TEXT PRIMARY KEY,
    last_state_sync TIMESTAMP,
    last_shared_hlc TEXT,
);
```

---

### sync.db (Per-Device, Prunable)

Lives in each library folder, contains only MY changes to shared resources:

```sql
-- MY changes to shared resources
CREATE TABLE shared_changes (
    hlc TEXT PRIMARY KEY,           -- e.g., "1730000000000-0-uuid"
    model_type TEXT NOT NULL,       -- "tag", "album", "user_metadata"
    record_uuid TEXT NOT NULL,
    change_type TEXT NOT NULL,      -- "insert", "update", "delete"
    data TEXT NOT NULL,             -- JSON payload
    created_at TIMESTAMP NOT NULL,
);

CREATE INDEX idx_shared_changes_hlc ON shared_changes(hlc);

-- Track peer ACKs
CREATE TABLE peer_acks (
    peer_device_id TEXT PRIMARY KEY,
    last_acked_hlc TEXT NOT NULL,
    acked_at TIMESTAMP NOT NULL,
);
```

**Size**: Stays tiny! Typically <100 entries after pruning.

---

## Model Dependencies and Sync Order

### Automatic Dependency Resolution

The sync system automatically computes the correct model ordering using topological sort. Each model declares its dependencies via the `Syncable` trait:

```rust
// Device has no dependencies (root of graph)
impl Syncable for device::Model {
    fn sync_depends_on() -> &'static [&'static str] {
        &[]
    }
}

// Location depends on Device
impl Syncable for location::Model {
    fn sync_depends_on() -> &'static [&'static str] {
        &["device"]
    }
}

// Entry depends on Location
impl Syncable for entry::Model {
    fn sync_depends_on() -> &'static [&'static str] {
        &["location"]
    }
}
```

At runtime, the system computes a topological sort:

```rust
let sync_order = compute_registry_sync_order().await?;
// Result: ["device", "location", "entry", "tag", ...]
// Guarantees: Parents always sync before children
```

**Benefits**:
- ✅ **Zero FK violations**: Dependencies always satisfied
- ✅ **Automatic**: No manual tier management
- ✅ **Self-documenting**: Dependencies declared in code
- ✅ **Validated**: Detects circular dependencies at startup

### Computed Dependency Graph

The actual sync order is computed from model declarations:

```
device ─────┐
            ↓
         location ─────┐
                       ↓
                    entry

tag (independent, no FK dependencies)
```

### Backfill Order

When a new device joins, models are automatically synced in computed dependency order:

```rust
// Phase 1: Compute dependency order
let sync_order = compute_registry_sync_order().await?;
// e.g., ["device", "location", "entry", "tag"]

// Phase 2: Sync device-owned models in order
for model_type in sync_order {
    if is_device_owned(&model_type).await {
        backfill_model(model_type).await?;
    }
}

// Phase 3: Sync shared resources (HLC ordered)
apply_shared_changes_in_hlc_order().await?;
```

**Implementation**: See `core/src/infra/sync/dependency_graph.rs` for the topological sort algorithm.

### Model Classification Reference

| Model | Type | Sync Strategy | Conflict Resolution |
|-------|------|---------------|--------------------|
| **Device** | Device-owned (self) | State broadcast | None (each device owns its record) |
| **Location** | Device-owned | State broadcast | None (device owns filesystem) |
| **Entry** | Device-owned | State broadcast | None (via location ownership) |
| **Volume** | Device-owned | State broadcast | None (device owns hardware) |
| **Tag** | Shared | HLC log | Union merge (preserve all) |
| **Album** | Shared | HLC log | Union merge entries |
| **UserMetadata** | Mixed | Depends on scope | Entry-scoped: device-owned, Content-scoped: LWW |
| **AuditLog** | Device-owned | State broadcast | None (device's actions) |
| **DirectoryPaths** | Derived (cache) | Not synced | Rebuilt locally from entry hierarchy |
| **EntryClosure** | Derived (cache) | Not synced | Rebuilt locally from entry parent_id |
| **TagClosure** | Derived (cache) | Not synced | Rebuilt locally from tag relationships |

### Delete Handling

**Device-Owned Resources**: No explicit delete propagation needed!
- When a device deletes a location/entry, it simply stops including it in state broadcasts
- Other devices detect absence during next state sync
- No delete records in logs = no privacy concerns

**Shared Resources**: Two approaches:

#### Option 1: Tombstone Records (Current Design)
```rust
// Explicit delete in sync log
SharedChangeEntry {
    hlc: HLC(1001,A),
    change_type: ChangeType::Delete,
    record_uuid: tag_uuid,
    data: {} // Empty
}
```
**Privacy Risk**: Deleted tag names remain in sync log until pruned

#### Option 2: Periodic State Reconciliation (Privacy-Preserving)
```rust
// No delete records! Instead, periodically sync full state
// Device A: "I have tags: [uuid1, uuid2]"
// Device B: "I have tags: [uuid1, uuid2, uuid3]"
// Device B detects uuid3 was deleted by A
```

**Recommendation**: Use Option 2 for sensitive data like tags. The slight delay in delete propagation is worth the privacy benefit.

---

## Sync Flows

### Example 1: Device Creates Location (State-Based)

```
Device A:
  1. LocationManager.add_location("/Users/jamie/Photos")
  2. INSERT INTO locations (device_id=A, uuid=loc-123, ...)
  3. Emit: LocationCreated event
  4. SyncService.on_location_created()
     └─ Broadcast StateChange to sync_partners: [B, C]

  Total time: ~50ms (database write + broadcast)

Device B, C:
  1. Receive: StateChange { device_id: A, ... }
  2. INSERT INTO locations (device_id=A, uuid=loc-123, ...)
  3. Emit: LocationSynced event
  4. UI updates → User sees Device A's location!

  Total time: ~100ms from Device A's action
```

**Result**: All devices can SEE Device A's location in the UI, but only Device A can access the physical files.

---

### Example 2: Device Creates Tag (Log-Based)

```
Device A:
  1. TagManager.create_tag("Vacation")
  2. Generate HLC: HLC(1730000000000, 0, device-a-uuid)
  3. BEGIN TRANSACTION
     ├─ INSERT INTO tags (uuid=tag-123, name="Vacation")
     └─ INSERT INTO shared_changes (hlc=..., model="tag", ...)
     COMMIT
  4. Broadcast SharedChange to sync_partners: [B, C]

  Total time: ~60ms (database + log write + broadcast)

Device B:
  1. Receive: SharedChange { hlc: HLC(1730000000000, 0, A) }
  2. Update local HLC (causality tracking)
  3. Check if tag exists (by UUID)
     └─ If exists: Update properties (last-writer-wins)
     └─ If not: INSERT INTO tags (...) preserving UUID
  4. Send AckSharedChanges to Device A

Device A (later):
  1. Receive ACK from Device B: up_to_hlc=HLC(1730000000000, 0, A)
  2. Receive ACK from Device C: up_to_hlc=HLC(1730000000000, 0, A)
  3. All acked → DELETE FROM shared_changes WHERE hlc <= ...
  4. Log pruned!
```

**Result**: Tag visible on all devices, log stays small.

---

### Example 3: New Device Joins Library

```
Device D joins "Jamie's Library":

Phase 0: Peer Selection
  Scan available peers: [A (online, 20ms), B (online, 50ms), C (offline)]
  Select Device A (fastest)

  Set sync state: BACKFILLING
  Start buffering any incoming live updates

Phase 1: Device-Owned State Sync (Backfill)
  For each peer device (A, B, C):
    Send: StateRequest {
      model_types: ["location", "entry", "volume"],
      device_id: peer.id,
      checkpoint: resume_token,  // For resumability
      batch_size: 10_000
    }

    Receive: StateResponse {
      locations: [peer's locations (batch 1 of 10)],
      entries: [peer's entries (batch 1 of 100)],
      checkpoint: "entry-10000",
      has_more: true
    }

    Apply batch (bulk insert, idempotent)
    Save checkpoint
    Repeat until has_more = false

  Meanwhile:
    - Device C comes online, creates location
    - Device D receives StateChange → BUFFERED (not applied yet)

  Result: Device D has all historical locations/entries from all devices

Phase 2: Shared Resource Sync (Backfill)
  Request from Device A:
    Send: SharedChangeRequest { since_hlc: None }
    Receive: SharedChangeResponse {
      entries: [...],  // All unacked shared changes
      current_state: { tags: [...], albums: [...] }
    }
    Apply in HLC order

  Result: Device D has all tags/albums

Phase 3: Catch Up (Process Buffer)
  Set sync state: CATCHING_UP

  Process buffered updates in order:
    1. StateChange(location from C, t=1000) → Apply
    2. SharedChange(new tag from B, HLC(2050,B)) → Apply
    3. StateChange(entry from A, t=1005) → Apply

  Continue buffering new updates during catch-up

  Result: Device D caught up on changes during backfill

Phase 4: Ready
  Buffer empty
  Set sync state: READY

  Device D is fully synced:
    ✅ Can make changes
    ✅ Applies live updates immediately
    ✅ Broadcasts to other peers
```

**Critical**: Live updates are buffered during backfill to prevent applying changes to incomplete state!

---

## Conflict Resolution

### No Conflicts (Device-Owned)

```rust
Device A: Creates location "/Users/jamie/Photos"
Device B: Creates location "/home/jamie/Documents"

Resolution: No conflict! Different owners.
Both apply. All devices see both locations.
```

### Union Merge (Tags)

```rust
Device A: Creates tag "Vacation" → HLC(1000,A) → UUID: abc-123
Device B: Creates tag "Vacation" → HLC(1001,B) → UUID: def-456

Resolution: Union merge
  Both tags preserved (different UUIDs)
  Semantic tagging supports polymorphic naming
  Tags differentiated by namespace/context

Result: Two "Vacation" tags with different contexts/UUIDs
```

### Union Merge (Albums)

```rust
Device A: Adds entry-1 to album → HLC(1000,A)
Device B: Adds entry-2 to album → HLC(1001,B)

Resolution: Union merge
  album_entries table gets both records
  Both additions preserved

Result: Album contains both entries
```

### Junction Table Sync

Many-to-many relationships sync as individual records:

```rust
// Album-Entry junction
Device A: INSERT album_entries (album_1, entry_1) → HLC(1000,A)
Device B: INSERT album_entries (album_1, entry_2) → HLC(1001,B)

Resolution: Both records preserved
  Primary key (album_id, entry_id) prevents duplicates
  Different entries = no conflict

// Duplicate case
Device A: INSERT album_entries (album_1, entry_1) → HLC(1000,A)
Device B: INSERT album_entries (album_1, entry_1) → HLC(1001,B)

Resolution: Idempotent
  Same primary key = update metadata only
  Last writer wins for added_at timestamp
```

### Last-Writer-Wins (Metadata)

```rust
Device A: Favorites photo → HLC(1000,A)
Device B: Un-favorites photo → HLC(1001,B)

Resolution: HLC ordering
  HLC(1001,B) > HLC(1000,A)
  Device B's change wins

Result: Photo is NOT favorited
```

---

## Implementation Components

### SyncService

Located: `core/src/service/sync/mod.rs`

```rust
pub struct SyncService {
    library_id: Uuid,
    sync_db: Arc<SyncDb>,              // My sync log
    hlc_generator: Arc<Mutex<HLCGenerator>>,
    protocol_handler: Arc<SyncProtocolHandler>,
    peer_states: Arc<RwLock<HashMap<Uuid, PeerSyncState>>>,
}

impl SyncService {
    /// Broadcast device-owned state change
    pub async fn broadcast_state(&self, change: StateChange);

    /// Broadcast shared resource change (with HLC)
    pub async fn broadcast_shared(&self, entry: SharedChangeEntry);

    /// Handle received state change
    pub async fn on_state_received(&self, change: StateChange);

    /// Handle received shared change
    pub async fn on_shared_received(&self, entry: SharedChangeEntry);
}
```

**No roles!** Every device runs the same service.

---

### HLCGenerator

Located: `core/src/infra/sync/hlc.rs`

```rust
pub struct HLCGenerator {
    device_id: Uuid,
    last_hlc: Option<HLC>,
}

impl HLCGenerator {
    /// Generate next HLC
    pub fn next(&mut self) -> HLC {
        HLC::new(self.last_hlc, self.device_id)
    }

    /// Update based on received HLC (causality)
    pub fn update(&mut self, received: HLC) {
        if let Some(ref mut last) = self.last_hlc {
            last.update(received);
        }
    }
}
```

---

### SyncDb (Per-Device Log)

Located: `core/src/infra/sync/sync_db.rs`

```rust
pub struct SyncDb {
    library_id: Uuid,
    device_id: Uuid,
    conn: DatabaseConnection,
}

impl SyncDb {
    /// Append shared change
    pub async fn append(&self, entry: SharedChangeEntry) -> Result<()>;

    /// Get changes since HLC
    pub async fn get_since(&self, since: Option<HLC>) -> Result<Vec<SharedChangeEntry>>;

    /// Record peer ACK
    pub async fn record_ack(&self, peer: Uuid, hlc: HLC) -> Result<()>;

    /// Prune acknowledged changes
    pub async fn prune_acked(&self) -> Result<usize> {
        let min_hlc = self.get_min_acked_hlc().await?;

        if let Some(min) = min_hlc {
            self.delete_where(hlc <= min).await
        } else {
            Ok(0)
        }
    }
}
```

---

### TransactionManager

Located: `core/src/infra/transaction/manager.rs`

```rust
impl TransactionManager {
    /// Commit device-owned resource (state-based)
    pub async fn commit_device_owned<M: Syncable>(
        &self,
        library: Arc<Library>,
        model: M,
    ) -> Result<M> {
        // 1. Write to database
        let saved = model.insert(db).await?;

        // 2. Broadcast state (no log!)
        sync_service.broadcast_state(StateChange::from(&saved)).await?;

        // 3. Emit event
        event_bus.emit(Event::ResourceChanged { ... });

        Ok(saved)
    }

    /// Commit shared resource (log-based with HLC)
    pub async fn commit_shared<M: Syncable>(
        &self,
        library: Arc<Library>,
        model: M,
    ) -> Result<M> {
        // 1. Generate HLC
        let hlc = hlc_generator.lock().await.next();

        // 2. Atomic: DB + log
        let saved = db.transaction(|txn| async {
            let saved = model.insert(txn).await?;

            sync_db.append(SharedChangeEntry {
                hlc,
                model_type: M::SYNC_MODEL,
                record_uuid: saved.sync_id(),
                change_type: ChangeType::Insert,
                data: serde_json::to_value(&saved)?,
            }, txn).await?;

            Ok(saved)
        }).await?;

        // 3. Broadcast with HLC
        sync_service.broadcast_shared(entry).await?;

        // 4. Emit event
        event_bus.emit(Event::ResourceChanged { ... });

        Ok(saved)
    }
}
```

**Key**: No leader checks! All devices can write.

---

### Syncable Trait

Located: `core/src/infra/sync/syncable.rs`

```rust
pub trait Syncable {
    /// Model identifier (e.g., "location", "tag")
    const SYNC_MODEL: &'static str;

    /// Global resource ID
    fn sync_id(&self) -> Uuid;

    /// Is this device-owned or shared?
    fn is_device_owned(&self) -> bool;

    /// Owner device (if device-owned)
    fn device_id(&self) -> Option<Uuid>;

    /// Convert to JSON for sync
    fn to_sync_json(&self) -> Result<serde_json::Value>;

    /// Apply sync change (model-specific logic)
    fn apply_sync_change(
        data: serde_json::Value,
        db: &DatabaseConnection
    ) -> Result<()>;
}
```

### Model Registry

All syncable models must register themselves:

```rust
// In core/src/infra/sync/registry.rs
pub fn register_models() {
    // Device-owned models
    registry.register("location", location::registration());
    registry.register("entry", entry::registration());
    registry.register("volume", volume::registration());
    registry.register("device", device::registration());
    registry.register("audit_log", audit_log::registration());

    // Shared models
    registry.register("tag", tag::registration());
    registry.register("album", album::registration());
    registry.register("user_metadata", user_metadata::registration());

    // Junction tables (special handling)
    registry.register("album_entry", album_entry::registration());
    registry.register("user_metadata_tag", user_metadata_tag::registration());
}
```

**Examples**:

```rust
// Device-owned
impl Syncable for location::Model {
    const SYNC_MODEL: &'static str = "location";
    fn sync_id(&self) -> Uuid { self.uuid }
    fn is_device_owned(&self) -> bool { true }
    fn device_id(&self) -> Option<Uuid> { Some(self.device_id) }
}

// Shared
impl Syncable for tag::Model {
    const SYNC_MODEL: &'static str = "tag";
    fn sync_id(&self) -> Uuid { self.uuid }
    fn is_device_owned(&self) -> bool { false }
    fn device_id(&self) -> Option<Uuid> { None }
}
```

---

## Sync Messages

### StateChange (Device-Owned)

```rust
pub enum SyncMessage {
    /// Single state change
    StateChange {
        model_type: String,
        record_uuid: Uuid,
        device_id: Uuid,
        data: serde_json::Value,
        timestamp: DateTime<Utc>,
    },

    /// Batch state changes (efficiency)
    StateBatch {
        model_type: String,
        device_id: Uuid,
        records: Vec<StateRecord>,
    },

    /// Request state from peer
    StateRequest {
        model_types: Vec<String>,
        device_id: Option<Uuid>,
        since: Option<DateTime>,
    },

    /// State response
    StateResponse {
        model_type: String,
        device_id: Uuid,
        records: Vec<StateRecord>,
        has_more: bool,
    },
}
```

### SharedChange (Shared Resources)

```rust
pub enum SyncMessage {
    /// Shared resource change (with HLC)
    SharedChange {
        hlc: HLC,
        model_type: String,
        record_uuid: Uuid,
        change_type: ChangeType, // Insert, Update, Delete (or StateSnapshot)
        data: serde_json::Value,
    },

    /// Batch shared changes
    SharedChangeBatch {
        entries: Vec<SharedChangeEntry>,
    },

    /// Request shared changes since HLC
    SharedChangeRequest {
        since_hlc: Option<HLC>,
        limit: usize,
    },

    /// Shared changes response
    SharedChangeResponse {
        entries: Vec<SharedChangeEntry>,
        has_more: bool,
    },

    /// Acknowledge (for pruning)
    AckSharedChanges {
        from_device: Uuid,
        up_to_hlc: HLC,
    },
}
```

---

## Performance Characteristics

### State-Based Sync (Device-Owned)

| Metric | Value | Notes |
|--------|-------|-------|
| **Latency** | ~100ms | One network hop |
| **Disk writes** | 1 | Just database |
| **Bandwidth** | ~1KB/change | Just the record |
| **Log growth** | 0 | No log! |

### Log-Based Sync (Shared)

| Metric | Value | Notes |
|--------|-------|-------|
| **Latency** | ~150ms | Write + broadcast + ACK |
| **Disk writes** | 2 | Database + log |
| **Bandwidth** | ~2KB/change | Record + ACK |
| **Log growth** | Bounded | Pruned to <1000 entries |

### Bulk Operations (1M Entries)

| Method | Time | Size | Notes |
|--------|------|------|-------|
| Individual messages | 10 min | 500MB | Too slow |
| Batched (1K chunks) | 2 min | 50MB compressed | Good |
| Database snapshot | 30 sec | 150MB | Best for initial sync |

---

## Setup Flow

### 1. Device Pairing (Network Layer)

```bash
# Device A
$ sd-cli network pair generate
> Code: WXYZ-1234

# Device B
$ sd-cli network pair join WXYZ-1234
> Paired successfully!
```

### 2. Library Sync Setup

```bash
# Device A - Create library
$ sd-cli library create "Jamie's Library"
> Library: jamie-lib-uuid

# Device B - Discover libraries
$ sd-cli network discover-libraries <device-a-uuid>
> Found: Jamie's Library (jamie-lib-uuid)

# Device B - Setup sync
$ sd-cli network sync-setup \
    --remote-device=<device-a-uuid> \
    --remote-library=jamie-lib-uuid \
    --action=register-only
```

**What happens**:
- Device B registered in Device A's `devices` table ✅
- Device A registered in Device B's `devices` table ✅
- `sync_partners` created on both devices ✅
- No leader assignment (not needed!) ✅

### 3. Library Opens → Sync Starts

```bash
# Both devices open library
$ sd-cli library open jamie-lib-uuid

# SyncService starts automatically
# Begins broadcasting changes to peers
# Ready for real-time sync!
```

---

## Advantages Over Leader-Based

| Aspect | Benefit |
|--------|---------|
| **No bottleneck** | Any device changes anytime |
| **Offline-first** | Full functionality offline |
| **Resilient** | No single point of failure |
| **Simpler** | ~800 lines less code |
| **Faster** | No leader coordination delay |
| **Better UX** | No "leader offline" errors |

---

## Migration from v1/Old Designs

### What's Removed

- ❌ Leader election
- ❌ Heartbeat mechanism
- ❌ `sync_leadership` field
- ❌ LeadershipManager
- ❌ Central `sync_log.db`
- ❌ Follower read-only restrictions

### What's Added

- ✅ HLC generator
- ✅ Per-device `sync.db`
- ✅ State-based sync protocol
- ✅ Peer ACK tracking
- ✅ Aggressive log pruning

### Migration Path

1. Implement HLC (LSYNC-009)
2. Add state-based sync (parallel with existing)
3. Add log-based sync with HLC (parallel with existing)
4. Verify new system works
5. Remove leader-based code
6. Simplify!

---

## Error Handling and Recovery

### Partial Sync Failures

```rust
// If sync fails mid-batch
async fn handle_sync_failure(error: SyncError, checkpoint: Checkpoint) {
    match error {
        SyncError::NetworkTimeout => {
            // Save checkpoint and retry later
            checkpoint.save().await?;
            schedule_retry(checkpoint);
        }
        SyncError::ConstraintViolation(model, uuid) => {
            // Skip problematic record, continue
            mark_record_failed(model, uuid).await?;
            continue_from_next(checkpoint).await?;
        }
        SyncError::SchemaVersion(peer_version) => {
            // Peer has incompatible schema
            if peer_version > our_version {
                prompt_user_to_update();
            } else {
                // Peer needs to update
                send_schema_update_request(peer);
            }
        }
    }
}
```

### Constraint Violation Resolution

| Violation Type | Resolution Strategy |
|----------------|--------------------|
| Duplicate UUID | Use existing record (idempotent) |
| Invalid FK | Queue for retry after parent syncs |
| Unique constraint | Merge records based on model type |
| Check constraint | Log error, skip record |

### Recovery Procedures

1. **Corrupted Sync DB**: Delete and rebuild from peer state
2. **Inconsistent State**: Run integrity check, re-sync affected models
3. **Missing Dependencies**: Queue changes until dependencies arrive

---

## Testing

### Unit Tests

```rust
#[tokio::test]
async fn test_state_sync_idempotent() {
    let location = create_test_location(device_a);

    // Apply same state twice
    on_state_change(StateChange::from(&location)).await.unwrap();
    on_state_change(StateChange::from(&location)).await.unwrap();

    // Should only have one record
    let count = location::Entity::find().count(&db).await.unwrap();
    assert_eq!(count, 1);
}

#[tokio::test]
async fn test_hlc_ordering() {
    let hlc1 = HLC { timestamp: 1000, counter: 0, device_id: device_a };
    let hlc2 = HLC { timestamp: 1000, counter: 1, device_id: device_b };
    let hlc3 = HLC { timestamp: 1001, counter: 0, device_id: device_c };

    // Verify total ordering
    assert!(hlc1 < hlc2);
    assert!(hlc2 < hlc3);
}

#[tokio::test]
async fn test_log_pruning() {
    // Create 100 changes
    for i in 0..100 {
        let hlc = hlc_gen.next();
        sync_db.append(change).await.unwrap();
    }

    // All peers ACK
    for peer in peers {
        sync_db.record_ack(peer, HLC(1100, 0, device_a)).await.unwrap();
    }

    // Prune
    let pruned = sync_db.prune_acked().await.unwrap();
    assert_eq!(pruned, 100);

    // Log is empty
    let remaining = sync_db.count().await.unwrap();
    assert_eq!(remaining, 0);
}
```

### Integration Tests

```rust
#[tokio::test]
async fn test_peer_to_peer_location_sync() {
    let device_a = create_device("Device A").await;
    let device_b = create_device("Device B").await;

    // Setup sync
    setup_sync_partners(&device_a, &device_b).await;

    // Device A creates location
    let location = device_a
        .add_location("/Users/jamie/Photos")
        .await
        .unwrap();

    // Wait for sync
    tokio::time::sleep(Duration::from_millis(200)).await;

    // Verify on Device B
    let locations = device_b.get_all_locations().await.unwrap();
    assert_eq!(locations.len(), 1);
    assert_eq!(locations[0].uuid, location.uuid);
    assert_eq!(locations[0].device_id, device_a.id); // Owned by A!
}

#[tokio::test]
async fn test_concurrent_tag_creation() {
    let device_a = create_device("Device A").await;
    let device_b = create_device("Device B").await;

    setup_sync_partners(&device_a, &device_b).await;

    // Both create "Vacation" tag simultaneously
    let (tag_a, tag_b) = tokio::join!(
        device_a.create_tag("Vacation"),
        device_b.create_tag("Vacation"),
    );

    // Wait for sync
    tokio::time::sleep(Duration::from_millis(500)).await;

    // Both devices should have TWO tags (different UUIDs)
    let tags_on_a = device_a.get_all_tags().await.unwrap();
    let tags_on_b = device_b.get_all_tags().await.unwrap();

    assert_eq!(tags_on_a.len(), 2); // Both tags preserved
    assert_eq!(tags_on_b.len(), 2);
    // Tags have different UUIDs but same canonical_name
    assert_ne!(tag_a.uuid, tag_b.uuid);
}
```

---

## Troubleshooting

### Device Not Seeing Peer's Changes

**Check**:
1. Are devices in each other's `sync_partners`?
2. Is networking connected?
3. Check logs for broadcast errors
4. Verify `peer_sync_state` is updating

### Sync Log Growing Large

**Check**:
1. Are peers sending ACKs?
2. Is pruning running?
3. Check `peer_acks` table

**Expected**: <1000 entries
**If >10K**: Peers not ACKing, investigate connectivity

### Conflicts Not Resolving

**Check**:
1. HLC causality tracking working?
2. Are changes being applied in HLC order?
3. Check merge strategy for model type

---

## Future Enhancements

### Selective Sync

Allow users to choose which peer's data to sync:
```rust
sync_partners {
    remote_device_id: device_b,
    sync_enabled: true,
    sync_models: ["location", "entry"],  // Don't sync tags from B
}
```

### Bandwidth Throttling

Rate-limit broadcasts for slow connections:
```rust
sync_service.set_bandwidth_limit(1_000_000); // 1MB/sec
```

### Compression

Compress large state batches:
```rust
StateBatch { records, compression: Gzip }
```

---

## Privacy and Security Considerations

### Delete Record Privacy

The sync log presents a privacy risk for deleted sensitive data:

```rust
// Problem: Deleted tag name persists in sync log
SyncLog: [
    { hlc: 1000, type: Insert, data: { name: "Private Medical Info" } },
    { hlc: 1001, type: Delete, uuid: tag_uuid } // Name still visible above!
]
```

**Solutions**:

1. **Minimal Delete Records**: Only store UUID, not data
2. **Aggressive Pruning**: Prune delete records more aggressively than inserts
3. **State Reconciliation**: Replace deletes with periodic full state sync
4. **Encryption**: Encrypt sync log entries with library-specific key

### Sync Log Retention Policy

```rust
// Configurable retention
pub struct SyncRetentionPolicy {
    max_age_days: u32,          // Default: 7
    max_entries: usize,         // Default: 10_000
    delete_retention_hours: u32, // Default: 24 (prune deletes faster)
}
```

## Special Considerations

### Derived Data (Caches)

Several tables are **derived/computed data** that should NOT be synced directly:

#### Directory Paths Cache

The `directory_paths` table is a denormalized cache for path lookups:

```sql
CREATE TABLE directory_paths (
    entry_id INTEGER PRIMARY KEY,  -- Local FK to entries(id)
    path TEXT NOT NULL             -- Full absolute path
);
```

**Why not sync**:
- Uses local integer FK (`entry_id`) that differs per device
- Paths are device-specific (`/Users/jamie` vs `/home/jamie`)
- Can be rebuilt from entry hierarchy

**Sync strategy**:
```rust
// When receiving synced entry
async fn apply_entry_state_change(data: Value, db: &DatabaseConnection) -> Result<()> {
    let entry: entry::Model = map_and_deserialize(data, db).await?;

    // Upsert entry
    entry.upsert(db).await?;

    // If directory, rebuild its directory_path entry
    if entry.entry_kind() == EntryKind::Directory {
        let path = compute_path_from_parent_chain(&entry, db).await?;

        directory_paths::ActiveModel {
            entry_id: Set(entry.id),  // Use LOCAL id
            path: Set(path),           // Use LOCAL path
        }.upsert(db).await?;
    }

    Ok(())
}
```

**See**: `core/src/infra/sync/ENTRY_PATH_SYNC_ANALYSIS.md` for detailed analysis

#### Closure Tables

Closure tables (`entry_closure`, `tag_closure`) store transitive relationships:

```sql
CREATE TABLE entry_closure (
    ancestor_id INTEGER,    -- Local FK
    descendant_id INTEGER,  -- Local FK
    depth INTEGER,
    PRIMARY KEY (ancestor_id, descendant_id)
);
```

**Why not sync**: Uses local integer FKs, can be rebuilt from direct relationships

**Sync strategy**: Rebuild after syncing base relationships:
```rust
// After syncing entries, rebuild closure
rebuild_entry_closure_for_location(location_id, db).await?;

// After syncing tag relationships, rebuild closure
rebuild_tag_closure(db).await?;
```

### UserMetadata Dual Nature

UserMetadata can be either device-owned or shared depending on its scope:

```rust
// Entry-scoped (device-owned via entry)
UserMetadata {
    entry_uuid: Some(uuid),           // Links to specific entry
    content_identity_uuid: None,      // Not content-universal
    // Syncs with Index domain (state-based)
}

// Content-scoped (shared across devices)
UserMetadata {
    entry_uuid: None,                 // Not entry-specific
    content_identity_uuid: Some(uuid), // Content-universal
    // Syncs with UserMetadata domain (HLC-based)
}
```

### Semantic Tag Relationships

Tag relationships form a DAG and require special handling:

```rust
// Tag relationships must sync after all tags exist
// Otherwise FK constraints fail
TagRelationship {
    parent_tag_id: uuid1,  // Must exist first
    child_tag_id: uuid2,   // Must exist first
    relationship_type: "parent_child",
}

// Closure table is rebuilt locally after sync
// Not synced directly (derived data)
```

### Large Batch Transactions

SQLite has limits on transaction size. For large syncs:

```rust
// Batch into manageable chunks
const BATCH_SIZE: usize = 1000;

for chunk in entries.chunks(BATCH_SIZE) {
    let txn = db.begin().await?;

    for entry in chunk {
        entry.insert(&txn).await?;
    }

    txn.commit().await?;

    // Allow other operations between batches
    tokio::task::yield_now().await;
}
```

### Schema Version Compatibility

```rust
// Each sync message includes schema version
SyncMessage {
    schema_version: 1,  // Current schema
    // ... message data
}

// Reject sync from incompatible versions
if message.schema_version != CURRENT_SCHEMA_VERSION {
    return Err(SyncError::IncompatibleSchema);
}
```

---

## References

- **Architecture**: `core/src/infra/sync/NEW_SYNC.md`
- **Dependency Graph**: `core/src/infra/sync/docs/DEPENDENCY_GRAPH.md`
- **Implementation Guide**: `core/src/infra/sync/docs/SYNC_IMPLEMENTATION_GUIDE.md`
- **Tasks**: `.tasks/LSYNC-*.md`
- **Whitepaper**: Section 4.5.1 (Library Sync)
- **HLC Paper**: "Logical Physical Clocks" (Kulkarni et al.)
- **Design Docs**: `docs/core/design/sync/` directory

---

## Summary

Spacedrive's leaderless hybrid sync model:

1. **Device-owned data** → State broadcasts (simple, fast)
2. **Shared resources** → HLC logs (small, ordered)
3. **No leader** → No bottlenecks, true P2P
4. **Offline-first** → Queue locally, sync later
5. **Resilient** → Any peer can sync new devices

This architecture is simpler, faster, and better aligned with Spacedrive's "devices own their data" principle.

