---
id: LSYNC-011
title: Conflict Resolution (HLC-Based)
status: Done
assignee: james
parent: LSYNC-000
priority: Medium
tags: [sync, conflict-resolution, hlc, merge]
depends_on: [LSYNC-009, LSYNC-010]
design_doc: core/src/infra/sync/NEW_SYNC.md
last_updated: 2025-10-15
---

## Description

Implement conflict resolution for shared resources using Hybrid Logical Clock (HLC) ordering and domain-specific merge strategies.

**Architecture**: HLC provides total ordering for conflict resolution without requiring a leader.

## Conflict Types

### 1. No Conflict (Device-Owned Data)

```
Device A: Creates location "/Users/jamie/Photos"
Device B: Creates location "/home/jamie/Documents"

Resolution: No conflict! Different devices own different data
Strategy: Both apply (state-based)
```

### 2. Deterministic Merge (Tags)

```
Device A: Creates tag "Vacation" → HLC(1000,A)
Device B: Creates tag "Vacation" → HLC(1001,B)

Resolution: Deterministic UUID from name
  Uuid::v5(NAMESPACE, "Vacation")
  Both devices generate same UUID
  Automatically merge (same record)
```

### 3. Union Merge (Albums)

```
Device A: Adds entry-1 to album → HLC(1000,A)
Device B: Adds entry-2 to album → HLC(1001,B)

Resolution: Union merge
  Album.entry_uuids = [entry-1, entry-2]
  Both additions preserved
```

### 4. Last-Writer-Wins (UserMetadata)

```
Device A: Favorites photo → HLC(1000,A)
Device B: Un-favorites photo → HLC(1001,B)

Resolution: HLC ordering
  HLC(1001,B) > HLC(1000,A)
  Device B's change wins
  Photo is NOT favorited
```

## Implementation

**File**: `core/src/service/sync/conflict.rs`

```rust
pub enum MergeStrategy {
    NoConflict,          // Device-owned, always apply
    DeterministicUUID,   // Tags (same name = same UUID)
    UnionMerge,          // Albums, tag lists
    LastWriterWins,      // Metadata fields (favorite, hidden)
    Manual,              // Complex conflicts (future)
}

pub async fn resolve_conflict(
    local: Model,
    remote: SharedChangeEntry,
    strategy: MergeStrategy,
) -> Result<Model> {
    match strategy {
        MergeStrategy::NoConflict => {
            // Just apply remote (state-based, no conflicts)
            Ok(remote.data.into())
        }

        MergeStrategy::DeterministicUUID => {
            // Check if UUIDs match
            if local.uuid == remote.record_uuid {
                // Same UUID, merge fields
                merge_fields(local, remote)
            } else {
                // Different UUID, both exist
                Ok(remote.data.into())
            }
        }

        MergeStrategy::UnionMerge => {
            // Combine arrays/sets
            let mut merged = local;
            merged.entry_uuids.extend(remote.entry_uuids);
            merged.entry_uuids.dedup();
            Ok(merged)
        }

        MergeStrategy::LastWriterWins => {
            // Compare HLCs (already ordered by protocol)
            // Remote always wins if we're applying it
            Ok(remote.data.into())
        }

        MergeStrategy::Manual => {
            // Store conflict for UI resolution
            store_conflict(local, remote).await?;
            Err(ConflictError::RequiresManualResolution)
        }
    }
}
```

## HLC-Based Ordering

The protocol ensures changes are applied in HLC order:

```rust
async fn apply_shared_changes(changes: Vec<SharedChangeEntry>) {
    // Sort by HLC (total ordering)
    changes.sort_by_key(|c| c.hlc);

    // Apply in order
    for change in changes {
        apply_with_conflict_resolution(change).await?;
    }
}
```

**Property**: If applied in HLC order, all devices converge to same state!

## Acceptance Criteria

- [x] Conflict resolution for tags (HLC-based LWW in apply_shared_change)
- [x] Conflict resolution for collections (HLC-based upsert)
- [x] Conflict resolution for user metadata (LWW via HLC)
- [x] HLC ordering ensures consistency (peer_log tracks latest HLC per record)
- [x] Peer.rs apply_shared_change checks HLC before applying
- [x] Integration test validates concurrent tag updates converge correctly
- [x] Test: `test_concurrent_tag_updates_hlc_conflict_resolution` passing
- [x] PeerLog.get_latest_hlc_for_record enables conflict detection

## Future Enhancements

- User-facing conflict resolution UI
- CRDT-based merge for rich text
- Conflict metrics and monitoring

## References

- `core/src/infra/sync/NEW_SYNC.md` - Conflict resolution section
- HLC: LSYNC-009
- Merge strategies: Lines 773-796 in NEW_SYNC.md
