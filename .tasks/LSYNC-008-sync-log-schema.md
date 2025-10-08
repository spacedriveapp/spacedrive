---
id: LSYNC-008
title: Sync Log Database Schema & Entity (Separate DB)
status: To Do
assignee: unassigned
parent: LSYNC-000
priority: High
tags: [sync, database, schema, migration]
---

## Description

Create the sync log database schema and SeaORM entity. The sync log is an append-only, sequentially-ordered log of all state changes per library, maintained by the leader device.

**Architecture Decision**: The sync log lives in its own separate database (`sync_log.db`) in the Library data folder rather than in the main library database. This provides:
- Better performance (no query contention)
- Easier maintenance (vacuum, archive old entries)
- Cleaner separation (infrastructure vs domain data)
- Simpler backup/restore (library can be backed up without sync log)

## Implementation Steps

1. Create sync log database connection per library
2. Create migration for `sync_log` table (in sync DB)
3. Create SeaORM entity `core/src/infra/db/entities/sync_log.rs`
4. Add indexes for efficient querying:
   - `(sequence)` - Primary lookup for sync (library_id not needed since it's per-library DB)
   - `(device_id)` - Filter by originating device
   - `(model_type, record_id)` - Find changes to specific records
5. Add unique constraint on `(sequence)`
6. Create `SyncLogDb` wrapper for database lifecycle
7. Create helper methods for querying sync entries

## Schema

```sql
CREATE TABLE sync_log (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    sequence INTEGER NOT NULL UNIQUE,  -- Monotonic per library (unique since this DB is per-library)
    device_id TEXT NOT NULL,            -- Device that created this entry
    timestamp TEXT NOT NULL,

    -- Change details
    model_type TEXT NOT NULL,           -- "album", "tag", "entry", "bulk_operation"
    record_id TEXT NOT NULL,            -- UUID of changed record
    change_type TEXT NOT NULL,          -- "insert", "update", "delete", "bulk_insert"
    version INTEGER NOT NULL DEFAULT 1, -- Optimistic concurrency

    -- Data payload (JSON)
    data TEXT NOT NULL
);

CREATE INDEX idx_sync_log_sequence ON sync_log(sequence);
CREATE INDEX idx_sync_log_device ON sync_log(device_id);
CREATE INDEX idx_sync_log_model_record ON sync_log(model_type, record_id);
CREATE INDEX idx_sync_log_timestamp ON sync_log(timestamp);
```

**Note**: `library_id` field removed since each library has its own sync log database.

## Database Location

- Path: `~/.spacedrive/libraries/{library_uuid}/sync.db`
- One sync log DB per library
- Created automatically when library is opened
- Managed by `SyncLogDb` wrapper

## SyncLogDb Wrapper

```rust
pub struct SyncLogDb {
    library_id: Uuid,
    conn: DatabaseConnection,
}

impl SyncLogDb {
    /// Open or create sync log DB for library
    pub async fn open(library_id: Uuid, data_dir: &Path) -> Result<Self, DbError>;

    /// Append entry to sync log (leader only)
    pub async fn append(&self, entry: SyncLogEntry) -> Result<u64, DbError>;

    /// Fetch entries since sequence
    pub async fn fetch_since(&self, sequence: u64, limit: usize) -> Result<Vec<SyncLogEntry>, DbError>;

    /// Get latest sequence number
    pub async fn latest_sequence(&self) -> Result<u64, DbError>;

    /// Vacuum old entries (> 30 days)
    pub async fn vacuum_old_entries(&self, before: DateTime<Utc>) -> Result<usize, DbError>;
}
```

## Acceptance Criteria

- [ ] Per-library sync log database created
- [ ] Migration created and tested
- [ ] SeaORM entity implemented
- [ ] Indexes created for performance
- [ ] SyncLogDb wrapper implemented
- [ ] Helper methods for common queries
- [ ] Database lifecycle managed correctly
- [ ] Documentation of schema design

## References

- `docs/core/sync.md` lines 211-236
