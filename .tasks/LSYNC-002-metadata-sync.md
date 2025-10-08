---
id: LSYNC-002
title: Metadata Sync (Albums, Tags, Locations)
status: To Do
assignee: unassigned
parent: LSYNC-000
priority: High
tags: [sync, metadata, albums, tags, volumes]
depends_on: [LSYNC-006, LSYNC-010]
---

## Description

Implement metadata synchronization for user-created resources (Albums, Tags, Locations) using the sync system. This is the first practical application of the TransactionManager and sync follower service.

**Note**: This task focuses on rich metadata (albums, tags), NOT file entries. File entry sync is handled separately with bulk optimization (see LSYNC-012-entry-sync.md).

## Implementation Steps

1. Implement `Syncable` trait for `albums::Model`
2. Implement `Syncable` trait for `tags::Model`
3. Implement `Syncable` trait for `locations::Model`
4. Implement `Syncable` trait for `volumes::Model`
4. Update album/tag/location/volume actions to use TransactionManager
5. Verify sync logs created on leader device
6. Verify follower service applies changes correctly
7. Test cross-device album/tag creation and updates

## Technical Details

- Albums: Sync name, cover, description
- Tags: Sync name, color
- Locations: Sync metadata only (path is device-specific)
- Entry relationships: Sync via junction tables

## Acceptance Criteria

- [ ] Albums sync between devices
- [ ] Tags sync between devices
- [ ] Location metadata syncs
- [ ] Changes appear instantly in client cache
- [ ] Conflict resolution works for concurrent edits
- [ ] Integration tests validate cross-device sync

## References

- See `docs/core/sync.md` for domain sync strategy
