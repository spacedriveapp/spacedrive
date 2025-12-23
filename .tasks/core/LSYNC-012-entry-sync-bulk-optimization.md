---
id: LSYNC-012
title: Bulk Entry Sync Optimization (State-Based)
status: Done
assignee: jamiepine
parent: LSYNC-000
priority: High
tags: [sync, indexing, bulk, performance, state-based]
depends_on: [LSYNC-006, LSYNC-010]
design_doc: core/src/infra/sync/NEW_SYNC.md
last_updated: 2025-12-02
---

## Description

Optimize entry (file/folder) synchronization for bulk indexing operations using efficient state broadcasts. When a device indexes 1M files, avoid sending 1M individual `StateChange` messages by using batched transfers or on-demand loading.

## The Problem

Device A indexes 1M files:

**Naive approach**: Send 1M individual `StateChange` messages

- ~500MB of messages
- 10+ minutes to broadcast
- Network congestion
- Memory pressure on receivers

**This doesn't scale.**

## The Solution: Multi-Strategy Approach

### Strategy 1: Batch State Transfers

```rust
// Device A finishes indexing location
let entries = query_all_entries_for_location(location_id).await?;

// Send in efficient batches
for chunk in entries.chunks(1000) {
    broadcast_to_peers(StateBatch {
        model_type: "entry",
        device_id: MY_DEVICE_ID,
        records: chunk.iter().map(|e| StateRecord {
            uuid: e.uuid,
            data: serde_json::to_value(e)?,
            timestamp: e.updated_at,
        }).collect(),
    }).await?;
}
```

**Benefits**:

- Compressed batches (gzip)
- Streaming application on receiver
- Progress tracking
- Resumable if interrupted

### Strategy 2: Bulk Notification + On-Demand Load

```rust
// Device A finishes indexing
broadcast_to_peers(BulkIndexComplete {
    device_id: MY_DEVICE_ID,
    location_id: location.uuid,
    entry_count: 1_000_000,
    indexed_at: Utc::now(),
}).await?;

// Peers decide what to do:
// Option A: Request entries on-demand (lazy loading)
// Option B: If same location exists, trigger own indexing
// Option C: Request full dump for initial sync
```

**Benefits**:

- Tiny notification (~100 bytes)
- Peers control when to sync (bandwidth-aware)
- Can trigger local indexing if same filesystem

### Strategy 3: Database-Level Replication (Initial Sync)

```rust
// New device joins with 0 entries
// Instead of: Request 1M entries via messages
// Do: Request database snapshot

let snapshot = peer.export_device_state(device_id).await?;
// Returns: SQLite database dump of just Device A's data

import_database_snapshot(snapshot).await?;
// Fast: Direct database import
```

**Benefits**:

- Extremely fast (database native format)
- No serialization overhead
- Atomic import

## Implementation

### Batch Transfers

**File**: `core/src/service/sync/state.rs`

```rust
/// Broadcast large state changes efficiently
pub async fn broadcast_bulk_state(
    &self,
    model_type: &str,
    records: Vec<StateRecord>,
) -> Result<()> {
    const BATCH_SIZE: usize = 1000;

    for (i, chunk) in records.chunks(BATCH_SIZE).enumerate() {
        let batch = SyncMessage::StateBatch {
            model_type: model_type.to_string(),
            device_id: self.device_id,
            batch_index: i,
            total_batches: (records.len() + BATCH_SIZE - 1) / BATCH_SIZE,
            records: chunk.to_vec(),
        };

        // Compress before sending
        let compressed = compress_batch(&batch)?;

        // Broadcast to all peers
        for peer in self.get_connected_sync_partners().await? {
            self.send_to_peer(peer, compressed.clone()).await?;
        }

        // Rate limit to avoid overwhelming receivers
        tokio::time::sleep(Duration::from_millis(10)).await;
    }

    Ok(())
}
```

### On-Demand Loading

**File**: `core/src/service/sync/state.rs`

```rust
/// Handle bulk notification from peer
async fn on_bulk_index_complete(
    &self,
    notification: BulkIndexComplete,
) -> Result<()> {
    info!(
        device = %notification.device_id,
        location = %notification.location_id,
        count = notification.entry_count,
        "Peer completed bulk indexing"
    );

    // Check if we need this data
    let should_sync = self.should_sync_location(
        notification.device_id,
        notification.location_id,
    ).await?;

    if should_sync {
        // Queue background job to fetch entries
        self.queue_bulk_fetch_job(
            notification.device_id,
            notification.location_id,
        ).await?;
    } else {
        // Just record that peer has this data (for future queries)
        self.update_peer_state(notification).await?;
    }

    Ok(())
}
```

### Database Snapshot Export

**File**: `core/src/service/sync/snapshot.rs`

```rust
/// Export device-owned state as database snapshot
pub async fn export_device_snapshot(
    &self,
    device_id: Uuid,
) -> Result<Vec<u8>> {
    // Create temporary database
    let temp_db = Database::memory().await?;

    // Copy only this device's data
    let locations = location::Entity::find()
        .filter(location::Column::DeviceId.eq(device_id))
        .all(self.db.conn())
        .await?;

    let entries = entry::Entity::find()
        .filter(entry::Column::DeviceId.eq(device_id))  // Via location
        .all(self.db.conn())
        .await?;

    // Insert into temp database
    for loc in locations {
        loc.insert(temp_db.conn()).await?;
    }
    for entry in entries {
        entry.insert(temp_db.conn()).await?;
    }

    // Export as binary blob
    let snapshot = temp_db.export_to_bytes().await?;

    Ok(snapshot)
}
```

## When to Use Each Strategy

| Scenario                       | Strategy                      | Reason               |
| ------------------------------ | ----------------------------- | -------------------- |
| New device joins               | Database snapshot             | Fast initial sync    |
| Incremental sync (few changes) | Individual StateChange        | Simple, immediate    |
| Large batch (100-10K entries)  | Batched StateBatch            | Efficient, streaming |
| Massive index (100K+ entries)  | Bulk notification + on-demand | Bandwidth-aware      |

## Performance Comparison

| Method                   | 1M Entries        | Network  | Time   | Memory |
| ------------------------ | ----------------- | -------- | ------ | ------ |
| Individual messages      | 500MB             | High     | 10 min | Low    |
| Batched (1K chunks)      | 50MB (compressed) | Medium   | 2 min  | Medium |
| Bulk notification + lazy | 1KB notification  | Minimal  | Async  | Low    |
| Database snapshot        | 150MB             | One-time | 30 sec | High   |

## Acceptance Criteria

- [ ] Batched state transfers implemented
- [ ] Compression for large batches (gzip)
- [ ] Bulk notification message type
- [ ] On-demand entry loading
- [ ] Database snapshot export/import
- [ ] Strategy selection based on entry count
- [ ] Progress tracking for large transfers
- [ ] Resumable batch transfers
- [ ] Performance test: 1M entries sync in <2 minutes

## Integration Points

### TransactionManager

```rust
impl TransactionManager {
    /// Commit bulk entries (indexer use case)
    pub async fn commit_bulk_entries(
        &self,
        library: Arc<Library>,
        entries: Vec<entry::ActiveModel>,
    ) -> Result<()> {
        // Write to database
        bulk_insert(entries).await?;

        // Don't create 1M sync messages!
        // Instead: Emit bulk completion event
        event_bus.emit(Event::BulkIndexComplete {
            device_id: MY_DEVICE_ID,
            location_id,
            entry_count: entries.len(),
        });

        // SyncService handles efficient broadcast
    }
}
```

### SyncService

```rust
impl SyncService {
    async fn on_bulk_index_complete(&self, event: BulkIndexComplete) {
        // Decide strategy based on peer state
        if self.is_initial_sync() {
            // Offer database snapshot
            self.broadcast_snapshot_available().await?;
        } else {
            // Send batched state
            self.broadcast_bulk_state(entries).await?;
        }
    }
}
```

## Migration from Leader Model

**Old approach**: Bulk operations in sync log with sequence numbers
**New approach**: Efficient state batching, no central log

**Changes needed**:

- Remove bulk operation sync log entries
- Add batching to state broadcasts
- Add database snapshot capability
- Strategy selection logic

## References

- `core/src/infra/sync/NEW_SYNC.md` - Leaderless architecture
- State-based sync: LSYNC-010
- Batch size tuning: Benchmark with 10K, 100K, 1M entries
