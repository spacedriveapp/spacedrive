# Library Sync Watermark Fix - Implementation Plan

## Problem Statement

The library sync system uses **two global watermarks per device**:

- `last_state_watermark` (timestamp) - for ALL device-owned resources
- `last_shared_watermark` (HLC) - for ALL shared resources

This breaks after ~10k entries because advancing the watermark for one resource type (e.g., entries) can cause other resource types (e.g., locations, volumes) to be skipped during reconnection sync.

**Root Cause**: A single timestamp cannot represent the sync state of multiple independent resource types that have different timestamp distributions.

## Proposed Solution

### Phase 1: Database Schema (Migration)

Create a new table in `sync.db` for per-resource watermark tracking:

```sql
-- Location: sync.db (NOT database.db!)
-- Migration: Add to sync.db initialization in core/src/infra/sync/db.rs

CREATE TABLE device_resource_watermarks (
    device_uuid TEXT NOT NULL,        -- Our device UUID
    peer_device_uuid TEXT NOT NULL,   -- Peer's device UUID
    resource_type TEXT NOT NULL,      -- "location", "entry", "volume", etc.
    last_watermark TEXT NOT NULL,     -- RFC3339 timestamp
    updated_at TEXT NOT NULL,         -- RFC3339 timestamp

    PRIMARY KEY (device_uuid, peer_device_uuid, resource_type)
);

CREATE INDEX idx_resource_watermarks_peer
    ON device_resource_watermarks(peer_device_uuid, resource_type);

CREATE INDEX idx_resource_watermarks_resource
    ON device_resource_watermarks(resource_type);
```

**Benefits of sync.db location:**

- Keeps main library database clean (domain data only)
- Watermarks are sync coordination metadata, not domain data
- Consistent with `shared_changes` and `peer_acks` tables
- No foreign key constraints needed (UUIDs are self-contained)
- Easy to initialize from existing global watermarks

**No migration needed!**

Since this is unreleased:

- Delete existing sync.db files on startup (force fresh sync)
- Remove old watermark columns from devices table
- Clean slate with per-resource watermarks from day one

```rust
// core/src/infra/sync/db.rs initialization
pub fn init_sync_db(library_path: &Path, device_uuid: Uuid) -> Result<Connection> {
    let sync_db_path = library_path.join("sync.db");

    // Open connection
    let conn = Connection::open(&sync_db_path)?;

    // Initialize tables
    ResourceWatermarkStore::init_table(&conn)?;

    // Initialize other sync tables (shared_changes, peer_acks, etc.)
    init_shared_changes_table(&conn)?;
    init_peer_acks_table(&conn)?;

    Ok(conn)
}
```

### Phase 2: Data Access Layer

**Update File**: `core/src/infra/sync/db.rs` (or new file `core/src/infra/sync/watermarks.rs`)

Since this goes in `sync.db` (SQLite without SeaORM), use direct SQL queries:

```rust
use rusqlite::{params, Connection, Result};
use uuid::Uuid;
use chrono::{DateTime, Utc};

/// Resource watermark tracking for incremental sync
pub struct ResourceWatermarkStore {
    device_uuid: Uuid,
}

impl ResourceWatermarkStore {
    pub fn new(device_uuid: Uuid) -> Self {
        Self { device_uuid }
    }

    /// Initialize the watermarks table in sync.db
    pub fn init_table(conn: &Connection) -> Result<()> {
        conn.execute(
            "CREATE TABLE IF NOT EXISTS device_resource_watermarks (
                device_uuid TEXT NOT NULL,
                peer_device_uuid TEXT NOT NULL,
                resource_type TEXT NOT NULL,
                last_watermark TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                PRIMARY KEY (device_uuid, peer_device_uuid, resource_type)
            )",
            [],
        )?;

        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_resource_watermarks_peer
             ON device_resource_watermarks(peer_device_uuid, resource_type)",
            [],
        )?;

        Ok(())
    }

    /// Upsert a resource watermark
    pub fn upsert(
        &self,
        conn: &Connection,
        peer_device_uuid: Uuid,
        resource_type: &str,
        watermark: DateTime<Utc>,
    ) -> Result<()> {
        // Check if newer before updating
        let existing: Option<String> = conn
            .query_row(
                "SELECT last_watermark FROM device_resource_watermarks
                 WHERE device_uuid = ? AND peer_device_uuid = ? AND resource_type = ?",
                params![
                    self.device_uuid.to_string(),
                    peer_device_uuid.to_string(),
                    resource_type,
                ],
                |row| row.get(0),
            )
            .ok();

        if let Some(existing_str) = existing {
            if let Ok(existing_ts) = DateTime::parse_from_rfc3339(&existing_str) {
                if watermark <= existing_ts.with_timezone(&Utc) {
                    // Don't update if not newer
                    return Ok(());
                }
            }
        }

        // Upsert
        conn.execute(
            "INSERT INTO device_resource_watermarks
             (device_uuid, peer_device_uuid, resource_type, last_watermark, updated_at)
             VALUES (?, ?, ?, ?, ?)
             ON CONFLICT (device_uuid, peer_device_uuid, resource_type)
             DO UPDATE SET
                last_watermark = excluded.last_watermark,
                updated_at = excluded.updated_at",
            params![
                self.device_uuid.to_string(),
                peer_device_uuid.to_string(),
                resource_type,
                watermark.to_rfc3339(),
                Utc::now().to_rfc3339(),
            ],
        )?;

        Ok(())
    }

    /// Get watermark for a specific resource type
    pub fn get(
        &self,
        conn: &Connection,
        peer_device_uuid: Uuid,
        resource_type: &str,
    ) -> Result<Option<DateTime<Utc>>> {
        let result: Option<String> = conn
            .query_row(
                "SELECT last_watermark FROM device_resource_watermarks
                 WHERE device_uuid = ? AND peer_device_uuid = ? AND resource_type = ?",
                params![
                    self.device_uuid.to_string(),
                    peer_device_uuid.to_string(),
                    resource_type,
                ],
                |row| row.get(0),
            )
            .ok();

        result
            .and_then(|s| DateTime::parse_from_rfc3339(&s).ok())
            .map(|dt| dt.with_timezone(&Utc))
            .ok_or_else(|| rusqlite::Error::QueryReturnedNoRows)
            .or(Ok(None))
    }

    /// Get all watermarks for a peer (for diagnostics)
    pub fn get_all_for_peer(
        &self,
        conn: &Connection,
        peer_device_uuid: Uuid,
    ) -> Result<Vec<(String, DateTime<Utc>)>> {
        let mut stmt = conn.prepare(
            "SELECT resource_type, last_watermark FROM device_resource_watermarks
             WHERE device_uuid = ? AND peer_device_uuid = ?
             ORDER BY resource_type",
        )?;

        let rows = stmt.query_map(
            params![self.device_uuid.to_string(), peer_device_uuid.to_string()],
            |row| {
                let resource_type: String = row.get(0)?;
                let watermark_str: String = row.get(1)?;
                Ok((resource_type, watermark_str))
            },
        )?;

        let mut results = Vec::new();
        for row in rows {
            if let Ok((resource_type, watermark_str)) = row {
                if let Ok(dt) = DateTime::parse_from_rfc3339(&watermark_str) {
                    results.push((resource_type, dt.with_timezone(&Utc)));
                }
            }
        }

        Ok(results)
    }
}
```

### Phase 3: Sync Service Updates

**File**: `core/src/service/sync/peer.rs`

Add a reference to sync.db and the watermark store:

```rust
use crate::infra::sync::watermarks::ResourceWatermarkStore;

pub struct PeerSync {
    // Existing fields...
    db: Arc<Database>,  // Main library database
    sync_db: Arc<Mutex<rusqlite::Connection>>,  // NEW: sync.db connection
    watermark_store: ResourceWatermarkStore,  // NEW: Watermark access
    // ...
}

impl PeerSync {
    pub fn new(
        library_id: Uuid,
        device_id: Uuid,
        db: Arc<Database>,
        sync_db: Arc<Mutex<rusqlite::Connection>>,
        // ... other params
    ) -> Arc<Self> {
        // Initialize watermark store
        let watermark_store = ResourceWatermarkStore::new(device_id);

        // Initialize table if needed
        if let Ok(conn) = sync_db.lock() {
            let _ = ResourceWatermarkStore::init_table(&conn);
        }

        Arc::new(Self {
            library_id,
            device_id,
            db,
            sync_db,
            watermark_store,
            // ...
        })
    }

    // NEW: Get resource-specific watermark from sync.db
    pub fn get_resource_watermark(
        &self,
        peer_device_id: Uuid,
        resource_type: &str,
    ) -> Option<chrono::DateTime<chrono::Utc>> {
        let conn = self.sync_db.lock().ok()?;
        self.watermark_store
            .get(&conn, peer_device_id, resource_type)
            .ok()?
    }

    // UPDATED: Update resource-specific watermark in sync.db
    async fn update_state_watermark(
        &self,
        peer_device_id: Uuid,
        resource_type: &str,  // NEW parameter
        timestamp: chrono::DateTime<chrono::Utc>,
    ) -> Result<()> {
        // Update in sync.db (not main database!)
        let conn = self.sync_db
            .lock()
            .map_err(|e| anyhow::anyhow!("Failed to lock sync.db: {}", e))?;

        self.watermark_store
            .upsert(&conn, peer_device_id, resource_type, timestamp)
            .map_err(|e| anyhow::anyhow!("Failed to update resource watermark: {}", e))?;

        debug!(
            peer = %peer_device_id,
            resource = %resource_type,
            watermark = %timestamp,
            "Updated resource watermark in sync.db"
        );

        Ok(())
    }

// UPDATED: Apply state change with resource-specific watermark
async fn apply_state_change(&self, change: StateChangeMessage) -> Result<()> {
    let start_time = std::time::Instant::now();

    debug!(
        model_type = %change.model_type,
        record_uuid = %change.record_uuid,
        device_id = %change.device_id,
        "Applying state change"
    );

    // Apply the change
    crate::infra::sync::apply_state_change(
        &change.model_type,
        change.data.clone(),
        self.db.clone(),
    )
    .await?;

    // Record metrics
    self.metrics.record_changes_applied(1);
    self.metrics.record_entries_synced(&change.model_type, 1).await;

    // Record latency
    let latency_ms = start_time.elapsed().as_millis() as u64;
    self.metrics.record_apply_latency(latency_ms);

    // Update RESOURCE-SPECIFIC watermark (FIX!)
    self.update_state_watermark(
        change.device_id,
        &change.model_type,  // ← Include resource type!
        change.timestamp,
    )
    .await?;

    // Emit resource event
    self.event_bus.emit(Event::ResourceChanged {
        resource_type: change.model_type.clone(),
        resource: change.data,
        metadata: None,
    });

    Ok(())
}
```

### Phase 4: Backfill Updates

**File**: `core/src/service/sync/backfill.rs`

```rust
// UPDATED: Backfill with per-resource watermarks
async fn backfill_all_state(
    &self,
    primary_peer: Uuid,
) -> Result<Option<String>> {
    info!("Starting backfill of device-owned state from peer {}", primary_peer);

    // Compute sync order
    let sync_order = crate::infra::sync::compute_registry_sync_order().await?;

    // Filter to device-owned models
    let mut model_types = Vec::new();
    for model in sync_order {
        if crate::infra::sync::is_device_owned(&model).await {
            model_types.push(model);
        }
    }

    // Backfill each resource type with its own watermark
    for model_type in model_types {
        // Get resource-specific watermark (FIX!)
        let resource_watermark = self.peer_sync
            .get_resource_watermark(primary_peer, &model_type)
            .await;

        info!(
            model_type = %model_type,
            watermark = ?resource_watermark,
            "Backfilling resource type"
        );

        let checkpoint = self
            .backfill_peer_state(
                primary_peer,
                vec![model_type.clone()],
                None,
                resource_watermark,  // ← Per-resource watermark!
            )
            .await?;

        info!(
            model_type = %model_type,
            final_checkpoint = ?checkpoint.resume_token,
            "Resource type backfill complete"
        );
    }

    Ok(None)
}
```

### Phase 5: Clean Break - Remove Old Watermarks

Since this is unreleased software, we can make breaking changes!

**Remove from `database.db`:**

```sql
-- Migration: m20251115_000002_remove_old_watermarks.sql
ALTER TABLE devices DROP COLUMN last_state_watermark;
ALTER TABLE devices DROP COLUMN last_shared_watermark;
```

**Remove from Device entity:**

```rust
// core/src/infra/db/entities/device.rs
pub struct Model {
    // ... other fields ...

    // Sync coordination fields (moved to sync.db!)
    pub sync_enabled: bool,
    pub last_sync_at: Option<DateTimeUtc>,
    // REMOVED: pub last_state_watermark: Option<DateTimeUtc>,
    // REMOVED: pub last_shared_watermark: Option<String>,
}
```

**Simplify PeerSync:**

```rust
// No fallback logic needed!
pub fn get_resource_watermark(
    &self,
    peer_device_id: Uuid,
    resource_type: &str,
) -> Option<chrono::DateTime<chrono::Utc>> {
    let conn = self.sync_db.lock().ok()?;
    self.watermark_store
        .get(&conn, peer_device_id, resource_type)
        .ok()?
    // That's it! No fallback to old global watermark
}
```

**Benefits of clean break:**

- No migration logic needed
- No dual-write during transition
- Simpler code, easier to maintain
- Clearer separation: sync.db = sync metadata, database.db = domain data
- Forces full re-sync on update (safe, ensures consistency)

## Testing Strategy

### Unit Tests

**Test 1**: Per-resource watermark storage

```rust
#[tokio::test]
async fn test_resource_watermark_storage() {
    let db = setup_test_db().await;
    let device_id = 1;
    let peer_id = Uuid::new_v4();

    // Store different watermarks for different resources
    DeviceResourceWatermark::upsert(
        device_id, peer_id, "location",
        parse_datetime("2025-01-01 10:00:00"),
        &db
    ).await.unwrap();

    DeviceResourceWatermark::upsert(
        device_id, peer_id, "entry",
        parse_datetime("2025-01-01 15:00:00"),
        &db
    ).await.unwrap();

    // Verify independent storage
    let loc_wm = DeviceResourceWatermark::get_watermark(
        device_id, peer_id, "location", &db
    ).await.unwrap().unwrap();

    let entry_wm = DeviceResourceWatermark::get_watermark(
        device_id, peer_id, "entry", &db
    ).await.unwrap().unwrap();

    assert_ne!(loc_wm, entry_wm);
}
```

### Integration Tests

**Test 2**: 10k+ mixed resources with disconnection

```rust
#[tokio::test]
async fn test_sync_10k_mixed_resources_with_reconnect() {
    let setup = SyncTestSetup::new().await?;

    // Device A: Create mixed resources
    let base_time = Utc::now();

    // 50 locations (early timestamps)
    for i in 0..50 {
        let loc = setup.create_location_with_timestamp(
            format!("loc_{}", i),
            base_time + Duration::seconds(i),
            &setup.library_a,
        ).await?;
        setup.library_a.sync_model(&loc, ChangeType::Insert).await?;
    }

    // 15,000 entries (later timestamps, will be batched)
    for i in 0..15000 {
        let entry = setup.create_entry_with_timestamp(
            format!("entry_{}", i),
            base_time + Duration::seconds(100 + i),
            &setup.library_a,
        ).await?;
        setup.library_a.sync_model(&entry, ChangeType::Insert).await?;
    }

    // 10 volumes (mid timestamps)
    for i in 0..10 {
        let vol = setup.create_volume_with_timestamp(
            format!("vol_{}", i),
            base_time + Duration::seconds(50 + i),
            &setup.library_a,
        ).await?;
        setup.library_a.sync_model(&vol, ChangeType::Insert).await?;
    }

    // Device B: Start backfill
    let backfill_mgr = setup.library_b.backfill_manager();
    tokio::spawn({
        let peer_id = setup.device_a_id;
        async move {
            backfill_mgr.start_full_backfill(peer_id).await
        }
    });

    // Wait for partial sync (5k entries)
    tokio::time::sleep(Duration::from_millis(500)).await;

    // Simulate disconnection
    setup.disconnect_devices().await;

    // Wait and reconnect
    tokio::time::sleep(Duration::from_secs(1)).await;
    setup.reconnect_devices().await;

    // Device B: Resume sync
    setup.library_b.sync_service()
        .unwrap()
        .peer_sync()
        .exchange_watermarks_and_catchup(setup.device_a_id)
        .await?;

    setup.wait_for_sync(Duration::from_secs(10)).await?;

    // ASSERTIONS: ALL resource types fully synced
    let loc_count = setup.count_locations(&setup.library_b).await?;
    let entry_count = setup.count_entries(&setup.library_b).await?;
    let vol_count = setup.count_volumes(&setup.library_b).await?;

    assert_eq!(loc_count, 50, "All locations should be synced");
    assert_eq!(entry_count, 15000, "All entries should be synced");
    assert_eq!(vol_count, 10, "All volumes should be synced");

    Ok(())
}
```

### Validation Queries

After deployment, run these queries to verify correctness:

```sql
-- Query sync.db (not database.db!)
-- Verify all peers have per-resource watermarks
SELECT
    device_uuid,
    peer_device_uuid,
    resource_type,
    last_watermark,
    updated_at
FROM device_resource_watermarks
ORDER BY peer_device_uuid, resource_type;

-- Check watermark progression for a specific peer
SELECT
    resource_type,
    last_watermark,
    julianday('now') - julianday(last_watermark) as days_old
FROM device_resource_watermarks
WHERE peer_device_uuid = 'your-peer-uuid-here'
ORDER BY resource_type;
```

## Files to Modify

1. **Sync Database (sync.db)**:

   - `core/src/infra/sync/watermarks.rs` (NEW - watermark store)
   - `core/src/infra/sync/db.rs` (update initialization)
   - `core/src/infra/sync/mod.rs` (export watermark module)

2. **Main Database (database.db) - Clean Up**:

   - `core/src/infra/db/migration/m20251115_000001_remove_watermarks.rs` (NEW - drop columns)
   - `core/src/infra/db/entities/device.rs` (remove watermark fields)

3. **Sync Service**:

   - `core/src/service/sync/peer.rs` (add sync_db, update watermark methods)
   - `core/src/service/sync/backfill.rs` (use per-resource watermarks)
   - `core/src/service/sync/mod.rs` (pass sync_db to PeerSync)

4. **Library Initialization**:

   - `core/src/library/mod.rs` (initialize sync.db, pass to sync service)

5. **Tests**:
   - `core/tests/sync_integration_test.rs` (add 10k+ mixed resource test)
   - Update existing watermark tests to use new API

## Rollout Plan (Simplified for Breaking Change)

### Phase 1: Sync DB Setup (Day 1-2)

- Create `ResourceWatermarkStore` in `core/src/infra/sync/watermarks.rs`
- Update `sync.db` initialization to create watermarks table
- Add sync_db connection to `PeerSync` struct

### Phase 2: Migration (Day 2-3)

- Create migration to drop old watermark columns from devices table
- Update `Device` entity to remove watermark fields
- Clean up any code referencing old watermarks

### Phase 3: Core Logic (Day 3-5)

- Implement `get_resource_watermark()` and `update_resource_watermark()`
- Update `apply_state_change()` to use resource-specific watermarks
- Update `backfill_all_state()` to query per-resource watermarks

### Phase 4: Testing (Day 5-7)

- Unit tests for watermark storage in sync.db
- Integration test for 10k+ mixed resources with reconnection
- Update existing watermark tests
- Manual testing with test datasets

### Phase 5: Deploy (Day 8)

- Merge PR
- User databases will auto-migrate (drop old columns, create new table)
- First sync will be fresh (no watermarks = full backfill)
- Subsequent syncs use per-resource watermarks

**Note:** Users will need to do one full re-sync after update. This is acceptable for unreleased software and ensures clean state.

## Success Metrics

- ✓ All resource types sync correctly even with >10k entries
- ✓ Disconnection/reconnection doesn't lose data
- ✓ No performance regression
- ✓ Tests pass consistently
- ✓ sync.db stays small (<1MB for typical library)
- ✓ Clear separation between domain data and sync metadata

## Risks and Mitigation

**Risk**: Users need to re-sync after update

- **Impact**: First sync after update will be full backfill
- **Mitigation**: This is acceptable for unreleased software
- **UX**: Show clear message: "Sync protocol updated, performing initial sync..."

**Risk**: Performance impact of sync.db queries

- **Mitigation**:
  - Composite primary key provides fast lookups
  - Indexes on peer + resource_type for range queries
  - sync.db stays small (only coordination data)
- **Monitoring**: Track query performance metrics

**Risk**: Sync.db corruption

- **Mitigation**:
  - Can be safely deleted and rebuilt (triggers full re-sync)
  - Regular SQLite integrity checks
  - Atomic writes with WAL mode

## Timeline

**Total Estimate**: 1-1.5 weeks (simplified by breaking change!)

- Sync DB Setup: 2 days
- Migration (remove old): 1 day
- Core Logic: 2 days
- Testing: 2 days
- Documentation: 1 day
- Deployment: 1 day

Much faster because:

- No backward compatibility logic
- No dual-write transition period
- No migration from old watermarks
- Clean slate implementation

## References

- Full analysis: `SYNC_WATERMARK_ANALYSIS.md`
- Visual summary: `SYNC_WATERMARK_ISSUE_SUMMARY.md`
- Current sync docs: `/docs/core/library-sync.mdx`
