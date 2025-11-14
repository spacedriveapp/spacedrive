# Library Sync Watermark Issue - Visual Summary

## The Problem in One Picture

```
Device A Timeline (10k+ entries):
Time:     14:00    14:10    14:20    14:30    14:40    14:50
          │        │        │        │        │        │
Locations: ●●●●    (done)
          (100)
          
Volumes:            ●●●●●   (done)
                    (5)
                    
Entries:                     ●●●●●●●●●●●●●●●●●●●●●●●●●●●●●
                            (10,000 entries, continuous indexing)
                            Batch 1    Batch 2    Batch 3
                            (10k)      (5k)       (2k)

Device B Sync:
Connects:                    ▼
Gets batch 1:                └─────────┐ 
Watermark:                             ↑ (14:32)
Disconnects:                              ▼
Watermark stored:                         ↑ (14:32)

Device B Reconnects:                              ▼
Requests since:                                   ↑ (14:32)
Gets: Entries >= 14:32 ✓
      Volumes >= 14:32 ✗  (were at 14:10, missed!)
      Locations >= 14:32 ✗ (were at 14:00, missed!)
```

## Current Architecture

### Watermark Structure (Per Device)

```
devices table:
┌─────────┬──────────────────────────┬────────────────────────────┐
│ uuid    │ last_state_watermark     │ last_shared_watermark      │
├─────────┼──────────────────────────┼────────────────────────────┤
│ dev-B   │ 2025-10-20 14:32:00      │ HLC(timestamp, counter, id)│
│         │ ▲                        │ ▲                          │
│         │ │                        │ │                          │
│         │ SINGLE timestamp for:   │ SINGLE HLC for:           │
│         │ • locations             │ • tags                    │
│         │ • entries               │ • collections             │
│         │ • volumes               │ • content_identities      │
│         │ • ALL device-owned data │ • ALL shared resources    │
└─────────┴──────────────────────────┴────────────────────────────┘
```

### Sync Request with Global Watermark

```rust
// Device B reconnects and sends:
StateRequest {
    model_types: ["location", "entry", "volume"],  // Multiple types!
    since: Some(2025-10-20 14:32:00),              // One watermark for all!
    checkpoint: None,
    batch_size: 10000,
}

// Device A processes with this query for EACH type:
// SELECT * FROM locations WHERE updated_at >= '14:32:00'  → 0 results (updated at 14:00)
// SELECT * FROM entries WHERE updated_at >= '14:32:00'    → remaining entries ✓
// SELECT * FROM volumes WHERE updated_at >= '14:32:00'    → 0 results (updated at 14:10)
```

## Why It Breaks at 10k

### Batch Size Configuration
```rust
// core/src/infra/sync/config.rs:176
pub struct BatchingConfig {
    pub backfill_batch_size: 10_000,  // Default batch size
    // ...
}
```

### The Critical Sequence

```
1. Device B starts backfill:
   ┌─────────────────────────────────────────┐
   │ Backfill Order (by dependencies):      │
   │ 1. Devices                              │
   │ 2. Locations        ← 100 records       │
   │ 3. Volumes          ← 5 records         │
   │ 4. Entries          ← 10,000+ records   │
   └─────────────────────────────────────────┘

2. First batch processes:
   • Devices: All synced (watermark: ~14:00)
   • Locations: All synced (watermark: ~14:05)
   • Volumes: All synced (watermark: ~14:10)
   • Entries (batch 1 of 3): 10,000 synced (watermark: 14:32)
   
   ️  WATERMARK ADVANCED TO 14:32 after entry batch!

3. Disconnection occurs before second entry batch

4. Reconnection attempts to resume:
   since = 14:32
   
   Locations with timestamp 14:00-14:05 → SKIPPED
   Volumes with timestamp 14:10       → SKIPPED  
   ✓  Entries with timestamp >= 14:32   → Synced
```

## Code Evidence

### Single Watermark Update (The Bug)

```rust
// core/src/service/sync/peer.rs:1689-1746
async fn apply_state_change(&self, change: StateChangeMessage) -> Result<()> {
    // Apply change for ANY model type
    crate::infra::sync::apply_state_change(
        &change.model_type,  // Could be "location", "entry", "volume", etc.
        change.data,
        self.db
    ).await?;
    
    // ️  BUG: Update GLOBAL watermark regardless of model_type
    self.update_state_watermark(change.device_id, change.timestamp).await?;
    //   ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
    //   This overwrites watermark for ALL resource types!
}
```

### Watermark Query (Uses Same Timestamp for All Resources)

```rust
// core/src/service/sync/backfill.rs:275-363
async fn backfill_peer_state(
    peer: Uuid,
    model_types: Vec<String>,  // ["location", "entry", "volume"]
    checkpoint: Option<BackfillCheckpoint>,
    since_watermark: Option<DateTime<Utc>>,  // ← SAME for all types!
) -> Result<BackfillCheckpoint> {
    for model_type in model_types {
        // Request with SAME watermark for EACH type
        let response = self.request_state_batch(
            peer,
            vec![model_type.clone()],
            cursor_checkpoint.clone(),
            since_watermark,  // ← Problem! Same timestamp for all!
            batch_size,
        ).await?;
    }
}
```

### Database Query Filters by Watermark

```rust
// core/src/infra/db/entities/entry.rs:118-146
async fn query_for_sync(
    device_id: Option<Uuid>,
    since: Option<DateTime<Utc>>,  // ← The global watermark
    cursor: Option<(DateTime<Utc>, Uuid)>,
    batch_size: usize,
    db: &DatabaseConnection,
) -> Result<Vec<(Uuid, serde_json::Value, DateTime<Utc>)>> {
    let mut query = Entity::find();
    
    // Filter by watermark timestamp
    if let Some(since_time) = since {
        query = query.filter(Column::IndexedAt.gte(since_time));
        //                                      ^^^
        //     This filters out entries older than watermark!
    }
}
```

## The Solution: Per-Resource Watermarks

### New Schema

```
device_resource_watermarks table:
┌───────────┬────────────────┬───────────────┬─────────────────────────┐
│ device_id │ peer_device_id │ resource_type │ last_watermark          │
├───────────┼────────────────┼───────────────┼─────────────────────────┤
│ dev-A     │ dev-B          │ location      │ 2025-10-20 14:05:00     │
│ dev-A     │ dev-B          │ entry         │ 2025-10-20 14:32:00     │
│ dev-A     │ dev-B          │ volume        │ 2025-10-20 14:10:00     │
│ dev-A     │ dev-B          │ content_id    │ 2025-10-20 14:25:00     │
└───────────┴────────────────┴───────────────┴─────────────────────────┘
                                ▲                      ▲
                                │                      │
                          Separate watermark     Per-resource!
                          per resource type      Independent advancement!
```

### Fixed Watermark Update

```rust
async fn apply_state_change(&self, change: StateChangeMessage) -> Result<()> {
    // Apply change
    crate::infra::sync::apply_state_change(&change.model_type, change.data, self.db).await?;
    
    //  FIX: Update resource-specific watermark
    self.update_resource_watermark(
        change.device_id,
        &change.model_type,  // ← Resource type included!
        change.timestamp,
    ).await?;
}

async fn update_resource_watermark(
    &self,
    peer_device_id: Uuid,
    resource_type: &str,
    timestamp: DateTime<Utc>,
) -> Result<()> {
    // Upsert into per-resource watermark table
    DeviceResourceWatermark::upsert(
        self.device_id,
        peer_device_id,
        resource_type,
        timestamp,
        db,
    ).await
}
```

### Fixed Backfill Query

```rust
async fn backfill_peer_state(
    peer: Uuid,
    model_types: Vec<String>,
    checkpoint: Option<BackfillCheckpoint>,
) -> Result<BackfillCheckpoint> {
    for model_type in model_types {
        //  FIX: Get watermark FOR THIS SPECIFIC resource type
        let resource_watermark = self.get_resource_watermark(
            peer,
            &model_type,  // ← Resource-specific lookup!
        ).await?;
        
        // Request with resource-specific watermark
        let response = self.request_state_batch(
            peer,
            vec![model_type.clone()],
            cursor_checkpoint.clone(),
            resource_watermark,  // ← Different per type!
            batch_size,
        ).await?;
    }
}
```

## Visual: Before and After

### BEFORE (Broken)

```
Watermark State after 5k entries synced:
┌─────────────────────────────────────┐
│ Device B's view of Device A:       │
│                                     │
│ last_state_watermark: 14:32:00     │
│   ↓ applies to:                    │
│   • locations    (WRONG!)          │
│   • entries      (correct)         │
│   • volumes      (WRONG!)          │
└─────────────────────────────────────┘

Query Results on Reconnect (since 14:32):
✗ Locations (updated 14:05) → Skipped
✓ Entries   (updated 14:32+) → Synced  
✗ Volumes   (updated 14:10) → Skipped
```

### AFTER (Fixed)

```
Watermark State after 5k entries synced:
┌─────────────────────────────────────┐
│ Device B's view of Device A:       │
│                                     │
│ resource_watermarks:                │
│   • location → 14:05:00            │
│   • entry    → 14:32:00            │
│   • volume   → 14:10:00            │
└─────────────────────────────────────┘

Query Results on Reconnect:
✓ Locations (since 14:05) → Synced
✓ Entries   (since 14:32) → Synced  
✓ Volumes   (since 14:10) → Synced
```

## Impact of 10k Batch Size

### < 10k Total Entries
```
Single batch contains ALL data:
┌────────────────────────────────────┐
│ Batch 1:                           │
│ • 100 locations                    │
│ • 5,000 entries                    │
│ • 5 volumes                        │
│ Total: 5,105 < 10,000 ✓           │
└────────────────────────────────────┘
Result: Everything syncs in one go
        No watermark advancement bug
```

### > 10k Total Entries
```
Multiple batches required:
┌────────────────────────────────────┐
│ Batch 1 (10,000 limit):            │
│ • 100 locations   ✓                │
│ • 9,895 entries   ✓                │
│ • 5 volumes      ✓                 │
│ Total: 10,000                      │
│ Watermark → 14:32                  │
└────────────────────────────────────┘

️  Disconnection here!

┌────────────────────────────────────┐
│ Batch 2 (would be requested):      │
│ • 5,105 more entries               │
│ But query filters by 14:32...     │
│ Locations/volumes already OLDER!   │
└────────────────────────────────────┘

Result: Locations and volumes LOST
```

## Why Shared Resources Don't Have This Bug

```
Shared resources use HLC (Hybrid Logical Clock):

HLC Properties:
• Monotonically increasing
• Globally ordered
• Resource-agnostic by design

Example:
┌────────────────────────────────────┐
│ Shared Resource Events:            │
│ HLC(14:00:00, 0, dev-A) → tag1     │
│ HLC(14:05:00, 0, dev-B) → album1   │
│ HLC(14:10:00, 0, dev-A) → tag2     │
│ HLC(14:15:00, 0, dev-A) → content1 │
└────────────────────────────────────┘

Query: since_hlc = HLC(14:05:00, 0, dev-B)
Returns: ALL events after 14:05:00
         (tag2, content1)
         
✓ Works correctly because:
  • HLC provides total ordering
  • All shared resources in same timeline
  • No interleaving of resource types
```

## When the Bug Manifests

### Conditions Required

```
1. Multiple resource types syncing from same device
   ✓ Common: locations + entries + volumes

2. At least one resource type has > batch_size records
   ✓ Common: entries often exceed 10k

3. Resource types have different timestamp ranges
   ✓ Common: locations indexed first, entries indexed over time

4. Disconnection during multi-batch sync
   ✓ Common: network issues, daemon restarts, user actions
```

### Real-World Scenario

```
User has external drive with:
• 50 locations (directories)
• 500,000 files (entries)
• 1 volume

Indexing timeline:
00:00 - Scan starts, create volume
00:05 - Create location records
00:10 - Start indexing entries (continuous)
01:00 - 50% indexed (250k entries)

Sync timeline:
00:15 - Peer connects, starts backfill
00:20 - Syncs volume (timestamp 00:00)
00:21 - Syncs locations (timestamps 00:05)
00:22 - Syncs entries batch 1 (10k, timestamps 00:10-00:15)
00:23 - Watermark now 00:15
00:24 - Network hiccup, disconnection

Reconnect at 01:30:
- Request since 00:15
- Volume (00:00) SKIPPED ❌
- Locations (00:05) SKIPPED  
- Remaining entries synced ✓

Result: Drive appears with files but NO volume/location metadata!
```

## Diagnostic Queries

To detect if you're affected:

```sql
-- Check watermark state
SELECT 
    d.uuid as device,
    d.last_state_watermark,
    COUNT(DISTINCT l.id) as location_count,
    COUNT(DISTINCT e.id) as entry_count,
    COUNT(DISTINCT v.id) as volume_count
FROM devices d
LEFT JOIN locations l ON l.device_id = d.id
LEFT JOIN entries e ON e.location_id = l.id  
LEFT JOIN volumes v ON v.device_id = d.id
GROUP BY d.uuid;

-- Find entries with timestamps before watermark but not synced
SELECT 
    e.uuid,
    e.indexed_at,
    d.last_state_watermark,
    CASE 
        WHEN e.indexed_at < d.last_state_watermark THEN 'SHOULD_BE_SYNCED'
        ELSE 'NOT_YET_DUE'
    END as sync_status
FROM entries e
JOIN locations l ON e.location_id = l.id
JOIN devices d ON l.device_id = d.id
WHERE e.indexed_at < d.last_state_watermark
ORDER BY e.indexed_at;
```

## Next Steps

1. **Immediate**: Implement per-resource watermark table
2. **Testing**: Add integration test for >10k mixed resources
3. **Migration**: Populate initial watermarks from existing global watermarks
4. **Monitoring**: Add metrics for per-resource sync lag
5. **Validation**: Ensure no data loss during migration

See `SYNC_WATERMARK_ANALYSIS.md` for full implementation details.

