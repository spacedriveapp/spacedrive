# Spacedrive Sync System

**Status**: Implementation Ready
**Version**: 3.0 (Leaderless)
**Last Updated**: 2025-10-08
**Architecture**: `core/src/infra/sync/NEW_SYNC.md`

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
| **Tag** | All devices | "Vacation" tag created by anyone |
| **Album** | All devices | "Summer 2024" collection with entries from multiple devices |
| **UserMetadata** | All devices (when content-scoped) | Favoriting a photo applies to the content everywhere |

**Sync Strategy**: HLC-ordered log
- Device writes to local database
- Generates HLC timestamp
- Writes to local `sync.db` log
- Broadcasts to all peers
- Peers apply in HLC order
- Log pruned when all peers ACK

**Why log needed**: Multiple devices can modify same tag/album. Need ordering for conflict resolution.

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
    name TEXT NOT NULL,
    color TEXT,
    -- NO device_id field!
);

CREATE TABLE albums (
    id INTEGER PRIMARY KEY,
    uuid TEXT NOT NULL UNIQUE,
    name TEXT NOT NULL,
    -- NO device_id field!
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
     └─ If exists: Merge (deterministic UUID prevents duplicates)
     └─ If not: INSERT INTO tags (...)
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

Phase 1: Device-Owned State Sync
  For each existing peer (A, B, C):
    Send: StateRequest {
      model_types: ["location", "entry", "volume"],
      device_id: peer.id,
      since: None  // Full sync first time
    }

    Receive: StateResponse {
      locations: [peer's locations],
      entries: [peer's entries],
      volumes: [peer's volumes]
    }

    Apply all (idempotent inserts)

  Result: Device D now has all locations/entries from all devices

Phase 2: Shared Resource Sync
  Pick any peer (say Device A):
    Send: SharedChangeRequest { since_hlc: None }
    Receive: SharedChangeResponse { entries: [...] }
    Apply in HLC order

  Fallback (if logs pruned):
    Send: StateRequest { model_types: ["tag", "album"] }
    Receive: Full tag/album list
    Insert all

  Result: Device D has all tags/albums

Phase 3: Ready
  Device D is fully synced
  Can make changes
  Becomes a peer like others
```

---

## Conflict Resolution

### No Conflicts (Device-Owned)

```rust
Device A: Creates location "/Users/jamie/Photos"
Device B: Creates location "/home/jamie/Documents"

Resolution: No conflict! Different owners.
Both apply. All devices see both locations.
```

### Deterministic Merge (Tags)

```rust
Device A: Creates tag "Vacation" → HLC(1000,A)
Device B: Creates tag "Vacation" → HLC(1001,B)

Resolution: Deterministic UUID from name
  uuid = Uuid::v5(NAMESPACE, "Vacation")
  Both generate SAME UUID
  Second creation is idempotent (already exists)

Result: One "Vacation" tag, no duplicates
```

### Union Merge (Albums)

```rust
Device A: Adds entry-1 to album → HLC(1000,A)
Device B: Adds entry-2 to album → HLC(1001,B)

Resolution: Union merge
  album.entry_uuids = [entry-1, entry-2]
  Both additions preserved

Result: Album contains both entries
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
`
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
        change_type: ChangeType,
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

    // Both devices should have ONE tag (deterministic UUID)
    let tags_on_a = device_a.get_all_tags().await.unwrap();
    let tags_on_b = device_b.get_all_tags().await.unwrap();

    assert_eq!(tags_on_a.len(), 1);
    assert_eq!(tags_on_b.len(), 1);
    assert_eq!(tags_on_a[0].uuid, tags_on_b[0].uuid); // Same UUID!
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

## References

- **Architecture**: `core/src/infra/sync/NEW_SYNC.md`
- **Tasks**: `.tasks/LSYNC-*.md`
- **Whitepaper**: Section 4.5.1 (Library Sync)
- **HLC Paper**: "Logical Physical Clocks" (Kulkarni et al.)

---

## Summary

Spacedrive's leaderless hybrid sync model:

1. **Device-owned data** → State broadcasts (simple, fast)
2. **Shared resources** → HLC logs (small, ordered)
3. **No leader** → No bottlenecks, true P2P
4. **Offline-first** → Queue locally, sync later
5. **Resilient** → Any peer can sync new devices

This architecture is simpler, faster, and better aligned with Spacedrive's "devices own their data" principle.
