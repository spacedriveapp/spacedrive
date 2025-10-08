---
id: LSYNC-012
title: Entry Sync with Bulk Optimization
status: To Do
assignee: unassigned
parent: LSYNC-000
priority: High
tags: [sync, indexing, bulk, performance]
depends_on: [LSYNC-006, LSYNC-010]
---

## Description

Implement entry (file/folder) synchronization with bulk optimization. When a device indexes 1M files, it creates ONE metadata sync log instead of 1M individual entries. Other devices trigger their own local indexing when they see this notification.

## The Problem

Naive approach: Index 1M files → Create 1M sync log entries → 500MB sync log → 10 minutes to replicate

**This doesn't scale.**

## The Solution

Bulk operations create metadata-only sync logs:
```json
{
  "sequence": 1234,
  "model_type": "bulk_operation",
  "operation": "InitialIndex",
  "location_id": "uuid-...",
  "affected_count": 1000000,
  "hints": { "location_path": "/Users/alice/Photos" }
}
```

Other devices see this, check if they have a matching location, and trigger their own indexing job.

## Implementation Steps

1. Create `BulkOperation` enum (InitialIndex, WatcherBatch)
2. Update `commit_bulk()` in TransactionManager
3. Create bulk operation sync log entries
4. Implement `handle_bulk_operation()` in sync follower
5. Match location by path/fingerprint on remote device
6. Queue local IndexerJob when match found
7. Handle watcher batches (10-1K items) with per-item logs

## Performance Impact

- **Before**: 1M entries, 500MB, 10 minutes, 3M operations
- **After**: 1 entry, 500 bytes, <1 second, 1M operations
- **Result**: 10x faster, 1 million times smaller!

## Technical Details

- Initial indexing: Always bulk (1K+ items)
- Watcher events: Batch if 10-1K, per-item if <10
- User operations: Always per-item (instant sync)
- Location matching: By path or volume fingerprint

## Acceptance Criteria

- [ ] Bulk operations create metadata-only sync logs
- [ ] Follower triggers local indexing on bulk notification
- [ ] 1M file indexing creates <10KB sync log
- [ ] Watcher batches use appropriate strategy
- [ ] Location matching across devices works
- [ ] Performance tests validate 10x improvement

## References

- `docs/core/sync.md` lines 172-196 (bulk operations)
- `docs/core/sync.md` lines 495-502 (performance metrics)
