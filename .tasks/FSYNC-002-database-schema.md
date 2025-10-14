---
id: FSYNC-002
title: Database Schema & Entities
status: To Do
assignee: unassigned
parent: FSYNC-000
priority: High
tags: [database, schema, migration, entities]
design_doc: workbench/FILE_SYNC_IMPLEMENTATION_PLAN.md
last_updated: 2025-10-14
---

## Description

Create persistent storage for sync state and history. Define SyncConduit and SyncGeneration entities with SeaORM migrations to track sync relationships and execution history.

**Goal:** Persistent storage enabling resumable sync, conflict detection, and sync history tracking.

## Entities

### SyncConduit

Represents a sync relationship between two directories.

```rust
// Location: core/src/entities/sync_conduit.rs

pub struct Model {
    pub id: i32,
    pub uuid: Uuid,

    // Endpoints - both must be Entry records of type Directory
    pub source_entry_id: i32,
    pub target_entry_id: i32,

    // Configuration
    pub sync_mode: String,            // "mirror" | "bidirectional" | "selective"
    pub enabled: bool,
    pub schedule: String,             // "instant" | "interval:5m" | "manual"

    // Index settings
    pub use_index_rules: bool,        // Default: true
    pub index_mode_override: Option<String>,

    // Performance tuning
    pub parallel_transfers: i32,
    pub bandwidth_limit_mbps: Option<i32>,

    // State tracking
    pub last_sync_completed_at: Option<DateTimeUtc>,
    pub sync_generation: i64,
    pub last_sync_error: Option<String>,

    // Statistics
    pub total_syncs: i64,
    pub files_synced: i64,
    pub bytes_transferred: i64,

    pub created_at: DateTimeUtc,
    pub updated_at: DateTimeUtc,
}
```

### SyncGeneration

Tracks individual sync operations for history and verification.

```rust
// Location: core/src/entities/sync_generation.rs

pub struct Model {
    pub id: i32,
    pub conduit_id: i32,
    pub generation: i64,
    pub started_at: DateTimeUtc,
    pub completed_at: Option<DateTimeUtc>,

    // Operation summary
    pub files_copied: i32,
    pub files_deleted: i32,
    pub conflicts_resolved: i32,
    pub bytes_transferred: i64,
    pub errors_encountered: i32,

    // Verification tracking (Trust Watcher approach)
    pub verified_at: Option<DateTimeUtc>,
    pub verification_status: String, // "unverified" | "waiting_watcher" | "verified" | "failed:reason"
}
```

## Implementation Steps

### 1. Create Entity Files

**SyncConduit Entity:**
- `core/src/entities/sync_conduit.rs`
- Define Model struct with all fields
- Implement Relation enum (foreign keys to Entry)
- Add SyncMode enum with as_str() and from_str()

**SyncGeneration Entity:**
- `core/src/entities/sync_generation.rs`
- Define Model struct
- Implement Relation enum (foreign key to SyncConduit)

### 2. Create Migration

```rust
// Location: core/src/migrations/m20250315_000001_create_sync_tables.rs

async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    // Create sync_conduit table with all columns
    // Create sync_generation table with all columns
    // Create indexes:
    //   - idx_sync_conduit_enabled (enabled)
    //   - idx_sync_generation_conduit (conduit_id, generation)
}
```

### 3. Register Migration

Add migration to migration list in `core/src/migrations/mod.rs`

### 4. Update Entity Module

Export new entities in `core/src/entities/mod.rs`

## Database Schema

### sync_conduit Table

```sql
CREATE TABLE sync_conduit (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    uuid BLOB NOT NULL UNIQUE,
    source_entry_id INTEGER NOT NULL,
    target_entry_id INTEGER NOT NULL,
    sync_mode TEXT NOT NULL,
    enabled INTEGER NOT NULL DEFAULT 1,
    schedule TEXT NOT NULL DEFAULT 'manual',
    use_index_rules INTEGER NOT NULL DEFAULT 1,
    index_mode_override TEXT,
    parallel_transfers INTEGER NOT NULL DEFAULT 3,
    bandwidth_limit_mbps INTEGER,
    last_sync_completed_at TEXT,
    sync_generation INTEGER NOT NULL DEFAULT 0,
    last_sync_error TEXT,
    total_syncs INTEGER NOT NULL DEFAULT 0,
    files_synced INTEGER NOT NULL DEFAULT 0,
    bytes_transferred INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    FOREIGN KEY (source_entry_id) REFERENCES entry(id) ON DELETE CASCADE,
    FOREIGN KEY (target_entry_id) REFERENCES entry(id) ON DELETE CASCADE
);

CREATE INDEX idx_sync_conduit_enabled ON sync_conduit(enabled);
```

### sync_generation Table

```sql
CREATE TABLE sync_generation (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    conduit_id INTEGER NOT NULL,
    generation INTEGER NOT NULL,
    started_at TEXT NOT NULL,
    completed_at TEXT,
    files_copied INTEGER NOT NULL DEFAULT 0,
    files_deleted INTEGER NOT NULL DEFAULT 0,
    conflicts_resolved INTEGER NOT NULL DEFAULT 0,
    bytes_transferred INTEGER NOT NULL DEFAULT 0,
    errors_encountered INTEGER NOT NULL DEFAULT 0,
    verified_at TEXT,
    verification_status TEXT NOT NULL DEFAULT 'unverified',
    FOREIGN KEY (conduit_id) REFERENCES sync_conduit(id) ON DELETE CASCADE
);

CREATE INDEX idx_sync_generation_conduit ON sync_generation(conduit_id, generation);
```

## Acceptance Criteria

- [ ] SyncConduit entity created with all fields
- [ ] SyncGeneration entity created with all fields
- [ ] SyncMode enum with Mirror, Bidirectional, Selective variants
- [ ] Migration creates both tables with correct schema
- [ ] Foreign key constraints properly defined
- [ ] Indexes created for query optimization
- [ ] Migration registered in migration list
- [ ] Entities exported in entities module
- [ ] Migration runs successfully: `cargo run --bin spacedrive migrate up`
- [ ] Tables queryable via SeaORM

## Relationships

```
sync_conduit
  ├─> entry (source_entry_id)
  ├─> entry (target_entry_id)
  └─> sync_generation (one-to-many)
```

Both source and target entries must be directories (kind=1). The conduit creates a directed relationship, though Bidirectional mode allows changes to flow both ways.

## Technical Notes

**Verification Status Values:**
- `unverified` - Sync completed, not yet verified
- `waiting_watcher` - Waiting for filesystem watcher to update index
- `waiting_library_sync` - Waiting for library sync to propagate changes
- `verified` - Verification query confirms consistency
- `failed:<reason>` - Verification detected remaining differences

**Why Trust Watcher?**
Option A (Trust Watcher) chosen over Option B (Eager Update) because:
- Single source of truth: Watcher already maintains index consistency
- No duplication: Sync service doesn't need filesystem semantics
- Eventual consistency: System naturally converges to consistent state

## References

- Implementation: FILE_SYNC_IMPLEMENTATION_PLAN.md (Lines 316-636)
- Documentation: docs/core/file-sync.mdx (Lines 180-238)
- Related: FSYNC-003 (uses these entities), FSYNC-005 (verification flow)
