---
id: LSYNC-011
title: Sync Conflict Resolution (Optimistic Concurrency)
status: To Do
assignee: unassigned
parent: LSYNC-000
priority: Medium
tags: [sync, conflict-resolution, versioning]
depends_on: [LSYNC-010]
---

## Description

Implement conflict resolution for sync entries using optimistic concurrency control. All Syncable models have a version field; when applying updates, the system compares versions to determine which change wins.

## Implementation Steps

1. Implement `apply_model_change()` with version checking
2. Add Last-Write-Wins (LWW) strategy
3. Handle Insert/Update/Delete operations
4. Skip updates when local version is newer
5. Log conflicts for debugging/monitoring
6. Add optional conflict resolution UI hooks (future)

## Conflict Strategy

- **Last-Write-Wins (LWW)**: Use version field to determine winner
- **Insert**: Always apply (no conflict possible)
- **Update**: Compare versions, skip if local >= remote
- **Delete**: Always apply (tombstone)
- **User Metadata** (tags, albums): Union merge (future)

## Technical Details

- Location: `core/src/service/sync/conflict.rs`
- Version field: Monotonically increasing integer
- Timestamp-based versioning for some models
- No CRDTs in Phase 1 (simpler, sufficient for metadata)

## Example Logic

```rust
if remote_model.version > local_model.version {
    // Remote is newer - apply update
    remote_model.update(db).await?;
} else {
    // Local is newer or same - skip
    tracing::debug!("Skipping sync entry: local version is newer");
}
```

## Acceptance Criteria

- [ ] Version comparison logic implemented
- [ ] Conflicts resolved automatically
- [ ] Conflicts logged for monitoring
- [ ] Unit tests cover all conflict scenarios
- [ ] Integration tests validate cross-device conflicts

## Future Enhancements

- CRDT-based merge for rich text fields
- User-facing conflict resolution UI
- Conflict metrics and alerting

## References

- `docs/core/sync.md` lines 403-443
