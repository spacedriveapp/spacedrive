# Library Sync Watermark Analysis

## Executive Summary

The library sync system **does not use per-resource watermarks**. Instead, it uses **two global watermarks per device**:

1. **`last_state_watermark`** (timestamp) - Tracks ALL device-owned state from a peer
2. **`last_shared_watermark`** (HLC) - Tracks ALL shared resources across the library

This architecture can break down after ~10k entries because:

- A single watermark timestamp applies to ALL resource types (locations, entries, volumes) from a device
- The watermark advances whenever ANY resource is synced
- When requesting catchup, the system queries `WHERE updated_at >= watermark` across all resources
- With 10k+ mixed entries, the watermark may have advanced past some resource types that haven't fully synced

## Current Architecture

### Watermark Storage

Location: `core/src/infra/db/entities/device.rs`

```rust
pub struct Device {
    // ... other fields ...
    pub last_state_watermark: Option<DateTimeUtc>,  // Single timestamp for ALL device-owned data
    pub last_shared_watermark: Option<String>,       // Single HLC for ALL shared resources
}
```

### Watermark Update Flow

**For Device-Owned State:**

```rust
// core/src/service/sync/peer.rs:1736
async fn apply_state_change(&self, change: StateChangeMessage) -> Result<()> {
    // Apply the change
    crate::infra::sync::apply_state_change(&change.model_type, change.data, db).await?;

    // Update GLOBAL state watermark for this device
    self.update_state_watermark(change.device_id, change.timestamp).await?;
    // ^^^ This updates the single watermark regardless of model_type
}
```

**Watermark Update Implementation:**

```rust
// core/src/service/sync/peer.rs:270-300
async fn update_state_watermark(&self, device_id: Uuid, timestamp: DateTime<Utc>) -> Result<()> {
    let device = find_device(device_id).await?;

    // Update watermark if newer
    if timestamp > device.last_state_watermark {
        device.last_state_watermark = timestamp;
        device.update(db).await?;
    }
}
```

### Incremental Sync Query

When Device B reconnects and requests catchup:

```rust
// Device B's watermark: 2025-10-20 14:30:00
StateRequest {
    model_types: ["location", "entry", "volume"],
    since: Some(2025-10-20 14:30:00),  // Single watermark for ALL types
    checkpoint: None,
}

// Device A queries:
// SELECT * FROM locations WHERE updated_at >= '2025-10-20 14:30:00'
// SELECT * FROM entries WHERE updated_at >= '2025-10-20 14:30:00'
// SELECT * FROM volumes WHERE updated_at >= '2025-10-20 14:30:00'
```

## The 10k Entry Problem

### Scenario

1. Device A has:

   - 100 locations (last updated: 14:00:00)
   - 10,000 entries (being indexed, timestamps from 14:00:00 to 14:45:00)
   - 5 volumes (last updated: 14:10:00)

2. Device B starts syncing at 14:30:00

   - Receives location updates
   - Starts receiving entry updates (in batches of 10,000)
   - Watermark updates to 14:32:00 after first entry batch
   - More entries come in with timestamps 14:33:00, 14:34:00, etc.

3. Device B disconnects at 14:35:00 (after syncing 5,000 entries)

   - `last_state_watermark = 14:34:00` (from last entry processed)
   - **Problem:** Volumes with timestamp 14:10:00 < watermark but never synced!

4. Device B reconnects at 14:40:00
   - Sends `StateRequest(since: 14:34:00)`
   - Gets entries with `updated_at >= 14:34:00` ✓
   - **Misses all volumes** because their timestamp (14:10:00) is before the watermark!

### Root Cause

The single `last_state_watermark` timestamp represents the high-water mark across **all resource types**. When one resource type (entries) has many records being synced continuously, it advances the watermark past timestamps of other resource types (volumes, locations) that may not have fully synced yet.

## Why This Manifests at 10k+

1. **Batch size is 10,000** (default `backfill_batch_size`)
2. With <10k entries, everything fits in one batch
3. With >10k entries:
   - First batch: entries 1-10,000
   - Watermark advances to timestamp of entry #10,000
   - If disconnection happens before second batch...
   - **Any resource type with earlier timestamps is lost**

## Evidence from Codebase

### State Request Handler

```rust
// core/src/service/network/protocol/sync/handler.rs:198-264
StateRequest { model_types, since, checkpoint, batch_size } => {
    // Query local state for EACH model type with SAME watermark
    for model_type in model_types {
        let records = peer_sync.get_device_state(
            vec![model_type],
            device_id,
            since,  // ← Same watermark for all model types!
            cursor,
            batch_size,
        ).await?;
    }
}
```

### Query Implementation

```rust
// core/src/service/sync/protocol_handler.rs:152-206
async fn query_state(model_type, device_id, since, cursor, limit) {
    let mut conditions = vec![];

    if let Some(ts) = since {
        conditions.push("updated_at > ?");  // ← Filters by watermark
        values.push(ts);
    }

    // This query misses records with updated_at < watermark
    let query = format!("SELECT * FROM {} WHERE {} ORDER BY updated_at", table_name, conditions);
}
```

### Per-Model Sync Order

```rust
// core/src/service/sync/backfill.rs:240-272
async fn backfill_all_state(since_watermark) {
    // Compute sync order: ["device", "location", "volume", "entry"]
    let sync_order = compute_registry_sync_order().await?;

    for model in sync_order {
        if is_device_owned(&model) {
            // Use SAME watermark for each model type
            backfill_peer_state(peer, model, checkpoint, since_watermark).await?;
            //                                              ^^^^^^^^^^^^^^
            //                                              Same for all!
        }
    }
}
```

## Confirmation: No Per-Resource Tracking

**Database Schema:**

- `devices` table: ONE `last_state_watermark` column (not per-model)
- No `device_model_watermarks` table
- No per-resource tracking anywhere

**Metrics System:**

```rust
// core/src/service/sync/metrics/types.rs:136-150
pub struct DataVolumeMetrics {
    pub last_sync_per_model: Arc<RwLock<HashMap<String, DateTime<Utc>>>>,
    //  ^^^ This is for METRICS only, not used for sync logic
}
```

The metrics track `last_sync_per_model` but **this is not persisted** and **not used for watermark-based catchup**.

## Solution: Per-Resource Watermarks

### Proposed Schema Change

```sql
-- New table to track watermarks per resource type
CREATE TABLE device_resource_watermarks (
    device_id INTEGER NOT NULL,
    peer_device_id TEXT NOT NULL,  -- UUID of device we're tracking
    resource_type TEXT NOT NULL,    -- "location", "entry", "volume", etc.
    last_watermark TEXT NOT NULL,   -- Timestamp in RFC3339 format
    updated_at TEXT NOT NULL,

    PRIMARY KEY (device_id, peer_device_id, resource_type),
    FOREIGN KEY (device_id) REFERENCES devices(id)
);

CREATE INDEX idx_resource_watermarks_lookup
    ON device_resource_watermarks(device_id, peer_device_id, resource_type);
```

### Proposed Update Logic

```rust
// Instead of single watermark update
async fn apply_state_change(&self, change: StateChangeMessage) -> Result<()> {
    crate::infra::sync::apply_state_change(&change.model_type, change.data, db).await?;

    // Update watermark FOR THIS SPECIFIC RESOURCE TYPE
    self.update_resource_watermark(
        change.device_id,
        &change.model_type,  // ← Resource-specific!
        change.timestamp,
    ).await?;
}

// New method
async fn update_resource_watermark(
    &self,
    peer_device_id: Uuid,
    resource_type: &str,
    timestamp: DateTime<Utc>
) -> Result<()> {
    // Upsert into device_resource_watermarks table
    let watermark = DeviceResourceWatermark {
        device_id: self.device_id,
        peer_device_id,
        resource_type: resource_type.to_string(),
        last_watermark: timestamp,
        updated_at: Utc::now(),
    };

    watermark.save(db).await?;
}
```

### Proposed Catchup Query

```rust
async fn request_catchup(&self, peer: Uuid) -> Result<()> {
    let sync_order = compute_registry_sync_order().await?;

    for model_type in sync_order {
        // Get watermark FOR THIS SPECIFIC RESOURCE TYPE
        let watermark = self.get_resource_watermark(peer, &model_type).await?;

        // Request only this resource type with its specific watermark
        let request = StateRequest {
            model_types: vec![model_type.clone()],
            since: watermark,  // ← Per-resource watermark!
            checkpoint: None,
            batch_size: config.backfill_batch_size,
        };

        self.network.send_sync_message(peer, request).await?;
    }
}
```

## Impact Analysis

### Files Requiring Changes

1. **Schema Migration**

   - `core/src/infra/db/migration/` - Add new migration for `device_resource_watermarks` table

2. **Entity Layer**

   - `core/src/infra/db/entities/device_resource_watermark.rs` - New entity

3. **Sync Service**

   - `core/src/service/sync/peer.rs` - Replace `update_state_watermark` with `update_resource_watermark`
   - `core/src/service/sync/peer.rs` - Replace `get_watermarks` with `get_resource_watermarks`

4. **Backfill Logic**

   - `core/src/service/sync/backfill.rs` - Pass per-resource watermarks in `backfill_peer_state`
   - `core/src/service/sync/backfill.rs` - Update watermark persistence after each model type

5. **Protocol Handler**

   - `core/src/service/network/protocol/sync/handler.rs` - Handle per-resource watermark requests

6. **Tests**
   - `core/tests/sync_integration_test.rs` - Update watermark tests
   - Add test for 10k+ entry scenario with mixed resource types

### Backward Compatibility

**Migration Strategy:**

1. Keep existing `last_state_watermark` column for backward compatibility
2. Initialize `device_resource_watermarks` table from existing watermarks
3. For each existing device, create entries:
   ```sql
   INSERT INTO device_resource_watermarks (device_id, peer_device_id, resource_type, last_watermark)
   SELECT id, uuid, 'location', last_state_watermark FROM devices WHERE last_state_watermark IS NOT NULL
   UNION ALL
   SELECT id, uuid, 'entry', last_state_watermark FROM devices WHERE last_state_watermark IS NOT NULL
   UNION ALL
   SELECT id, uuid, 'volume', last_state_watermark FROM devices WHERE last_state_watermark IS NOT NULL;
   ```
4. Deprecate `last_state_watermark` column in future version

## Alternative: Per-Device-Per-Resource In-Memory Tracking

Instead of a database table, track watermarks in memory:

```rust
pub struct PeerSync {
    // Existing fields...

    /// Per-peer, per-resource watermark tracking
    resource_watermarks: Arc<RwLock<HashMap<Uuid, HashMap<String, DateTime<Utc>>>>>,
    //                                      ^^^^         ^^^^^^      ^^^^^^^^^^^^^
    //                                      peer_id      resource    watermark
}
```

**Pros:**

- No schema changes
- Faster lookups
- Simpler implementation

**Cons:**

- Lost on daemon restart (requires full backfill)
- Doesn't persist across sessions
- No historical tracking

**Verdict:** Database approach is better for correctness and persistence.

## Shared Resources (HLC-Based)

Shared resources use HLC-based ordering and appear to work correctly because:

1. **HLC provides total ordering** across all shared resource types
2. **Single `last_shared_watermark`** works because HLC is monotonic
3. **Shared resources sync in HLC order** regardless of type

The problem is specific to **device-owned state** which uses timestamps that can overlap across resource types.

## Recommendations

### Immediate (Fix the Bug)

1. Implement per-resource watermark tracking in database
2. Update sync service to use resource-specific watermarks
3. Add integration test for >10k mixed-resource scenario

### Short-term (Improve Robustness)

1. Add watermark validation before catchup queries
2. Log watermark state transitions for debugging
3. Add metrics for watermark lag per resource type

### Long-term (Architecture)

1. Consider unified sync protocol (HLC for all resources?)
2. Investigate hybrid approach (coarse global + fine per-resource)
3. Add sync health checks that detect watermark drift

## Test Case

```rust
#[tokio::test]
async fn test_sync_10k_mixed_resources() {
    // Device A: Create mixed resources with various timestamps
    for i in 0..5 {
        create_location(format!("loc_{}", i)).await; // Early timestamps
    }

    for i in 0..12000 {
        create_entry(format!("entry_{}", i)).await; // Many entries
    }

    for i in 0..3 {
        create_volume(format!("vol_{}", i)).await; // Mid timestamps
    }

    // Device B: Start sync
    device_b.start_backfill(device_a).await;

    // Simulate disconnection after 5k entries
    tokio::time::sleep(Duration::from_millis(100)).await;
    device_b.disconnect().await;

    // Reconnect and verify all resource types synced
    device_b.reconnect().await;
    device_b.wait_for_sync_complete().await;

    // ASSERTION: All locations, volumes, and entries present
    assert_eq!(device_b.count_locations().await, 5);
    assert_eq!(device_b.count_entries().await, 12000);
    assert_eq!(device_b.count_volumes().await, 3);
}
```

This test should **fail** with current implementation and **pass** after per-resource watermarks.

## References

- **Documentation:** `/docs/core/library-sync.mdx:485-512` (Watermark-Based Incremental Sync)
- **Watermark Storage:** `core/src/infra/db/entities/device.rs:27-31`
- **Watermark Update:** `core/src/service/sync/peer.rs:270-300`
- **State Query:** `core/src/service/sync/protocol_handler.rs:152-206`
- **Backfill Logic:** `core/src/service/sync/backfill.rs:240-364`
- **Test:** `core/tests/sync_integration_test.rs` (search for watermark tests)

## Conclusion

The library sync system's use of **global watermarks** rather than **per-resource watermarks** causes data loss in scenarios where:

- Multiple resource types sync from the same device
- One resource type has significantly more records than others
- Disconnections occur during multi-batch backfill

**The fix requires per-resource watermark tracking** to ensure each resource type can resume sync independently from its last known checkpoint.
