---
id: FSYNC-004
title: Service Integration & API
status: To Do
assignee: unassigned
parent: FSYNC-000
priority: Medium
tags: [api, integration, routes, events]
design_doc: workbench/FILE_SYNC_IMPLEMENTATION_PLAN.md
last_updated: 2025-10-14
related_tasks: [FSYNC-003]
---

## Description

Wire FileSyncService into Spacedrive infrastructure - add service to CoreContext, implement API routes for UI integration, and set up event handling for real-time updates.

**Goal:** Make File Sync accessible from UI with full CRUD, sync triggering, and status monitoring.

## Implementation Steps

### 1. Add to Services

```rust
// Location: core/src/service/mod.rs

pub struct Services {
    // ... existing services
    pub file_sync: Arc<FileSyncService>,
}

impl Services {
    pub fn new(context: Arc<CoreContext>) -> Self {
        // ... initialize other services

        let file_sync = Arc::new(FileSyncService::new(
            context.library_db(),
            context.job_manager.clone(),
            context.clone(),
        ));

        Self {
            // ... other services
            file_sync,
        }
    }
}
```

### 2. Create API Routes

```rust
// Location: core/src/api/sync.rs

pub fn mount() -> Router {
    Router::new()
        // Conduit Management
        .query("listConduits", |ctx, _: ()| async move { ... })
        .mutation("createConduit", |ctx, input: CreateConduitInput| async move { ... })
        .mutation("updateConduit", |ctx, input: UpdateConduitInput| async move { ... })
        .mutation("deleteConduit", |ctx, conduit_id: i32| async move { ... })

        // Sync Operations
        .mutation("syncNow", |ctx, conduit_id: i32| async move { ... })
        .mutation("pauseSync", |ctx, conduit_id: i32| async move { ... })
        .mutation("resumeSync", |ctx, conduit_id: i32| async move { ... })

        // Status & History
        .query("getSyncStatus", |ctx, conduit_id: i32| async move { ... })
        .query("getSyncProgress", |ctx, conduit_id: i32| async move { ... })
        .query("getSyncHistory", |ctx, input: HistoryInput| async move { ... })
        .query("getConflicts", |ctx, conduit_id: i32| async move { ... })
}
```

**API Types:**
```rust
// Request/Response types

#[derive(Type, Deserialize)]
pub struct CreateConduitInput {
    pub source_entry_id: i32,
    pub target_entry_id: i32,
    pub sync_mode: String,           // "mirror" | "bidirectional" | "selective"
    pub schedule: String,            // "instant" | "interval:5m" | "manual"
}

#[derive(Type, Serialize)]
pub struct SyncConduitResponse {
    pub id: i32,
    pub uuid: String,
    pub source_entry_id: i32,
    pub target_entry_id: i32,
    pub sync_mode: String,
    pub enabled: bool,
    pub last_sync_completed_at: Option<String>,
    pub total_syncs: i64,
    pub files_synced: i64,
    pub bytes_transferred: i64,
}

#[derive(Type, Serialize)]
pub struct SyncStatusResponse {
    pub conduit_id: i32,
    pub is_syncing: bool,
    pub current_phase: Option<String>,  // "copying" | "deleting" | "verifying"
    pub last_completed: Option<String>,
    pub last_error: Option<String>,
}

#[derive(Type, Serialize)]
pub struct SyncProgressResponse {
    pub conduit_id: i32,
    pub phase: String,
    pub total_files: usize,
    pub completed_files: usize,
    pub total_bytes: u64,
    pub completed_bytes: u64,
    pub copy_progress: Option<CopyProgress>,
    pub delete_progress: Option<DeleteProgress>,
}
```

### 3. Event Integration

```rust
// Location: core/src/service/file_sync/events.rs

pub enum FileSyncEvent {
    ConduitCreated { conduit_id: i32, uuid: Uuid },
    ConduitUpdated { conduit_id: i32 },
    ConduitDeleted { conduit_id: i32 },

    SyncStarted { conduit_id: i32, generation: i64 },
    SyncProgress { conduit_id: i32, progress: SyncProgress },
    SyncCompleted { conduit_id: i32, generation: i64, stats: SyncStats },
    SyncFailed { conduit_id: i32, error: String },

    ConflictDetected { conduit_id: i32, conflict: SyncConflict },
    VerificationComplete { conduit_id: i32, verified: bool },
}

impl FileSyncService {
    fn emit_event(&self, event: FileSyncEvent) {
        if let Some(event_bus) = &self.context.event_bus {
            event_bus.emit("file_sync", serde_json::to_value(event).unwrap());
        }
    }
}
```

**Event Emission Points:**
- sync_now() → SyncStarted
- monitor_sync() progress loop → SyncProgress
- monitor_sync() completion → SyncCompleted
- monitor_sync() error → SyncFailed
- resolver → ConflictDetected
- verification → VerificationComplete

### 4. Register API Router

```rust
// Location: core/src/api/mod.rs

pub fn mount() -> Router {
    Router::new()
        // ... existing routes
        .merge("sync.", crate::api::sync::mount())
}
```

## Files to Create/Modify

**API Implementation:**
- `core/src/api/sync.rs` - API routes and types

**Event Integration:**
- `core/src/service/file_sync/events.rs` - Event types and emission

**Service Registration:**
- `core/src/service/mod.rs` - Add file_sync to Services struct
- `core/src/api/mod.rs` - Register sync router

## Acceptance Criteria

- [ ] FileSyncService added to Services struct
- [ ] Service initializes with correct dependencies
- [ ] API routes implemented for conduit CRUD
- [ ] syncNow mutation triggers sync and returns handle
- [ ] getSyncStatus query returns current sync state
- [ ] getSyncProgress query returns aggregated progress
- [ ] getSyncHistory query returns generation history
- [ ] Events emitted at appropriate lifecycle points
- [ ] UI can create conduit between two directories
- [ ] UI can trigger manual sync
- [ ] UI shows real-time progress via events
- [ ] UI displays sync history and statistics
- [ ] Integration test: Full API workflow (create → sync → monitor)

## API Usage Examples

### Create Sync Conduit

```typescript
const conduit = await core.sync.createConduit({
  sourceEntryId: 123,
  targetEntryId: 456,
  syncMode: "mirror",
  schedule: "manual",
});
```

### Trigger Sync

```typescript
const handle = await core.sync.syncNow(conduit.id);

// Subscribe to progress
core.events.on("file_sync", (event) => {
  if (event.type === "SyncProgress" && event.conduit_id === conduit.id) {
    console.log(
      `Progress: ${event.progress.completed_files}/${event.progress.total_files}`
    );
  }
});
```

### Monitor Status

```typescript
const status = await core.sync.getSyncStatus(conduit.id);
console.log(status.is_syncing, status.last_completed);

const progress = await core.sync.getSyncProgress(conduit.id);
console.log(progress.phase, progress.completed_bytes, progress.total_bytes);
```

## UI Integration Points

**Location Context Menu:**
- "Sync to..." option on directory right-click
- Opens modal to select target location and configure sync mode

**Sync Status Panel:**
- List of all conduits with status indicators
- Per-conduit progress bars during active sync
- History view showing past generations

**Settings:**
- Configure schedule, bandwidth limits, conflict resolution
- Enable/disable conduits
- View and resolve conflicts

## References

- Implementation: FILE_SYNC_IMPLEMENTATION_PLAN.md (Lines 1499-1630)
- API patterns: core/src/api/jobs.rs (similar structure)
- Related: FSYNC-003 (FileSyncService core), FSYNC-005 (progress aggregation)
