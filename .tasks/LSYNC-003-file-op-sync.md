---
id: LSYNC-003
title: Cross-Device File Operations (Future Phase)
status: To Do
assignee: unassigned
parent: LSYNC-000
priority: Low
tags: [sync, file-ops, future]
---

## Description

Enable file operations (copy, move, delete) to be executed across devices. This is a **future phase** feature - not part of the initial sync implementation.

**Current Architecture**: File operations are device-local. If you delete a file on Device A, only the metadata syncs (the Entry is marked deleted). Device B sees the metadata change but does NOT delete its local file copy.

**Future Goal**: User can optionally enable "sync conduits" where file operations replicate across devices. Example: Delete on Device A → Device B also deletes local file.

## Implementation Steps (Future)

1. Design "sync conduit" configuration (which locations participate)
2. File operation actions emit special sync log entries
3. Follower service recognizes file-op entries
4. Follower executes corresponding local file operation
5. Handle conflicts (file already deleted, moved, etc.)
6. Add user controls for sync conduit policies

## Why Not Phase 1?

- Metadata sync is complex enough initially
- File operations need robust conflict resolution
- Users may not want all devices to mirror operations
- Bandwidth/storage considerations (mobile devices)

## Acceptance Criteria

- [ ] Sync conduit configuration schema
- [ ] File operations create special sync log type
- [ ] Follower can execute file operations
- [ ] Conflict resolution for file ops
- [ ] User can enable/disable per location pair

## References

- `docs/core/sync.md` - Sync domains (Content future phase)
