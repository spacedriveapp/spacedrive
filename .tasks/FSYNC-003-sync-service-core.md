---
id: FSYNC-003
title: FileSyncService Core Implementation
status: To Do
assignee: unassigned
parent: FSYNC-000
priority: High
tags: [service, core, orchestration, resolver]
design_doc: workbench/FILE_SYNC_IMPLEMENTATION_PLAN.md
last_updated: 2025-10-14
related_tasks: [FSYNC-001, FSYNC-002]
---

## Description

Implement the core FileSyncService - a service-based orchestrator that calculates sync operations from index queries and dispatches FileCopyJob and DeleteJob to perform work.

**Architecture:** Service dispatches jobs, doesn't execute operations directly. This enables proper separation of concerns and reuses battle-tested file operation infrastructure.

## Service Structure

```rust
// Location: core/src/service/file_sync/mod.rs

pub struct FileSyncService {
    db: Arc<DatabaseConnection>,
    job_manager: Arc<JobManager>,
    context: Arc<CoreContext>,
    conduit_manager: Arc<ConduitManager>,
    resolver: Arc<SyncResolver>,
    active_syncs: Arc<RwLock<HashMap<i32, SyncOperation>>>,
}
```

**Key Components:**
- **ConduitManager**: CRUD operations for sync conduits
- **SyncResolver**: Calculates operations from index queries
- **Active syncs tracker**: Prevents duplicate syncs, enables progress monitoring

## Implementation Steps

### 1. Create Service Module Structure

```
core/src/service/file_sync/
  ├── mod.rs           - FileSyncService main implementation
  ├── conduit.rs       - ConduitManager (CRUD operations)
  ├── resolver.rs      - SyncResolver (index queries & diff calculation)
  ├── conflict.rs      - ConflictResolver (bidirectional mode)
  └── history.rs       - Generation history queries
```

### 2. Implement ConduitManager

```rust
// core/src/service/file_sync/conduit.rs

pub struct ConduitManager {
    db: Arc<DatabaseConnection>,
}

impl ConduitManager {
    pub async fn create_conduit(...) -> Result<sync_conduit::Model>
    pub async fn get_conduit(id: i32) -> Result<sync_conduit::Model>
    pub async fn list_enabled() -> Result<Vec<sync_conduit::Model>>
    pub async fn update_after_sync(conduit_id: i32) -> Result<()>
    pub async fn create_generation(...) -> Result<sync_generation::Model>
    pub async fn complete_generation(gen_id: i32) -> Result<()>
}
```

**Responsibilities:**
- Validate entries are directories before creating conduit
- Check for duplicate conduits
- Manage generation records
- Update statistics after successful sync

### 3. Implement SyncResolver

```rust
// core/src/service/file_sync/resolver.rs

pub struct SyncResolver {
    db: Arc<DatabaseConnection>,
}

impl SyncResolver {
    pub async fn calculate_operations(
        &self,
        conduit: &sync_conduit::Model,
    ) -> Result<SyncOperations>
}

pub struct SyncOperations {
    pub source_to_target: DirectionalOps,
    pub target_to_source: Option<DirectionalOps>, // Bidirectional only
    pub conflicts: Vec<SyncConflict>,
}

pub struct DirectionalOps {
    pub to_copy: Vec<EntryWithPath>,
    pub to_delete: Vec<EntryWithPath>,
}
```

**Index Query Logic:**
1. Load entries recursively for both source and target
2. Build path maps (relative path → entry)
3. Apply mode-specific resolution:
   - **Mirror**: source_only → copy, target_only → delete
   - **Bidirectional**: detect changes since last sync, find conflicts
   - **Selective**: (future) access pattern filtering

**Key Method:**
```rust
fn resolve_mirror(
    source_map: &HashMap<PathBuf, entry::Model>,
    target_map: &HashMap<PathBuf, entry::Model>,
) -> Result<SyncOperations> {
    // Files in source but not target → copy
    // Files differ (content_id mismatch) → copy
    // Files in target but not source → delete
}
```

### 4. Implement FileSyncService Core

```rust
// core/src/service/file_sync/mod.rs

impl FileSyncService {
    pub async fn sync_now(&self, conduit_id: i32) -> Result<SyncHandle> {
        // 1. Load conduit and verify enabled
        // 2. Check if already syncing
        // 3. Verify library sync is complete
        // 4. Calculate operations via resolver
        // 5. Create new generation
        // 6. Dispatch job batches (copy + delete)
        // 7. Track active sync
        // 8. Start monitoring background task
        // 9. Return handle
    }

    async fn dispatch_job_batch(
        &self,
        conduit: &sync_conduit::Model,
        operations: &DirectionalOps,
        direction: &str,
    ) -> Result<JobBatch> {
        // Dispatch FileCopyJob if to_copy is not empty
        // Dispatch DeleteJob if to_delete is not empty
        // Return JobBatch with job IDs
    }

    async fn monitor_sync_internal(...) -> Result<()> {
        // Phase 1: Wait for copy jobs to complete
        // Phase 2: Wait for delete jobs to complete (after copies)
        // Phase 3: Mark sync as complete
        // Phase 4: Start verification (Trust Watcher approach)
        // Phase 5: Remove from active syncs
    }
}
```

**Job Ordering Critical Rule:**
Copies must complete before deletes to prevent data loss.

### 5. Implement Conflict Resolution

```rust
// core/src/service/file_sync/conflict.rs

pub struct ConflictResolver {
    strategy: ConflictStrategy,
}

pub enum ConflictStrategy {
    NewestWins,           // Use most recent modification
    SourceWins,           // Source always wins
    TargetWins,           // Target always wins
    CreateConflictFile,   // Create "file (conflict).txt"
    PromptUser,           // UI intervention required
}
```

## Files to Create

**Core Service:**
- `core/src/service/file_sync/mod.rs` - Main service implementation
- `core/src/service/file_sync/conduit.rs` - ConduitManager
- `core/src/service/file_sync/resolver.rs` - SyncResolver
- `core/src/service/file_sync/conflict.rs` - ConflictResolver
- `core/src/service/file_sync/history.rs` - Generation history queries

## Acceptance Criteria

- [ ] FileSyncService struct with ConduitManager and SyncResolver
- [ ] ConduitManager implements full CRUD for conduits
- [ ] SyncResolver calculates operations from index queries
- [ ] Mirror mode sync works end-to-end (MVP)
- [ ] sync_now() creates generation and dispatches jobs
- [ ] Job ordering enforced: copies complete before deletes
- [ ] Active sync tracking prevents duplicate syncs
- [ ] Monitor task waits for jobs and updates state
- [ ] Library sync verification prevents operation when index stale
- [ ] Trust Watcher verification flow implemented
- [ ] Integration test: Mirror sync copies files source → target
- [ ] Integration test: Mirror sync deletes extraneous files from target
- [ ] Integration test: Cannot start sync when library sync incomplete

## Verification Flow (Trust Watcher)

```rust
async fn complete_sync_with_verification(...) -> Result<()> {
    // 1. Mark sync as complete
    // 2. Update status to "waiting_watcher"
    // 3. Trigger watcher scan on both endpoints
    // 4. Wait for watcher completion
    // 5. Update status to "waiting_library_sync"
    // 6. Wait for library sync round
    // 7. Re-run sync resolution
    // 8. If no operations needed → "verified"
    // 9. If operations still needed → "failed:<reason>", optionally retry
}
```

**Why Trust Watcher?**
- Single source of truth: Watcher maintains index consistency
- No duplication: Sync doesn't need filesystem semantics
- Handles concurrent changes: User modifications during sync detected naturally
- Eventual consistency: System converges over multiple sync rounds

## Performance Considerations

**Index Queries:**
- Use location_id filtering for efficient entry queries
- Implement pagination for very large directories
- Cache path maps during resolution

**Job Batching:**
- Parallel copy jobs (configurable via parallel_transfers setting)
- Sequential deletes after all copies complete
- Progress aggregation from job manager

## References

- Implementation: FILE_SYNC_IMPLEMENTATION_PLAN.md (Lines 637-1855)
- Resolver logic: Lines 1133-1430
- Verification: Lines 1699-1855
- Related: FSYNC-001 (DeleteStrategy), FSYNC-002 (entities), FILE-001 (FileCopyJob)
