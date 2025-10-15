---
id: LSYNC-002
title: Shared Metadata Sync (Albums, Tags) with HLC
status: To Do
assignee: james
parent: LSYNC-000
priority: High
tags: [sync, metadata, albums, tags, hlc, shared-resources]
depends_on: [LSYNC-006, LSYNC-009, LSYNC-010]
design_doc: core/src/infra/sync/NEW_SYNC.md
---

## Description

Implement synchronization for truly shared resources (Albums, Tags) using the HLC-based log model. These resources can be modified by any device and need conflict resolution.

**Architecture**: Log-based sync with Hybrid Logical Clocks for ordering.

## Data Classification

**Shared Resources** (this task):

- Tags: Global tag definitions (no device owner)
- Albums: Collections referencing entries from multiple devices
- UserMetadata: When scoped to ContentIdentity (content-universal)

**Device-Owned** (separate - state-based):

- Locations: Owned by specific device
- Entries: Owned via location's device
- (Handled by state-based sync, not this task)

## Implementation Steps

1. Mark tags/albums as shared in `Syncable` trait
2. Implement `commit_shared()` in TransactionManager:
   - Generate HLC
   - Write to database
   - Write to `sync.db`
   - Broadcast to all peers
3. Implement conflict resolution:
   - Tags: Deterministic UUID from name (merge duplicates)
   - Albums: Union merge for entry lists
   - UserMetadata: HLC ordering for LWW
4. Implement ACK mechanism for log pruning
5. Test concurrent tag creation across devices
6. Test album modification conflicts

## Technical Details

- Changes written to `sync.db` (per-device log)
- Broadcast via `SharedChange` message with HLC
- Peers apply in HLC order
- ACKs enable aggressive pruning (log stays <1000 entries)

## Conflict Examples

### Tag Name Collision

```
Device A: Creates tag "Vacation" → HLC(1000,A)
Device B: Creates tag "Vacation" → HLC(1001,B)

Resolution: Deterministic UUID from name
  Both generate: Uuid::v5(NAMESPACE, "Vacation")
  Same UUID → automatically merge
```

### Album Concurrent Edits

```
Device A: Adds entry-1 to "Summer" → HLC(1000,A)
Device B: Adds entry-2 to "Summer" → HLC(1001,B)

Resolution: Union merge
  Album contains: [entry-1, entry-2]
```

## Acceptance Criteria

- [ ] Tags sync between all peers
- [ ] Albums sync between all peers
- [ ] Concurrent tag creation merges correctly
- [ ] Album edits merge (union)
- [ ] HLC ordering works
- [ ] `sync.db` stays small (<1MB)
- [ ] ACK mechanism prunes old entries
- [ ] Integration tests validate conflicts

## References

- `core/src/infra/sync/NEW_SYNC.md` - Shared resource sync
- HLC: LSYNC-009
- Sync service: LSYNC-010
