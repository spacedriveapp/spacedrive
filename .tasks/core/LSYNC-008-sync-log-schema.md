---
id: LSYNC-008
title: Sync Log Schema (Per-Device, HLC-Based)
status: Done
assignee: james
parent: LSYNC-000
priority: High
tags: [sync, database, schema, migration, hlc]
design_doc: core/src/infra/sync/NEW_SYNC.md
last_updated: 2025-10-14
---

## Description

Create the `sync.db` schema - a per-device log of changes to truly shared resources (tags, albums). Uses HLC for ordering instead of central sequences.

**Architecture Change**: Replaces central `sync_log.db` (leader only) with per-device `sync.db` (all devices).

## Key Differences from Old Design

| Aspect           | Old (sync_log.db)   | New (sync.db)               |
| ---------------- | ------------------- | --------------------------- |
| **Who has it**   | Leader only         | Every device                |
| **What's in it** | All changes         | Only MY shared changes      |
| **Ordering**     | Sequence numbers    | HLC timestamps              |
| **Size**         | Large (all history) | Small (pruned aggressively) |
| **Purpose**      | Source of truth     | Pending changes queue       |

## Implementation Steps

1. Create `sync.db` separate database per library
2. Create migration for `shared_changes` table
3. Create migration for `peer_acks` table
4. Create SeaORM entities
5. Add HLC-based indexes
6. Create `SharedChangesDb` wrapper
7. Implement pruning logic (delete when all peers ack)

## Schema

```sql
-- MY changes to shared resources
CREATE TABLE shared_changes (
    hlc TEXT PRIMARY KEY,           -- Hybrid Logical Clock (sortable string)
    model_type TEXT NOT NULL,       -- "tag", "album", "user_metadata"
    record_uuid TEXT NOT NULL,      -- UUID of changed record
    change_type TEXT NOT NULL,      -- "insert", "update", "delete"
    data TEXT NOT NULL,             -- JSON payload
    created_at TEXT NOT NULL,
);

CREATE INDEX idx_shared_changes_hlc ON shared_changes(hlc);
CREATE INDEX idx_shared_changes_model ON shared_changes(model_type);
CREATE INDEX idx_shared_changes_record ON shared_changes(record_uuid);

-- Track which peers have acked which HLCs (for pruning)
CREATE TABLE peer_acks (
    peer_device_id TEXT NOT NULL,
    last_acked_hlc TEXT NOT NULL,
    acked_at TEXT NOT NULL,
    PRIMARY KEY (peer_device_id)
);

CREATE INDEX idx_peer_acks_hlc ON peer_acks(last_acked_hlc);
```

## Database Location

Each library has:

```
Jamie's Library.sdlibrary/
  ├── database.db  ← Shared state (all devices)
  └── sync.db      ← MY pending shared changes (pruned)
```

## SyncDb Wrapper

```rust
pub struct SyncDb {
    library_id: Uuid,
    device_id: Uuid,
    conn: DatabaseConnection,
}

impl SyncDb {
    /// Open or create sync DB
    pub async fn open(
        library_id: Uuid,
        device_id: Uuid,
        data_dir: &Path,
    ) -> Result<Self, DbError>;

    /// Append shared change entry
    pub async fn append(&self, entry: SharedChangeEntry) -> Result<HLC, DbError>;

    /// Get changes since HLC
    pub async fn get_since(&self, since: Option<HLC>, limit: usize)
        -> Result<Vec<SharedChangeEntry>, DbError>;

    /// Record peer ACK
    pub async fn record_ack(&self, peer: Uuid, hlc: HLC) -> Result<(), DbError>;

    /// Prune entries all peers have acked
    pub async fn prune_acked(&self) -> Result<usize, DbError> {
        // Get minimum HLC across all peers
        let min_acked = self.get_min_acked_hlc().await?;

        if let Some(min_hlc) = min_acked {
            // Delete entries everyone has
            let deleted = shared_changes::Entity::delete_many()
                .filter(shared_changes::Column::Hlc.lte(min_hlc.to_string()))
                .exec(&self.conn)
                .await?
                .rows_affected;

            Ok(deleted as usize)
        } else {
            Ok(0)
        }
    }
}
```

## Pruning Strategy

```rust
// After receiving ACK from peer
async fn on_ack(peer_id: Uuid, up_to_hlc: HLC) {
    // Record ACK
    sync_db.record_ack(peer_id, up_to_hlc).await?;

    // Try to prune
    let pruned = sync_db.prune_acked().await?;

    if pruned > 0 {
        info!(pruned, "Pruned shared changes log");
    }
}
```

**Result**: Log stays small! Typically <100 entries even with active use.

## Acceptance Criteria

- [ ] `sync.db` created per library
- [ ] Migration created and tested
- [ ] SeaORM entities implemented
- [ ] HLC-based indexes
- [ ] SyncDb wrapper functional
- [ ] Pruning logic works
- [ ] Log stays small (<1000 entries under normal use)
- [ ] Documentation complete

## Migration from sync_log.db

**Old structure**:

- One `sync_log.db` on leader
- Sequence-based
- Never pruned

**New structure**:

- One `sync.db` per device
- HLC-based
- Aggressively pruned

## References

- `core/src/infra/sync/NEW_SYNC.md` - Shared changes log design
- HLC: LSYNC-009
- Pruning: Lines 407-429 in NEW_SYNC.md
