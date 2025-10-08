# New Sync Architecture: Leaderless Hybrid Model

**Date**: 2025-10-08
**Status**: Proposed Architecture
**Replaces**: Leader-based sync (SYNC_DESIGN.md)

---

## Core Insight: Data Ownership Drives Sync Strategy

Spacedrive's data naturally splits into two categories with fundamentally different sync requirements:

| Category | Examples | Conflicts? | Strategy |
|----------|----------|------------|----------|
| **Device-Owned** | Locations, Entries, Volumes, Audit Logs | ❌ Never | State-Based |
| **Truly Shared** | Tags, Albums, UserMetadata (on content) | ✅ Possible | Log-Based + HLC |

**Key Principle**: Device-owned data doesn't need ordering or logs—just replicate final state. Only truly shared resources need conflict resolution via ordered logs.

---

## Architecture Overview

### No Central Leader

Every device is a peer. No leader election, no heartbeats, no single point of failure.

### Hybrid Sync Model

```
┌─────────────────────────────────────────────────────────────┐
│ Device A                                                     │
├─────────────────────────────────────────────────────────────┤
│                                                              │
│ database.db (shared state):                                 │
│   locations: [A's locations, B's locations, C's locations]  │
│   entries: [A's entries, B's entries, C's entries]          │
│   tags: [all tags from all devices]                         │
│   albums: [all albums from all devices]                     │
│                                                              │
│ sync.db (MY shared changes only):                           │
│   HLC(1000,A): Created tag "Vacation"                       │
│   HLC(1050,A): Created album "Summer 2024"                  │
│   HLC(1100,A): Tagged content-123 as "favorite"             │
│   # Pruned once all peers ack                               │
│                                                              │
│ peer_ack_state:                                             │
│   Device B: last_acked = HLC(1050,A)  ← B has my changes up to 1050
│   Device C: last_acked = HLC(1100,A)  ← C has all my changes
│                                                              │
└─────────────────────────────────────────────────────────────┘
```

---

## Data Model Classification

### Device-Owned Data (State-Based Sync)

#### Locations
```rust
location {
    uuid: Uuid,
    device_id: Uuid,  // ← OWNER
    path: String,
    name: String,
}

// Sync strategy: State broadcast
Device A creates → Broadcast current state → Peers insert
No log, no ordering, no conflicts
```

#### Entries
```rust
entry {
    uuid: Uuid,
    location_id: Uuid → device_id,  // ← OWNED via location
    name: String,
    size: i64,
}

// Sync strategy: State broadcast per-device
Device A indexes 1000 files → Broadcast entry list → Peers insert
Can batch: "Here are 1000 entries from Device A"
```

#### Volumes
```rust
volume {
    uuid: Uuid,
    device_id: Uuid,  // ← OWNER
    name: String,
    mount_point: String,
}

// Sync strategy: State broadcast
```

#### Audit Logs
```rust
audit_entry {
    uuid: Uuid,
    device_id: Uuid,  // ← THIS DEVICE'S ACTION
    action: String,
    timestamp: DateTime,
}

// Sync strategy: State broadcast
Each device broadcasts its own audit entries
No conflicts (I can't create your audit entry)
All devices collect all audit entries for full history
```

#### Devices Table
```rust
device {
    uuid: Uuid,
    name: String,
    os: String,
}

// Sync strategy: Each device broadcasts its OWN device record
Device A updates its name → Broadcast → Peers update
```

---

### Truly Shared Data (Log-Based Sync)

#### Tags (Definitions)
```rust
tag {
    uuid: Uuid,       // Deterministic from name hash?
    name: String,
    color: String,
}

// Conflict scenario:
Device A creates tag "Vacation" at HLC(1000,A)
Device B creates tag "Vacation" at HLC(1001,B)

// Resolution:
Both create, deterministic UUID prevents duplicate
OR: Name collision detected, merge via HLC ordering
```

#### Albums
```rust
album {
    uuid: Uuid,
    name: String,
    entry_uuids: Vec<Uuid>,  // References entries from multiple devices
}

// Conflict scenario:
Device A: Adds entry-1 to album at HLC(1000,A)
Device B: Adds entry-2 to album at HLC(1001,B)

// Resolution:
Union merge, album contains both entries
```

#### UserMetadata (on Content)
```rust
user_metadata {
    uuid: Uuid,
    content_identity_uuid: Uuid,  // ← Content, not device-specific
    notes: String,
    favorite: bool,
}

// Conflict scenario:
Device A: Favorites photo at HLC(1000,A)
Device B: Un-favorites photo at HLC(1001,B)

// Resolution:
HLC ordering: B's change wins (later timestamp)
```

---

## Sync Protocol Design

### For Device-Owned Data (State-Based)

#### On Change
```rust
// Device A creates location
async fn create_location(path: &str) -> Result<Location> {
    let location = location::ActiveModel {
        uuid: Set(Uuid::new_v4()),
        device_id: Set(MY_DEVICE_ID),
        path: Set(path.to_string()),
        created_at: Set(Utc::now()),
        updated_at: Set(Utc::now()),
    };

    // 1. Write to local database
    let saved = location.insert(db).await?;

    // 2. Broadcast state (no log!)
    broadcast_to_peers(StateChange {
        model_type: "location",
        record_uuid: saved.uuid,
        device_id: MY_DEVICE_ID,
        data: serde_json::to_value(&saved)?,
        timestamp: Utc::now(),
    }).await?;

    // 3. Done! No sync log write.

    Ok(saved.into())
}
```

#### On Receive
```rust
// Peer receives state change
async fn on_state_change(change: StateChange) {
    // Idempotent: upsert based on UUID
    match change.model_type.as_str() {
        "location" => {
            let location: location::Model = serde_json::from_value(change.data)?;

            // Insert or update
            location::ActiveModel::from(location)
                .insert_or_update(db)
                .await?;

            // Emit event for UI
            event_bus.emit(Event::LocationSynced { uuid: change.record_uuid });
        }
        "entry" => { /* similar */ }
        // ...
    }
}
```

#### New Device Joins
```rust
// Device D joins library
async fn initial_sync_device_owned() {
    // For each peer
    for peer in peers {
        // Request their state
        let request = StateSyncRequest {
            model_types: vec!["location", "entry", "volume"],
            device_id: peer.device_id,  // "Give me YOUR data"
        };

        let response = peer.send(request).await?;

        // Response is just a list of records
        for change in response.records {
            on_state_change(change).await?;
        }
    }

    // No log replay needed!
}
```

---

### For Shared Data (Log-Based with HLC)

#### Hybrid Logical Clock (HLC)

```rust
#[derive(Debug, Clone, Copy, Serialize, Deserialize, Ord, PartialOrd, Eq, PartialEq)]
pub struct HLC {
    /// Milliseconds since epoch (physical time)
    pub timestamp: u64,

    /// Logical counter for events in same millisecond
    pub counter: u64,

    /// Device that generated this clock
    pub device_id: Uuid,
}

impl HLC {
    /// Generate new HLC (increments from last)
    pub fn generate(last: Option<HLC>, device_id: Uuid) -> Self {
        let now = Utc::now().timestamp_millis() as u64;

        match last {
            Some(last) if last.timestamp == now => {
                // Same millisecond, increment counter
                Self {
                    timestamp: now,
                    counter: last.counter + 1,
                    device_id,
                }
            }
            _ => {
                // New millisecond
                Self {
                    timestamp: now,
                    counter: 0,
                    device_id,
                }
            }
        }
    }

    /// Update based on received HLC (causality tracking)
    pub fn update(&mut self, received: HLC) {
        // Take max of local and received timestamp
        self.timestamp = self.timestamp.max(received.timestamp);

        // If same timestamp, increment counter
        if self.timestamp == received.timestamp {
            self.counter = self.counter.max(received.counter) + 1;
        }
    }
}

// Total ordering: timestamp, then counter, then device_id
// This gives us a consistent global order!
```

#### On Change
```rust
// Device A creates tag
async fn create_tag(name: &str) -> Result<Tag> {
    let tag = tag::ActiveModel {
        uuid: Set(Uuid::new_v4()),
        name: Set(name.to_string()),
        created_at: Set(Utc::now()),
    };

    // 1. Write to local database
    let saved = tag.insert(db).await?;

    // 2. Generate HLC
    let hlc = my_hlc_generator.next();

    // 3. Write to MY sync log
    let entry = SharedChangeEntry {
        hlc,
        model_type: "tag",
        record_uuid: saved.uuid,
        change_type: ChangeType::Insert,
        data: serde_json::to_value(&saved)?,
    };

    sync_db.append(entry.clone()).await?;

    // 4. Broadcast to all peers
    broadcast_to_peers(entry).await?;

    // 5. Done!

    Ok(saved.into())
}
```

#### On Receive
```rust
// Peer receives shared change
async fn on_shared_change(entry: SharedChangeEntry) {
    // 1. Update our HLC (causality tracking)
    my_hlc_generator.update(entry.hlc);

    // 2. Insert into OUR copy of sender's log
    // (We track what changes each peer has made)
    peer_changes_db.insert(entry.clone()).await?;

    // 3. Apply to database (with conflict resolution)
    apply_shared_change(entry).await?;

    // 4. Send ACK to sender
    send_ack(entry.hlc, entry.device_id).await?;
}

async fn apply_shared_change(entry: SharedChangeEntry) {
    match entry.change_type {
        ChangeType::Insert => {
            // Check if exists (by UUID)
            if exists(entry.record_uuid).await? {
                // Conflict! Merge or dedupe
                merge_conflict(entry).await?;
            } else {
                // New record, insert
                insert_from_entry(entry).await?;
            }
        }
        ChangeType::Update => {
            // Load local version
            let local = load(entry.record_uuid).await?;

            // Merge (union for tags, LWW for others)
            let merged = merge(local, entry)?;

            // Update database
            merged.update(db).await?;
        }
        ChangeType::Delete => {
            delete(entry.record_uuid).await?;
        }
    }
}
```

#### Pruning (Keep Logs Small)

```rust
// After receiving ACK
async fn on_ack_received(from_device: Uuid, up_to_hlc: HLC) {
    // Update ack tracking
    peer_acks.insert(from_device, up_to_hlc);

    // Check if ALL peers have acked up to a point
    let min_acked_hlc = peer_acks.values().min();

    if let Some(min_hlc) = min_acked_hlc {
        // Prune entries that all peers have
        sync_db
            .delete_where(hlc <= min_hlc)
            .await?;

        info!(
            pruned_up_to = ?min_hlc,
            "Pruned shared changes log"
        );
    }
}
```

#### New Device Joins
```rust
// Device D joins library
async fn initial_sync_shared() {
    // Pick any peer (they all have the same shared state eventually)
    let peer = peers.first();

    // 1. Get all shared changes that haven't been fully acked yet
    let unacked_changes = peer.get_shared_changes_log().await?;

    // 2. Apply in HLC order
    unacked_changes.sort_by_key(|e| e.hlc);
    for entry in unacked_changes {
        on_shared_change(entry).await?;
    }

    // 3. Get current state of shared resources (in case logs pruned)
    let current_tags = peer.get_all_tags().await?;
    let current_albums = peer.get_all_albums().await?;

    // 4. Insert locally
    for tag in current_tags {
        tag.insert_or_ignore(db).await?;
    }

    // Now fully synced!
}
```

---

## Database Structure

### Per-Library Database (database.db)

```sql
-- Device-owned (state replicated)
CREATE TABLE locations (
    id INTEGER PRIMARY KEY,
    uuid UUID NOT NULL UNIQUE,
    device_id UUID NOT NULL,  -- Owner device
    path TEXT NOT NULL,
    -- ... other fields
);

CREATE TABLE entries (
    id INTEGER PRIMARY KEY,
    uuid UUID NOT NULL UNIQUE,
    location_id INTEGER → device_id,  -- Owner via location
    name TEXT NOT NULL,
    -- ... other fields
);

-- Truly shared (log replicated)
CREATE TABLE tags (
    id INTEGER PRIMARY KEY,
    uuid UUID NOT NULL UNIQUE,
    name TEXT NOT NULL,
    color TEXT,
    -- NO device_id - shared resource
);

CREATE TABLE albums (
    id INTEGER PRIMARY KEY,
    uuid UUID NOT NULL UNIQUE,
    name TEXT NOT NULL,
    -- NO device_id - shared resource
);

-- Devices (special: each device broadcasts its own record)
CREATE TABLE devices (
    id INTEGER PRIMARY KEY,
    uuid UUID NOT NULL UNIQUE,
    name TEXT NOT NULL,
    sync_leadership TEXT,  -- Can remove! No leader needed
);

-- Sync partners (who we sync with)
CREATE TABLE sync_partners (
    id INTEGER PRIMARY KEY,
    remote_device_id UUID NOT NULL UNIQUE,
    sync_enabled BOOLEAN DEFAULT true,
    last_sync_at TIMESTAMP,
);

-- Track what we've received from each peer
CREATE TABLE peer_sync_state (
    device_id UUID PRIMARY KEY,
    last_device_owned_sync TIMESTAMP,    -- Last time we synced their state
    last_shared_change_hlc TEXT,         -- Last HLC we received from them
);
```

### Per-Device Shared Changes Log (sync.db)

```sql
-- Only MY changes to shared resources
CREATE TABLE shared_changes (
    hlc TEXT PRIMARY KEY,  -- Hybrid Logical Clock (sortable)
    model_type TEXT NOT NULL,
    record_uuid UUID NOT NULL,
    change_type TEXT NOT NULL,  -- insert/update/delete
    data TEXT NOT NULL,         -- JSON payload
    created_at TIMESTAMP NOT NULL,
);

CREATE INDEX idx_shared_changes_hlc ON shared_changes(hlc);
CREATE INDEX idx_shared_changes_model ON shared_changes(model_type);

-- Track which peers have acked which HLCs
CREATE TABLE peer_acks (
    peer_device_id UUID NOT NULL,
    last_acked_hlc TEXT NOT NULL,
    acked_at TIMESTAMP NOT NULL,
    PRIMARY KEY (peer_device_id)
);
```

**Size**: Stays small! Entries pruned once all peers ack. Typically <1000 entries even with heavy use.

---

## Sync Protocol Messages

### StateChange (Device-Owned Data)

```rust
#[derive(Serialize, Deserialize)]
pub enum SyncMessage {
    /// Broadcast current state of device-owned resource
    StateChange {
        model_type: String,        // "location", "entry", "volume"
        record_uuid: Uuid,
        device_id: Uuid,           // Owner device
        data: serde_json::Value,   // Full record
        timestamp: DateTime<Utc>,
    },

    /// Batch state changes (for efficiency)
    StateBatch {
        model_type: String,
        device_id: Uuid,
        records: Vec<StateRecord>,
    },

    /// Request full state from peer
    StateRequest {
        model_types: Vec<String>,
        device_id: Option<Uuid>,   // Specific device or all
        since: Option<DateTime>,   // Incremental sync
    },

    /// Response with state
    StateResponse {
        model_type: String,
        device_id: Uuid,
        records: Vec<StateRecord>,
        has_more: bool,
    },

    /// Broadcast shared resource change (with HLC)
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

    /// Response with shared changes
    SharedChangeResponse {
        entries: Vec<SharedChangeEntry>,
        has_more: bool,
    },

    /// Acknowledge received shared changes
    AckSharedChanges {
        from_device: Uuid,
        up_to_hlc: HLC,
    },
}
```

---

## Sync Flows

### Flow 1: Device A Creates Location (State-Based)

```
Device A:
  1. INSERT INTO locations (device_id=A, ...)
  2. Broadcast StateChange to all sync_partners
  3. Done! (no log write)

Device B, C:
  1. Receive StateChange
  2. INSERT INTO locations (device_id=A, ...)
  3. Emit Event::LocationSynced
  4. UI updates!
```

**Latency**: ~100ms (one network hop)
**Disk Writes**: 1 (just database, no log)

---

### Flow 2: Device A Tags Photo (Log-Based)

```
Device A:
  1. Generate HLC(1000,A)
  2. INSERT INTO user_metadata (...)
  3. INSERT INTO shared_changes (hlc=HLC(1000,A), ...)
  4. Broadcast SharedChange to all sync_partners
  5. Wait for ACKs (background)

Device B:
  1. Receive SharedChange
  2. Update my_hlc.update(HLC(1000,A))
  3. INSERT INTO user_metadata (...) [may merge with existing]
  4. Send ACK to Device A
  5. Emit Event::TagSynced

Device A (later):
  1. Receive ACK from Device B: up_to_hlc=HLC(1000,A)
  2. Receive ACK from Device C: up_to_hlc=HLC(1000,A)
  3. All peers acked → DELETE FROM shared_changes WHERE hlc <= HLC(1000,A)
  4. Log stays small!
```

**Latency**: ~100ms (one hop + ack)
**Disk Writes**: 2 (database + log), then pruned
**Log Size**: Only unacked entries (typically <100)

---

### Flow 3: New Device Joins

```
Device D connects to library:

Phase 1: Sync Device-Owned State
  For each peer (A, B, C):
    Request: StateRequest { device_id: peer.id }
    Receive: StateResponse {
      locations: [...],
      entries: [...],
      volumes: [...]
    }
    Apply state (idempotent inserts)

Phase 2: Sync Shared Resources
  Pick any peer (say Device A):
    Request: SharedChangeRequest { since_hlc: None }
    Receive: SharedChangeResponse {
      entries: [all unacked changes]
    }
    Apply in HLC order

  Fallback: Get current shared state
    Request: StateRequest { model_types: ["tag", "album"] }
    Receive full tag/album lists
    Insert into database

Phase 3: Ready!
  Now Device D is fully synced
  Can make changes
  Broadcasts to peers
```

---

## Comparison: Leader vs Leaderless

| Aspect | Leader-Based (Old) | Leaderless (New) |
|--------|-------------------|------------------|
| **Bottleneck** | ❌ Leader must process all changes | ✅ Each device independent |
| **Offline operation** | ❌ Followers read-only when leader offline | ✅ All devices can make changes |
| **Complexity** | ❌ Leader election, heartbeats, failover | ✅ No election needed |
| **Single point of failure** | ❌ Leader down = no changes | ✅ No single point of failure |
| **Sync log** | 1 central log (all changes) | N small logs (shared changes only) |
| **New device join** | Pull from leader only | Pull state from any peer |
| **Index sync** | Goes through leader unnecessarily | Direct peer-to-peer |
| **Shared metadata** | Goes through leader | Peer-to-peer with HLC |
| **Code complexity** | Medium (leader logic) | Low (simpler) |

---

## Implementation Changes

### What Stays

- ✅ `sync_partners` table (still need to track who we sync with)
- ✅ `SyncProtocolHandler` (still need messaging)
- ✅ `TransactionManager` concept (atomic writes + events)
- ✅ Domain separation (Index vs UserMetadata)
- ✅ Event-driven architecture

### What Changes

- ❌ **Remove**: `sync_leadership` field from devices
- ❌ **Remove**: `LeadershipManager`
- ❌ **Remove**: Leader election logic
- ❌ **Remove**: Single `sync_log.db` on leader
- ✅ **Add**: `shared_changes.db` per device (small, prunable)
- ✅ **Add**: HLC generator per device
- ✅ **Add**: State-based sync for device-owned data
- ✅ **Add**: Peer ack tracking for pruning

### What Simplifies

```rust
// OLD: Check if leader before writing
if !is_leader(library_id) {
    return Err("Not leader");
}

// NEW: Just write!
async fn create_tag(name: &str) -> Result<Tag> {
    // Always allowed!
    tag.insert(db).await?;
    log_and_broadcast(tag).await?;
    Ok(tag)
}
```

---

## Edge Cases & Solutions

### Problem: Two Devices Create Same Tag Name

```
Device A: Creates "Vacation" → HLC(1000,A)
Device B: Creates "Vacation" → HLC(1001,B)
```

**Solution 1**: Deterministic UUID from name
```rust
tag.uuid = Uuid::new_v5(NAMESPACE, tag.name);
// Both devices generate SAME UUID
// When synced, they merge (same UUID = same tag)
```

**Solution 2**: Keep both, user resolves
```rust
// Two different UUIDs
// UI shows: "You have duplicate tags: 'Vacation' (2)"
// User can merge manually
```

**Recommendation**: Solution 1 for tags (names should be unique anyway)

---

### Problem: Device Offline for Days

```
Device A offline for 3 days
Device B, C continue making changes
Device A comes back online
```

**Solution**:
```rust
// Device A reconnects
let last_sync = load_last_sync_time(); // 3 days ago

// Sync device-owned state (quick)
for peer in [B, C] {
    request_state_since(peer, last_sync).await?;
    // Only changed locations/entries
}

// Sync shared changes (from logs)
for peer in [B, C] {
    let my_last_hlc_from_peer = get_last_hlc(peer);
    request_shared_changes_since(peer, my_last_hlc_from_peer).await?;
}

// Caught up!
```

---

### Problem: All Devices Offline, Then Sync

```
Device A, B, C all offline
Each makes local changes
All come online at once
```

**Solution**: Gossip protocol
```rust
// Each device broadcasts its state
Device A → B, C: "I have HLC(2000,A) for shared changes"
Device B → A, C: "I have HLC(1500,B) for shared changes"
Device C → A, B: "I have HLC(1800,C) for shared changes"

// Each compares
Device A sees:
  - B has changes I don't have (HLC(1500,B) > my last from B)
  - C has changes I don't have (HLC(1800,C) > my last from C)

// Device A requests
request_shared_changes(B, since=my_last_hlc_from_b).await;
request_shared_changes(C, since=my_last_hlc_from_c).await;

// Apply in HLC order
// Converge to same state!
```

---

## Why This Is Better

### 1. Matches the Architecture

Spacedrive's key insight: **"Devices own their filesystem indices"**

The leaderless model directly embodies this:
- Device A syncs Device A's data (simple broadcast)
- Device B syncs Device B's data (simple broadcast)
- Shared metadata (rare) uses logs + HLC

### 2. Offline-First By Design

- Any device can make changes anytime
- Changes queue locally
- Sync when reconnected
- No "waiting for leader" frustration

### 3. Simpler Implementation

**Remove**:
- Leader election logic (~500 lines)
- Heartbeat system (~200 lines)
- Failover logic (~300 lines)
- Complex role management (~200 lines)

**Add**:
- HLC generator (~100 lines)
- Peer ack tracking (~100 lines)
- State-based sync (~300 lines)

**Net**: ~800 lines simpler!

### 4. Better UX

**Old**: "You can't tag this photo because the leader device is offline"
**New**: "Tagged! Will sync when devices reconnect"

---

## Migration Path from Current Code

### Phase 1: Add State-Based Sync (Parallel)

1. Implement state-based sync for locations/entries
2. Keep existing leader-based sync running
3. Devices use both systems
4. Verify state-based works

### Phase 2: Add HLC for Shared

1. Implement HLC generator
2. Implement `shared_changes.db` per device
3. Use for tags/albums
4. Parallel with leader system

### Phase 3: Remove Leader

1. Stop using central `sync_log.db`
2. Remove leadership checks
3. Remove election logic
4. Simplify!

---

## Open Questions

### Q1: Do we need ANY log for device-owned data?

**Answer**: Only if we want efficient delta sync.

**With timestamps**:
```sql
SELECT * FROM locations
WHERE device_id = 'peer-id'
  AND updated_at > :last_sync_time
```

This gives us changed locations without a log!

**Verdict**: No log needed if we add `updated_at` to all models (already have this!)

### Q2: How big would sync.db get?

**Estimate**: 10 changes/day/device × 3 devices × 7 days until all ack = ~210 entries

Each entry: ~500 bytes → ~100KB total

**Verdict**: Tiny! Negligible overhead.

### Q3: What about initial backfill of 1M entries?

**Batch approach**:
```rust
// Request in chunks
for offset in (0..1_000_000).step_by(10_000) {
    let batch = peer.get_entries_batch(offset, 10_000).await?;
    insert_batch(batch).await?;
}
```

Or use existing database replication:
```rust
// Just copy their database.db as starting point!
// Then sync delta
```

---

## Conclusion

**Your intuition is correct**: A leaderless model is simpler and better aligned with Spacedrive's architecture.

**The key insight**:
- Device-owned data = state-based (no log)
- Shared resources = log-based with HLC (small, prunable)

**Benefits**:
- ✅ No leader bottleneck
- ✅ Works offline
- ✅ Simpler code
- ✅ More resilient
- ✅ Matches architecture

**This is a significant architectural improvement!**

Should we explore this further and create a migration plan? Or do you see issues I'm missing?
