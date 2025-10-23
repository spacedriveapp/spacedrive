---
id: LSYNC-020
title: Device-Owned Deletion Sync via Cascading Tombstones
status: To Do
assignee: james
priority: High
tags: [sync, core, bug-fix, vdfs]
last_updated: 2025-10-22
related_tasks: []
---

# Device-Owned Deletion Sync via Cascading Tombstones

## Problem Statement

When a device deletes device-owned data (locations, entries, volumes), other devices in the library never learn about the deletion. This leaves stale data on remote devices, violating the **Virtual Distributed Filesystem (VDFS)** guarantee that all devices have a completely up-to-date view of filesystem data.

### Current Behavior

```
Device A: Deletes location "Photos" (with 10,000 entry children)
    ↓
Device A: Location removed from local DB (cascade deletes entries via FK)
    ↓
Device B: Still shows "Photos" location and all 10,000 entries
Device C: Still shows "Photos" location and all 10,000 entries
    ↓
Library state is permanently inconsistent 
VDFS contract violated 
```

### Root Cause

State-based sync for device-owned data only broadcasts **updates and creates**, not deletions. The docs mention "absence detection" but this was never implemented:

> **Device-owned**: Stop broadcasting. Others detect absence and remove.
> — `docs/core/library-sync.mdx:442`

There is no mechanism for peers to detect absence.

## Design Goals

1. **VDFS Consistency** - All devices maintain accurate filesystem view (critical requirement)
2. **Scalable** - Handle high-volume entry deletions (thousands per day)
3. **Efficient** - Minimal storage and network overhead
4. **Simple** - Leverage existing tree structure, minimal protocol changes
5. **Reliable** - Works for online and offline devices (up to 30 days)

## Proposed Solution: Cascading Tombstones

Track deletions using **tombstones** that leverage the existing `entry_closure` tree structure for automatic cascade deletion on receiving devices.

### Key Insight: Leverage Tree Structure

Entries already form a tree via `entry_closure`. When we delete a folder with 10,000 files:
- **Traditional approach**: Create 10,000 tombstones (one per entry)
- **Cascading approach**: Create 1 tombstone (just the root), receiving device cascades automatically

**Compression ratio: 10,000:1** 

### High-Level Flow

```
Device A: User deletes folder with 10,000 entries
    ↓
Device A: Executes delete_subtree() in transaction:
  - DELETE entries (10,000 rows)
  - INSERT tombstone (1 row for root UUID only)
    ↓
Device B: Reconnects, sends StateRequest with watermark
    ↓
Device A: Responds with deleted_uuids: [folder_uuid]
    ↓
Device B: Receives folder_uuid deletion
    ↓
Device B: Looks up folder by UUID, calls delete_subtree()
    ↓
Device B: Cascade deletes all 10,000 children automatically
    ↓
Result: VDFS consistency restored ✅
```

## Technical Design

### Architecture: Unified Sync Coordination

This design unifies pruning behavior across both sync mechanisms:

```
Shared Resources (sync.db):
- shared_changes table → Peer log with HLC ordering
- peer_acks table → Tracks what each peer has seen
- Pruning: Delete entries where all peers acked (min HLC)

Device-Owned Deletions (library.db):
- device_state_tombstones table → Deletion tracking
- devices.last_state_watermark → Tracks what each device has seen
- Pruning: Delete tombstones where all devices synced (min watermark)

Both use "minimum of all devices" pattern!
Both run in same hourly pruning task!
```

**Why tombstones stay in library.db:**
- Atomic transactions (create entries + tombstones together)
- Foreign key constraints work (to devices table)
- No coupling - indexer/watcher don't need sync.db access
- Simpler error handling (single database transaction)

**Why pruning is unified:**
- Same conceptual model (ack-based pruning)
- One configuration point for retention policy
- Easier to understand and maintain
- Both databases stay minimal

### Schema Changes

#### Single Table: `device_state_tombstones`

```sql
CREATE TABLE device_state_tombstones (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    model_type TEXT NOT NULL,           -- "location", "entry", "volume"
    record_uuid TEXT NOT NULL,          -- UUID of deleted record (root only!)
    device_id INTEGER NOT NULL,         -- Owner device (FK to devices.id)
    deleted_at TIMESTAMP NOT NULL,      -- When deletion occurred

    UNIQUE(model_type, record_uuid, device_id),

    FOREIGN KEY (device_id) REFERENCES devices(id) ON DELETE CASCADE
);

CREATE INDEX idx_tombstones_lookup
    ON device_state_tombstones(model_type, device_id, deleted_at);
```

**Storage characteristics with cascading + ack-based pruning:**
```
Normal operation (all devices online, hourly pruning):
- Tombstones live ~1 hour max
- Typical size: 10-50 rows (~5 KB)

One device offline for 3 days:
- Tombstones accumulate for 3 days
- Size: ~200-300 rows (~30 KB)

One device offline for 7+ days:
- Safety limit kicks in (7-day max retention)
- Size: ~500-800 rows (~100 KB)
- Offline device does full sync on reconnection

Worst case (broken watermarks, 30-day fallback):
- Size: ~113 tombstones (~17 KB)

Without cascading would be:
- Entries: 100 + 10,000 = 10,100 tombstones
- Storage: ~1.5 MB
- Compression: 89:1 with cascading! 
```

### Protocol Changes

#### Update `SyncMessage::StateResponse`

```rust
// core/src/service/network/protocol/sync/messages.rs

StateResponse {
    library_id: Uuid,
    model_type: String,
    device_id: Uuid,
    records: Vec<StateRecord>,

    // NEW: Root UUIDs of deleted trees (cascade on receiver)
    deleted_uuids: Vec<Uuid>,

    checkpoint: Option<String>,
    has_more: bool,
}
```

**Backward compatibility:** Older clients ignore `deleted_uuids` field (graceful degradation).

### Core Implementation

#### 1. Entry Deletion with Cascading Tombstone

Entry deletions happen in two places:
- **File watcher** (`watcher/worker.rs` → `responder::handle_remove()`)
- **Change detector** (indexer re-scan finds deleted files)

Both flow through the same atomic deletion:

```rust
// core/src/ops/indexing/responder.rs

async fn delete_subtree(ctx: &impl IndexingCtx, entry_id: i32) -> Result<()> {
    let txn = ctx.library_db().begin().await?;

    // 1. Get ROOT entry UUID before deletion (this is what we'll tombstone)
    let root_uuid: Uuid = entities::entry::Entity::find_by_id(entry_id)
        .select_only()
        .column(Column::Uuid)
        .into_tuple()
        .one(&txn)
        .await?
        .ok_or_else(|| anyhow::anyhow!("Entry not found"))?;

    // 2. Find all descendants (existing logic)
    let mut to_delete_ids: Vec<i32> = vec![entry_id];
    if let Ok(rows) = entities::entry_closure::Entity::find()
        .filter(entities::entry_closure::Column::AncestorId.eq(entry_id))
        .all(&txn)
        .await
    {
        to_delete_ids.extend(rows.into_iter().map(|r| r.descendant_id));
    }
    to_delete_ids.sort_unstable();
    to_delete_ids.dedup();

    // 3. Delete entries (existing logic)
    if !to_delete_ids.is_empty() {
        let _ = entities::entry_closure::Entity::delete_many()
            .filter(entities::entry_closure::Column::DescendantId.is_in(to_delete_ids.clone()))
            .exec(&txn)
            .await;
        let _ = entities::entry_closure::Entity::delete_many()
            .filter(entities::entry_closure::Column::AncestorId.is_in(to_delete_ids.clone()))
            .exec(&txn)
            .await;
        let _ = entities::directory_paths::Entity::delete_many()
            .filter(entities::directory_paths::Column::EntryId.is_in(to_delete_ids.clone()))
            .exec(&txn)
            .await;
        let _ = entities::entry::Entity::delete_many()
            .filter(entities::entry::Column::Id.is_in(to_delete_ids))
            .exec(&txn)
            .await;
    }

    // 4. NEW: Create tombstone for ROOT only (children cascade on receiver)
    entities::device_state_tombstone::Entity::insert(ActiveModel {
        id: NotSet,
        model_type: Set("entry".to_string()),
        record_uuid: Set(root_uuid),  // Just the top-level UUID!
        device_id: Set(ctx.device_id()),
        deleted_at: Set(chrono::Utc::now()),
    })
    .on_conflict(
        OnConflict::columns(vec![
            Column::ModelType,
            Column::RecordUuid,
            Column::DeviceId,
        ])
        .do_nothing()  // Idempotent
        .to_owned(),
    )
    .exec(&txn)
    .await?;

    txn.commit().await?;
    Ok(())
}
```

**Key points:**
- Delete 10,000 entries → Create 1 tombstone
- Atomic transaction (all-or-nothing)
- Works for files (no children) and folders (many children)

**Note:** We'll need two variants of `delete_subtree()`:
- `delete_subtree()` - Creates tombstones (used by watcher/indexer)
- `delete_subtree_internal()` - No tombstones (used by `apply_deletion` to avoid recursion)

#### 2. Location Deletion with Tombstone

```rust
// core/src/location/manager.rs

pub async fn remove_location(
    &self,
    library: &Library,
    location_id: Uuid,
) -> LocationResult<()> {
    info!("Removing location {}", location_id);

    // Find location
    let location = entities::location::Entity::find()
        .filter(entities::location::Column::Uuid.eq(location_id))
        .one(library.db().conn())
        .await?
        .ok_or_else(|| LocationError::LocationNotFound { id: location_id })?;

    // Delete root entry (cascades to all child entries via delete_subtree)
    // This creates tombstones for the entry tree
    if let Some(entry_id) = location.entry_id {
        // Call delete_subtree which creates entry tombstone
        crate::ops::indexing::responder::delete_subtree_by_id(entry_id, library.db()).await?;
    }

    // Delete location record
    entities::location::Entity::delete_by_id(location.id)
        .exec(library.db().conn())
        .await?;

    // Create tombstone for location
    entities::device_state_tombstone::Entity::insert(ActiveModel {
        id: NotSet,
        model_type: Set("location".to_string()),
        record_uuid: Set(location_id),
        device_id: Set(location.device_id),
        deleted_at: Set(chrono::Utc::now()),
    })
    .on_conflict(
        OnConflict::columns(vec![
            Column::ModelType,
            Column::RecordUuid,
            Column::DeviceId,
        ])
        .do_nothing()
        .to_owned(),
    )
    .exec(library.db().conn())
    .await?;

    // Emit event
    self.events.emit(Event::LocationRemoved {
        library_id: library.id(),
        location_id,
    });

    info!("Successfully removed location {}", location_id);
    Ok(())
}
```

#### 3. Querying Tombstones in StateRequest Handler

```rust
// core/src/service/sync/protocol_handler.rs

async fn handle_state_request(&self, req: StateRequest) -> Vec<StateResponse> {
    let mut responses = Vec::new();

    for model_type in req.model_types {
        // 1. Query regular records (existing logic)
        let results = registry::query_for_sync(
            model_type.clone(),
            req.device_id,
            req.since,
            req.checkpoint,
            req.batch_size,
            db,
        ).await?;

        // 2. NEW: Query tombstones if incremental sync
        let deleted_uuids = if let Some(since) = req.since {
            let mut query = entities::device_state_tombstone::Entity::find()
                .filter(Column::ModelType.eq(&model_type))
                .filter(Column::DeletedAt.gte(since));

            // Filter by device if specified
            if let Some(device_id) = req.device_id {
                let device_local_id = map_device_uuid_to_local_id(device_id, db).await?;
                query = query.filter(Column::DeviceId.eq(device_local_id));
            }

            query.all(db)
                .await?
                .into_iter()
                .map(|t| t.record_uuid)
                .collect()
        } else {
            vec![]  // Full sync doesn't need tombstones
        };

        responses.push(StateResponse {
            library_id: req.library_id,
            model_type,
            device_id: req.device_id.unwrap_or_default(),
            records: results.records,
            deleted_uuids,  // Root UUIDs only!
            checkpoint: results.next_checkpoint,
            has_more: results.len() == batch_size,
        });
    }

    responses
}
```

#### 4. Extend Syncable Trait for Deletions

Following the existing library sync pattern, deletion logic belongs in the `Syncable` trait:

```rust
// core/src/infra/sync/syncable.rs

pub trait Syncable: Serialize + Clone {
    const SYNC_MODEL: &'static str;

    fn sync_id(&self) -> Uuid;
    fn version(&self) -> i64;

    // ... existing methods ...

    /// NEW: Apply a deletion by UUID
    ///
    /// This is called when a tombstone is received during sync.
    /// The implementation should:
    /// 1. Find the record by UUID
    /// 2. Delete it (with any necessary cascades)
    /// 3. Be idempotent (no-op if already deleted)
    ///
    /// # Parameters
    /// - `uuid`: The UUID of the record to delete
    /// - `db`: Database connection
    fn apply_deletion(
        uuid: Uuid,
        db: &DatabaseConnection,
    ) -> impl std::future::Future<Output = Result<(), sea_orm::DbErr>> + Send
    where
        Self: Sized,
    {
        async move {
            // Default: no-op (models must override if they support deletion sync)
            Ok(())
        }
    }

    /// NEW: Check if a record is tombstoned (deleted)
    ///
    /// Used during apply_state_change to prevent re-creating deleted records.
    /// This handles the race condition where a deletion tombstone arrives
    /// before or during backfill of the record itself.
    async fn is_tombstoned(
        uuid: Uuid,
        db: &DatabaseConnection,
    ) -> Result<bool, sea_orm::DbErr> {
        use crate::infra::db::entities::device_state_tombstone;
        use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};

        let exists = device_state_tombstone::Entity::find()
            .filter(device_state_tombstone::Column::ModelType.eq(Self::SYNC_MODEL))
            .filter(device_state_tombstone::Column::RecordUuid.eq(uuid))
            .one(db)
            .await?
            .is_some();

        Ok(exists)
    }
}
```

**Model implementations with tombstone awareness:**

```rust
// core/src/infra/db/entities/location.rs
impl Syncable for Model {
    async fn apply_state_change(data: serde_json::Value, db: &DatabaseConnection) -> Result<(), DbErr> {
        let uuid: Uuid = extract_uuid_from_json(&data)?;

        // CRITICAL: Check if location was deleted (prevents race condition)
        if Self::is_tombstoned(uuid, db).await? {
            debug!("Skipping state change for tombstoned location {}", uuid);
            return Ok(());
        }

        // Normal upsert logic...
        // (existing implementation)
    }

    async fn apply_deletion(uuid: Uuid, db: &DatabaseConnection) -> Result<(), DbErr> {
        if let Some(location) = Entity::find()
            .filter(Column::Uuid.eq(uuid))
            .one(db)
            .await?
        {
            // Delete root entry (cascades to all children)
            if let Some(entry_id) = location.entry_id {
                crate::ops::indexing::responder::delete_subtree_internal(entry_id, db).await?;
            }

            Entity::delete_by_id(location.id).exec(db).await?;
        }
        Ok(())
    }
}

// core/src/infra/db/entities/entry.rs
impl Syncable for Model {
    async fn apply_state_change(data: serde_json::Value, db: &DatabaseConnection) -> Result<(), DbErr> {
        let uuid: Uuid = extract_uuid_from_json(&data)?;

        // CRITICAL: Check if entry was deleted (prevents race condition)
        if Self::is_tombstoned(uuid, db).await? {
            debug!("Skipping state change for tombstoned entry {}", uuid);
            return Ok(());
        }

        // Check if parent is tombstoned (prevents orphaned children)
        if let Some(parent_uuid) = extract_parent_uuid_from_json(&data)? {
            if Self::is_tombstoned(parent_uuid, db).await? {
                debug!("Skipping entry {} - parent is tombstoned", uuid);
                return Ok(());
            }
        }

        // Normal upsert logic...
        // (existing implementation)
    }

    async fn apply_deletion(uuid: Uuid, db: &DatabaseConnection) -> Result<(), DbErr> {
        if let Some(entry) = Entity::find()
            .filter(Column::Uuid.eq(uuid))
            .one(db)
            .await?
        {
            // Cascade delete entire subtree!
            crate::ops::indexing::responder::delete_subtree_internal(entry.id, db).await?;
        }
        Ok(())
    }
}

// core/src/infra/db/entities/volume.rs
impl Syncable for Model {
    async fn apply_state_change(data: serde_json::Value, db: &DatabaseConnection) -> Result<(), DbErr> {
        let uuid: Uuid = extract_uuid_from_json(&data)?;

        // Check if volume was deleted
        if Self::is_tombstoned(uuid, db).await? {
            debug!("Skipping state change for tombstoned volume {}", uuid);
            return Ok(());
        }

        // Normal upsert logic...
    }

    async fn apply_deletion(uuid: Uuid, db: &DatabaseConnection) -> Result<(), DbErr> {
        Entity::delete_many()
            .filter(Column::Uuid.eq(uuid))
            .exec(db)
            .await?;
        Ok(())
    }
}
```

**Registry dispatches deletions generically:**

```rust
// core/src/infra/sync/registry.rs

pub async fn apply_deletion(
    model_type: &str,
    uuid: Uuid,
    db: Arc<DatabaseConnection>,
) -> Result<(), ApplyError> {
    let registry = SYNCABLE_REGISTRY.read().await;

    let registration = registry
        .get(model_type)
        .ok_or_else(|| ApplyError::UnknownModel(model_type.to_string()))?;

    let deletion_fn = registration.deletion_apply_fn
        .ok_or_else(|| ApplyError::MissingDeletionHandler(model_type.to_string()))?;

    deletion_fn(uuid, db).await?;
    Ok(())
}
```

#### 5. Processing Deletions (Clean Generic Code!)

```rust
// core/src/service/sync/backfill.rs

async fn apply_state_batch(&self, response: StateResponse) -> Result<()> {
    let SyncMessage::StateResponse {
        model_type,
        records,
        deleted_uuids,
        ..
    } = response;

    // 1. Apply updates (existing registry pattern)
    for record in records {
        registry::apply_state_change(&model_type, record.data, db).await?;
    }

    // 2. NEW: Apply deletions (same registry pattern!)
    for uuid in deleted_uuids {
        registry::apply_deletion(&model_type, uuid, db).await?;
    }

    info!(
        "Applied {} updates and {} deletions for {}",
        records.len(),
        deleted_uuids.len(),
        model_type
    );

    Ok(())
}
```

**Benefits:**
- Completely generic - no model-specific code in backfill handler
- Matches existing sync design (updates use `apply_state_change`, deletions use `apply_deletion`)
- Extensible - new models just implement the trait method
- Cascading handled by model implementation (entries cascade, volumes don't)

#### 6. Unified Acknowledgment-Based Pruning

Tombstones use the same pruning pattern as the peer log - prune when all devices have synced past them.

```rust
// core/src/service/sync/mod.rs

impl SyncService {
    pub fn new(library: Arc<Library>, network: Arc<NetworkingService>) -> Self {
        let service = Self { /* ... */ };

        // Spawn unified pruning task (runs hourly)
        let db = library.db().clone();
        let library_id = library.id();

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(3600));

            loop {
                interval.tick().await;

                if let Err(e) = Self::prune_sync_coordination_data(&db, library_id).await {
                    error!(
                        library_id = %library_id,
                        error = %e,
                        "Failed to prune sync coordination data"
                    );
                }
            }
        });

        service
    }

    /// Unified pruning for all sync coordination data
    ///
    /// Prunes both peer log (shared resources) and tombstones (device-owned deletions)
    /// using the same acknowledgment-based pattern.
    async fn prune_sync_coordination_data(
        db: &crate::library::Database,
        library_id: Uuid,
    ) -> Result<()> {
        // 1. Prune peer log (shared resources, in sync.db)
        let pruned_peer_log = Self::prune_peer_log_acked(&library_id).await?;

        // 2. Prune tombstones (device-owned deletions, in library.db)
        let pruned_tombstones = Self::prune_tombstones_acked(db).await?;

        if pruned_peer_log > 0 || pruned_tombstones > 0 {
            info!(
                library_id = %library_id,
                peer_log_pruned = pruned_peer_log,
                tombstones_pruned = pruned_tombstones,
                "Pruned sync coordination data (ack-based)"
            );
        }

        Ok(())
    }

    /// Prune tombstones that all devices have synced past
    async fn prune_tombstones_acked(db: &crate::library::Database) -> Result<usize> {
        // Get all devices that have completed at least one sync
        let synced_devices = entities::device::Entity::find()
            .filter(entities::device::Column::LastSyncAt.is_not_null())
            .all(db.conn())
            .await?;

        if synced_devices.is_empty() {
            return Ok(0);
        }

        // Find minimum watermark (slowest device)
        let min_watermark = synced_devices
            .iter()
            .filter_map(|d| d.last_state_watermark)
            .min();

        let Some(min_wm) = min_watermark else {
            return Ok(0);
        };

        // Safety limit: Don't keep tombstones longer than 7 days
        // Prevents one offline device from blocking pruning forever
        let max_retention = chrono::Utc::now() - chrono::Duration::days(7);
        let effective_cutoff = min_wm.min(max_retention);

        // Prune tombstones older than effective cutoff
        let result = entities::device_state_tombstone::Entity::delete_many()
            .filter(entities::device_state_tombstone::Column::DeletedAt.lt(effective_cutoff))
            .exec(db.conn())
            .await?;

        Ok(result.rows_affected() as usize)
    }

    /// Prune peer log entries that all peers have acknowledged
    async fn prune_peer_log_acked(library_id: &Uuid) -> Result<usize> {
        // Get peer log for this library
        let peer_log = /* get from service */;

        // Use existing peer_acks mechanism
        peer_log.prune_acked().await.map_err(|e| anyhow::anyhow!(e))
    }
}
```

**Key insights:**
- Both use "min of all devices" pattern (HLC for shared, watermark for device-owned)
- Both have safety limits (7 days for tombstones, similar for peer log)
- Both run in same task (unified pruning)
- **Result: Minimal storage in both databases**

## Edge Cases & Solutions

### 1. Child Deleted Before Parent

```
Device A: Delete /Photos/2024/file.jpg
Device B: Receives tombstone, deletes file
    ↓
Later:
Device A: Delete /Photos (parent folder)
Device B: Receives tombstone for /Photos
    → Calls delete_subtree()
    → File already gone (no-op, idempotent)
    → Successfully deletes /Photos ✅
```

**Verdict:** Safe! `delete_subtree()` handles missing children gracefully.

### 2. Multiple Deletion Levels

```
User deletes individually:
- /Photos/2024/January/img1.jpg (creates tombstone)
- /Photos/2024/February/img2.jpg (creates tombstone)
- /Photos/2023/img3.jpg (creates tombstone)

Later deletes /Photos (parent):
- Creates tombstone for /Photos

Receiving device processes all 4 tombstones:
- Deletes individual files first
- Then deletes /Photos (cascade to already-deleted children is no-op)
- Correct final state ✅
```

**Verdict:** Order-independent, idempotent.

### 3. Partial Tree on Receiver

```
Device A has full tree:
/Photos (root)
  /Subfolder
    file1.jpg
    file2.jpg

Device B is backfilling, only has:
/Photos
  /Subfolder (children haven't synced yet)

Device A deletes /Photos
Device B receives tombstone:
- Calls delete_subtree() on /Photos entry
- Cascades to /Subfolder
- file1.jpg, file2.jpg never made it to B anyway
- Correct state ✅
```

**Verdict:** Safe! Can only delete what exists locally.

### 4. Backfill Race Condition (Critical!)

**Problem:** Deletion tombstone arrives before or during backfill of the record itself.

```
T0: Device B starts backfilling from Device A
T1: Device A deletes entry X (creates tombstone)
T2: Device B's StateRequest happens (includes entry X in response)
T3: Device B receives deletion tombstone for entry X (in buffer queue)
T4: Device B applies state changes (creates entry X)
T5: Device B processes buffer (tries to delete entry X)

Issue: Entry X exists on Device B despite being deleted on Device A!
```

**Solution:** Check tombstones before applying state changes:

```rust
// In each model's apply_state_change():
async fn apply_state_change(data: Json, db: &DatabaseConnection) -> Result<(), DbErr> {
    let uuid = extract_uuid(&data)?;

    // Check if tombstoned (skip if deleted)
    if Self::is_tombstoned(uuid, db).await? {
        debug!("Skipping state change for tombstoned record {}", uuid);
        return Ok(());
    }

    // For entries: also check if parent is tombstoned
    if Self::SYNC_MODEL == "entry" {
        if let Some(parent_uuid) = extract_parent_uuid(&data)? {
            if Self::is_tombstoned(parent_uuid, db).await? {
                debug!("Skipping entry - parent is tombstoned");
                return Ok(());  // Don't create orphans
            }
        }
    }

    // Normal upsert logic...
}
```

**This prevents:**
- Re-creating deleted records during backfill
- Creating orphaned children when parent is deleted

### 5. Device Offline >30 Days

**Problem:** Tombstones pruned before device reconnects.

**Solution:** Check watermark age before incremental sync:

```rust
pub async fn catch_up_from_peer(
    &self,
    peer: Uuid,
    state_watermark: Option<DateTime<Utc>>,
) -> Result<()> {
    let watermark_age = state_watermark
        .map(|w| chrono::Utc::now() - w)
        .unwrap_or(chrono::Duration::max_value());

    let effective_watermark = if watermark_age > chrono::Duration::days(25) {
        warn!("Watermark is {} days old, forcing full sync", watermark_age.num_days());
        None  // Force full sync to ensure consistency
    } else {
        state_watermark
    };

    // Use effective_watermark...
}
```

**Rationale:** 25-day threshold (5-day safety margin) ensures reliable detection.

## Research Validation & Findings

A comprehensive audit of the sync codebase confirmed the design is sound with minor additions needed.

### Protocol Compatibility ✅

- StateResponse uses serde JSON serialization (backward compatible)
- Adding `deleted_uuids: Vec<Uuid>` as optional field is safe
- Old clients will ignore unknown fields gracefully
- No breaking changes to existing messages

### Registry Pattern Compatibility ✅

- Registry uses function pointers (easy to add deletion dispatch)
- Can add `StateDeleteFn` type alongside `StateApplyFn`
- Completely generic - no model-specific code in handlers
- Matches existing `apply_state_change()` pattern exactly

### Existing Deletion Paths Identified

**Device-Owned Models:**
- `location::Manager::remove_location()` - Only local deletion, NO sync
- `responder::delete_subtree()` - Entry deletion via watcher, NO sync
- File delete operations - Local only, NO sync

**Shared Models:**
- Tags/Collections already support `ChangeType::Delete` via peer log
- Uses HLC-based sync with acknowledgments
- Device-owned models lack equivalent

### Critical Race Condition Found

**Issue:** Deletion tombstone can arrive during backfill, then record is applied, re-creating deleted data.

**Solution:** Add `is_tombstoned()` check in all `apply_state_change()` implementations.

**Additional:** For entries, also check if parent is tombstoned (prevents orphaned children).

### Watermark Infrastructure ✅

- `devices.last_state_watermark` already tracks device-owned sync progress
- Can reuse for tombstone acknowledgment (no new table needed)
- Unified pruning with peer log is feasible (both use "min of all devices" pattern)

### Files Requiring Modification

**Core Infrastructure (4 files):**
1. `core/src/infra/sync/syncable.rs` - Add `apply_deletion()` and `is_tombstoned()` methods
2. `core/src/infra/sync/registry.rs` - Add deletion dispatch function
3. `core/src/service/network/protocol/sync/messages.rs` - Add `deleted_uuids` field
4. `core/src/infra/db/entities/device_state_tombstone.rs` - New entity model

**Model Implementations (3 files):**
5. `core/src/infra/db/entities/location.rs` - Implement tombstone check + apply_deletion
6. `core/src/infra/db/entities/entry.rs` - Implement tombstone check + apply_deletion
7. `core/src/infra/db/entities/volume.rs` - Implement tombstone check + apply_deletion

**Deletion Logic (2 files):**
8. `core/src/ops/indexing/responder.rs` - Split delete_subtree variants, create tombstones
9. `core/src/location/manager.rs` - Create tombstones on location deletion

**Sync Protocol (2 files):**
10. `core/src/service/sync/protocol_handler.rs` - Query tombstones in StateResponse
11. `core/src/service/sync/backfill.rs` - Process deleted_uuids via registry

**Pruning (1 file):**
12. `core/src/service/sync/mod.rs` - Unified ack-based pruning task

**Migration (1 file):**
13. New migration file for `device_state_tombstones` table

**Total: 13 files** (no deletions, all additions/modifications)

## Performance Analysis

### Storage Overhead (With Cascading)

```
Realistic 30-day usage:
- Locations: 2 deletions = 2 tombstones
- Volumes: 1 deletion = 1 tombstone
- Individual files: 100 deletions = 100 tombstones
- Folders: 10 deletions (1000 files each) = 10 tombstones

Total: 113 tombstones
Storage: 113 × 150 bytes = ~17 KB

Without cascading:
- Entry tombstones: 100 + 10,000 = 10,100 tombstones
- Storage: ~1.5 MB
- Compression ratio: 89:1 
```

### Network Overhead (With Cascading)

```
StateResponse for folder deletion:
- deleted_uuids: [folder_uuid]  // 1 UUID = 36 bytes

vs without cascading:
- deleted_uuids: [...10000 UUIDs...]  // 360 KB

Network compression: 10,000:1 
```

### Query Performance

```sql
-- Tombstone query (very fast with cascading)
SELECT record_uuid FROM device_state_tombstones
WHERE model_type = 'entry'
  AND device_id = ?
  AND deleted_at >= ?;

-- Returns ~110 rows (not 10,100!)
-- Query time: <5ms
-- Index: idx_tombstones_lookup(model_type, device_id, deleted_at)
```

## Testing Strategy

### Unit Tests

```rust
#[tokio::test]
async fn test_cascading_tombstone_on_folder_delete() {
    // Given: Folder with 1000 entries
    // When: Folder deleted via delete_subtree()
    // Then: 1 tombstone created (for root), 1000 entries deleted
}

#[tokio::test]
async fn test_individual_file_tombstone() {
    // Given: Single file (no children)
    // When: File deleted
    // Then: 1 tombstone created
}

#[tokio::test]
async fn test_tombstone_processing_cascades() {
    // Given: Device B has folder with 1000 entries
    // When: Receives tombstone for folder UUID
    // Then: All 1000 entries deleted via cascade
}

#[tokio::test]
async fn test_idempotent_cascade_deletion() {
    // Given: Children already deleted
    // When: Process parent tombstone
    // Then: No error, parent deleted successfully
}

#[tokio::test]
async fn test_tombstone_pruning() {
    // Given: Tombstones older than 30 days exist
    // When: Pruning task runs
    // Then: Old tombstones deleted, recent ones remain
}
```

### Integration Tests

```rust
#[tokio::test]
async fn test_folder_deletion_sync_with_cascade() {
    // Given: Two devices, Device A has folder with 10,000 entries
    // When: Device A deletes folder
    // Then: Device B receives 1 tombstone, cascades to 10,000 deletions
}

#[tokio::test]
async fn test_location_deletion_sync() {
    // Given: Two devices, both have location
    // When: Device A deletes location
    // Then: Device B deletes location and all entries within 60 seconds
}

#[tokio::test]
async fn test_offline_device_catches_up() {
    // Given: Device B offline for 1 day
    // When: Device A deletes 100 entries, B reconnects
    // Then: B catches up via tombstones, deletes all 100
}

#[tokio::test]
async fn test_watermark_too_old_forces_full_sync() {
    // Given: Device offline for 35 days
    // When: Device reconnects
    // Then: Performs full sync instead of incremental
}
```

## Migration Plan

### Phase 1: Schema & Core Logic (2-3 hours)

1. Create migration for `device_state_tombstones` table in library.db
2. Add entity model `core/src/infra/db/entities/device_state_tombstone.rs`
3. Add `apply_deletion()` and `is_tombstoned()` methods to `Syncable` trait
4. Refactor `delete_subtree()` in `responder.rs` into:
   - `delete_subtree()` - Creates tombstones (for local deletions)
   - `delete_subtree_internal()` - No tombstones (for applying synced deletions)
5. Update `delete_subtree()` to create tombstones atomically
6. Update `LocationManager::remove_location()` to create tombstones
7. Add volume deletion support (if not exists)
8. Implement `prune_sync_coordination_data()` with unified ack-based pruning
9. Update `SyncService::new()` to spawn unified pruning task (hourly)

**Deliverable:** Tombstones created with unified ack-based pruning, not yet synced.

### Phase 2: Protocol & Registry Changes (2-3 hours)

1. Add `deleted_uuids: Vec<Uuid>` to `StateResponse` message
2. Add `StateDeleteFn` type and `deletion_apply_fn` to `SyncableModelRegistration`
3. Implement `apply_deletion()` for location, entry, and volume models
4. **CRITICAL:** Update `apply_state_change()` in all device-owned models to check `is_tombstoned()`
5. Add `registry::apply_deletion()` dispatch function
6. Register deletion functions in `initialize_registry()`
7. Update `StateSyncHandler::handle_state_request()` to query tombstones
8. Update `BackfillManager::apply_state_batch()` to call `registry::apply_deletion()`
9. Add watermark age check for 25-day boundary (force full sync if older)

**Deliverable:** Deletions sync via registry pattern with race condition protection.

### Phase 3: Testing & Polish (2-3 hours)

1. Write unit tests for tombstone lifecycle and cascading
2. Write integration tests for sync scenarios including:
   - Backfill race condition (deletion during backfill)
   - Orphaned children prevention (parent tombstoned)
   - Offline device catch-up
   - Cascading folder deletions
3. Test registry dispatch for all model types
4. Test unified pruning (peer log + tombstones)
5. Add metrics/logging for observability
6. Update documentation (library-sync.mdx)

**Deliverable:** Production-ready cascading deletion sync with race condition protection.

**Total Estimate:** 6-9 hours

## Design Principles

### 1. Cascading Tombstones

**Problem:** Deleting a folder with 10,000 files would create 10,000 tombstones.

**Solution:** Only tombstone the root UUID. Receiving device uses `entry_closure` to cascade automatically.

**Benefits:**
- **Massive compression** - 100:1 to 10,000:1 for folders
- **Leverages existing tree structure** - `entry_closure` does the heavy lifting
- **Order-independent** - Process deletions in any order
- **Idempotent** - Deleting missing children is safe
- **Network efficient** - 1 UUID vs thousands (10,000:1 compression)
- **Storage efficient** - ~17 KB vs ~1.5 MB (30 days)

### 2. Registry-Based Dispatch

**Problem:** Backfill handler would need model-specific deletion code for each type.

**Solution:** Extend `Syncable` trait with `apply_deletion()` method. Use registry pattern for dispatch.

**Benefits:**
- **Consistent with library sync design** - Matches existing `apply_state_change()` pattern
- **Completely generic** - Backfill handler has zero model-specific code
- **Extensible** - New models just implement the trait method
- **Type-safe** - Compiler ensures all models handle deletions
- **Testable** - Each model's deletion logic tested independently
- **Clean separation** - Deletion logic lives with the model

### 3. Unified Acknowledgment-Based Pruning

**Problem:** Different pruning strategies for tombstones vs peer log would be confusing.

**Solution:** Use the same "min of all devices" pattern for both. Single unified pruning task.

**Benefits:**
- **Conceptually unified** - Peer log and tombstones both use ack-based pruning
- **Aggressive cleanup** - Prunes within 1 hour when all devices caught up
- **Safety limits** - 7-day max prevents offline devices blocking forever
- **Simpler mental model** - "Sync coordination data auto-pruned when acknowledged"
- **Configurable** - One place to configure sync retention policy
- **Keeps both DBs minimal** - Typical: <50 rows tombstones, <1MB peer log

## Future: Configurable Sync Retention Policy

The unified pruning design enables future user configuration:

```rust
// Future: User-configurable sync retention policy
struct SyncRetentionPolicy {
    /// How aggressively to prune sync coordination data
    pruning_mode: PruningMode,

    /// Maximum retention regardless of acks (safety net)
    max_retention_days: u32,
}

enum PruningMode {
    /// Prune as soon as all devices acknowledge (minimal storage)
    Aggressive,

    /// Keep for minimum duration even if acked (safety buffer)
    Conservative { min_days: u32 },

    /// Always keep for fixed duration (ignore acks)
    TimeBased { days: u32 },
}

// Example configurations:
// Power user (fast sync, minimal storage):
SyncRetentionPolicy {
    pruning_mode: PruningMode::Aggressive,
    max_retention_days: 7,
}

// Cautious user (handles flaky connections):
SyncRetentionPolicy {
    pruning_mode: PruningMode::Conservative { min_days: 3 },
    max_retention_days: 30,
}
```

**Single config affects:**
- Peer log pruning (shared resources)
- Tombstone pruning (device-owned deletions)
- Both watermark-based sync mechanisms

**Not in scope for LSYNC-020** - can be added later once deletion sync is stable.

## Success Criteria

1. **VDFS Consistency** - All devices have accurate filesystem view (zero stale data)
2. **Performance** - <1% overhead on incremental sync
3. **Scalability** - Handles 1000+ folder deletions/month with minimal storage
4. **Reliability** - Zero data loss, zero permanent inconsistencies
5. **Unified Pruning** - Both peer log and tombstones use same ack-based pattern
6. **Tests** - 95%+ code coverage for deletion sync paths

## References

- [Library Sync Documentation](/docs/core/library-sync.mdx)
- [Location Manager](/core/src/location/manager.rs)
- [Indexing Responder](/core/src/ops/indexing/responder.rs)
- [Sync Protocol Handler](/core/src/service/sync/protocol_handler.rs)
- [Backfill Manager](/core/src/service/sync/backfill.rs)
- Related discussion: Device-owned deletion sync design (this conversation)

---

**Next Steps:**
1. Review and approve cascading tombstone design
2. Implement Phase 1 (schema + tombstone creation)
3. Implement Phase 2 (sync protocol changes)
4. Test with high-volume deletion scenarios
