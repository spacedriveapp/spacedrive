---
id: FSYNC-005
title: Advanced Features (Scheduling, Progress, Conflicts)
status: To Do
assignee: unassigned
parent: FSYNC-000
priority: Medium
tags: [scheduler, progress, conflicts, polish]
design_doc: workbench/FILE_SYNC_IMPLEMENTATION_PLAN.md
last_updated: 2025-10-14
related_tasks: [FSYNC-003, FSYNC-004]
---

## Description

Polish File Sync with production-ready features: automatic scheduling, aggregated progress tracking, advanced conflict resolution strategies, and monitoring capabilities.

**Goal:** Transform MVP into production-ready system with excellent user experience.

## Implementation Steps

### 1. Schedule Processing

```rust
// Location: core/src/service/file_sync/scheduler.rs

impl FileSyncService {
    pub async fn start_scheduler(self: Arc<Self>) {
        tokio::spawn(async move {
            self.run_scheduler().await
        });
    }

    async fn run_scheduler(&self) -> Result<()> {
        let mut tick = interval(Duration::from_secs(60));

        loop {
            tick.tick().await;
            self.process_scheduled_syncs().await?;
        }
    }

    async fn process_scheduled_syncs(&self) -> Result<()> {
        let enabled = self.conduit_manager.list_enabled().await?;

        for conduit in enabled {
            if self.is_sync_due(&conduit).await {
                info!("Triggering scheduled sync for conduit {}", conduit.id);
                self.sync_now(conduit.id).await?;
            }
        }
    }

    async fn is_sync_due(&self, conduit: &sync_conduit::Model) -> bool {
        match conduit.schedule.as_str() {
            "manual" => false,
            "instant" => true,  // TODO: integrate with file watcher
            s if s.starts_with("interval:") => {
                // Parse "interval:5m", "interval:1h" etc.
                self.parse_interval_due(s, conduit.last_sync_completed_at)
            }
            _ => false,
        }
    }
}
```

**Schedule Formats:**
- `"manual"` - Only triggered via API
- `"instant"` - Triggers on filesystem change (requires watcher integration)
- `"interval:5m"` - Every 5 minutes
- `"interval:1h"` - Every hour
- `"interval:1d"` - Daily

**Watcher Integration (Instant Mode):**
```rust
// Subscribe to location watcher events
// When files change in source or target directory:
//   - Check if conduit has instant schedule
//   - Debounce rapid changes (wait 5s after last change)
//   - Trigger sync_now()
```

### 2. Progress Aggregation

```rust
// Location: core/src/service/file_sync/progress.rs

#[derive(Debug, Clone, Serialize)]
pub struct AggregatedSyncProgress {
    pub phase: String,                    // "copying" | "deleting" | "verifying"
    pub copy_progress: Option<CopyProgress>,
    pub delete_progress: Option<DeleteProgress>,
    pub total_files: usize,
    pub completed_files: usize,
    pub total_bytes: u64,
    pub completed_bytes: u64,
    pub current_speed_mbps: f64,
    pub eta_seconds: Option<u64>,
}

impl FileSyncService {
    pub async fn get_sync_progress(
        &self,
        conduit_id: i32,
    ) -> Result<Option<AggregatedSyncProgress>> {
        let syncs = self.active_syncs.read().await;
        let Some(sync_op) = syncs.get(&conduit_id) else {
            return Ok(None);
        };

        let mut progress = AggregatedSyncProgress::default();

        // Aggregate copy job progress
        if let Some(job_id) = sync_op.source_to_target.copy_job_id {
            if let Ok(copy_prog) = self.job_manager.get_progress(job_id).await {
                progress.copy_progress = Some(copy_prog);
                progress.total_files += copy_prog.total_files;
                progress.completed_files += copy_prog.completed_files;
                progress.total_bytes += copy_prog.total_bytes;
                progress.completed_bytes += copy_prog.completed_bytes;
            }
        }

        // Aggregate delete job progress
        if let Some(job_id) = sync_op.source_to_target.delete_job_id {
            if let Ok(delete_prog) = self.job_manager.get_progress(job_id).await {
                progress.delete_progress = Some(delete_prog);
                progress.total_files += delete_prog.total_files;
                progress.completed_files += delete_prog.completed_files;
            }
        }

        // Calculate speed and ETA
        progress.current_speed_mbps = self.calculate_current_speed(&sync_op);
        progress.eta_seconds = self.calculate_eta(&progress);

        Ok(Some(progress))
    }
}
```

**Progress Tracking:**
- Query job manager for individual job progress
- Aggregate totals across all active jobs
- Calculate transfer speed from job metrics
- Estimate completion time

### 3. Advanced Conflict Resolution

```rust
// Location: core/src/service/file_sync/conflict.rs

pub struct ConflictResolver {
    strategy: ConflictStrategy,
}

pub enum ConflictStrategy {
    NewestWins,           // Use most recent modification (default)
    SourceWins,           // Source always wins
    TargetWins,           // Target always wins
    LargestWins,          // Keep larger file (useful for media)
    CreateConflictFile,   // Create "file (conflict 2025-10-14).txt"
    PromptUser,           // Queue for user decision
}

pub enum ConflictResolution {
    UseSource,
    UseTarget,
    CreateConflictCopy {
        original: entry::Model,
        conflicted: entry::Model,
    },
    PromptUser(SyncConflict),
}

impl ConflictResolver {
    pub fn resolve(&self, conflict: SyncConflict) -> ConflictResolution {
        match self.strategy {
            ConflictStrategy::NewestWins => {
                if conflict.source_entry.updated_at > conflict.target_entry.updated_at {
                    ConflictResolution::UseSource
                } else {
                    ConflictResolution::UseTarget
                }
            }
            ConflictStrategy::LargestWins => {
                if conflict.source_entry.size > conflict.target_entry.size {
                    ConflictResolution::UseSource
                } else {
                    ConflictResolution::UseTarget
                }
            }
            ConflictStrategy::CreateConflictFile => {
                // Generate conflict filename with timestamp
                ConflictResolution::CreateConflictCopy {
                    original: conflict.target_entry,
                    conflicted: conflict.source_entry,
                }
            }
            ConflictStrategy::PromptUser => {
                // Queue for UI resolution
                ConflictResolution::PromptUser(conflict)
            }
            _ => { /* ... */ }
        }
    }
}
```

**Conflict Filename Format:**
```
original.txt
→ original (conflict 2025-10-14 Device-Name).txt
```

### 4. Monitoring & Telemetry

```rust
// Location: core/src/service/file_sync/telemetry.rs

pub struct SyncTelemetry {
    pub total_conduits: usize,
    pub active_syncs: usize,
    pub total_syncs_24h: i64,
    pub total_bytes_24h: i64,
    pub average_sync_duration: Duration,
    pub conflict_count_24h: i64,
    pub error_count_24h: i64,
}

impl FileSyncService {
    pub async fn get_telemetry(&self) -> Result<SyncTelemetry> {
        // Query database for statistics
        // Aggregate across all conduits
        // Calculate averages and totals
    }
}
```

**Metrics to Track:**
- Total conduits created
- Active sync count
- Syncs completed (24h, 7d, 30d)
- Total bytes transferred
- Average sync duration
- Conflict frequency
- Error rates

## Files to Create

**Scheduler:**
- `core/src/service/file_sync/scheduler.rs` - Background scheduler

**Progress:**
- `core/src/service/file_sync/progress.rs` - Progress aggregation

**Conflicts:**
- `core/src/service/file_sync/conflict.rs` - Conflict resolution strategies (enhanced)

**Monitoring:**
- `core/src/service/file_sync/telemetry.rs` - Telemetry and metrics

## Acceptance Criteria

- [ ] Scheduler runs in background checking for due syncs
- [ ] Interval schedules parsed correctly (5m, 1h, 1d formats)
- [ ] Instant mode triggers on filesystem changes (with debouncing)
- [ ] get_sync_progress aggregates across copy and delete jobs
- [ ] Progress includes current speed and ETA
- [ ] ConflictResolver supports all strategies
- [ ] CreateConflictFile generates unique filenames
- [ ] PromptUser queues conflicts for UI resolution
- [ ] Telemetry endpoint returns accurate statistics
- [ ] UI displays real-time progress with speed and ETA
- [ ] UI shows conflict queue with resolution options
- [ ] Integration test: Scheduled sync triggers automatically
- [ ] Integration test: Conflict resolution strategies work correctly

## User Experience Improvements

**Real-Time Progress:**
- Show current file being copied
- Display transfer speed (MB/s)
- Show ETA for completion
- Indicate phase (copying/deleting/verifying)

**Conflict Management:**
- Highlight conflicts in sync status
- Preview both versions before resolution
- Batch resolution for multiple conflicts
- Remember user's preferred strategy

**Scheduling UI:**
- Visual schedule picker
- Next sync time indicator
- Manual sync button always available
- Pause/resume controls

## References

- Implementation: FILE_SYNC_IMPLEMENTATION_PLAN.md (Lines 1856-2073)
- Scheduler: Lines 1921-2014
- Progress: Lines 2016-2073
- Related: FSYNC-003 (resolver conflicts), FSYNC-004 (API for conflicts)
