<!--CREATED: 2025-06-18-->
# Spacedrive Job System Design v2

## Executive Summary

This document presents a redesigned job system for Spacedrive that dramatically reduces boilerplate while maintaining the power needed for complex operations like indexing. The new design leverages Rust's type system and the existing task-system crate to provide a clean, extensible API.

## Core Design Principles

1. **Zero Boilerplate**: Define jobs as simple async functions with a derive macro
2. **Auto-Registration**: Use `inventory` crate for compile-time job discovery
3. **Type-Safe Progress**: Structured progress reporting, not string-based
4. **Layered Architecture**: Jobs built on top of task-system for execution
5. **Library-Scoped**: Each library has its own job database
6. **Resumable by Design**: Automatic state persistence at checkpoints

## Architecture Overview

```
┌─────────────────────────────────────────────────┐
│            Application Layer                     │
│  (Copy Job, Indexer Job, Thumbnail Job, etc.)   │
└─────────────────────┬───────────────────────────┘
                      │
┌─────────────────────┴───────────────────────────┐
│              Job System Layer                    │
│  (Scheduling, Persistence, Progress, Registry)   │
└─────────────────────┬───────────────────────────┘
                      │
┌─────────────────────┴───────────────────────────┐
│             Task System Layer                    │
│  (Execution, Parallelism, Interruption)         │
└─────────────────────┬───────────────────────────┘
                      │
┌─────────────────────┴───────────────────────────┐
│              Worker Pool                         │
│         (CPU-bound thread pool)                  │
└─────────────────────────────────────────────────┘
```

## Job Definition API

### Simple Job Example - File Copy

```rust
use spacedrive_jobs::prelude::*;

#[derive(Job)]
#[job(name = "file_copy")]
pub struct FileCopyJob {
    sources: Vec<SdPath>,
    destination: SdPath,
    #[job(persist = false)]  // Don't persist this field
    options: CopyOptions,
}

#[job_handler]
impl FileCopyJob {
    async fn run(&mut self, ctx: JobContext) -> JobResult {
        let total = self.sources.len();
        ctx.progress(Progress::count(0, total));
        
        for (i, source) in self.sources.iter().enumerate() {
            // Check for interruption
            ctx.check_interrupt().await?;
            
            // Perform copy
            let dest_path = self.destination.join(source.file_name()?);
            copy_file(source, &dest_path).await?;
            
            // Update progress
            ctx.progress(Progress::count(i + 1, total));
            
            // Checkpoint - job can be resumed from here
            ctx.checkpoint().await?;
        }
        
        Ok(JobOutput::FileCopy { 
            copied_count: total,
            total_bytes: ctx.metrics().bytes_processed,
        })
    }
}
```

### Complex Job Example - Indexer

```rust
#[derive(Job, Serialize, Deserialize)]
#[job(name = "indexer", resumable = true)]
pub struct IndexerJob {
    location_id: Uuid,
    root_path: SdPath,
    mode: IndexMode,
    #[serde(skip)]
    walked_paths: HashSet<PathBuf>,
}

#[job_handler]
impl IndexerJob {
    async fn run(&mut self, ctx: JobContext) -> JobResult {
        // Initialize from saved state or start fresh
        let mut state = self.load_state(&ctx).await?
            .unwrap_or_else(|| IndexerState::new(&self.root_path));
        
        // Report initial progress
        ctx.progress(Progress::indeterminate("Scanning directories..."));
        
        // Walk directories with resumable state machine
        while let Some(entry) = state.next_entry(&ctx).await? {
            ctx.check_interrupt().await?;
            
            match entry {
                WalkEntry::Dir(path) => {
                    // Spawn sub-job for deep directories
                    if should_spawn_subjob(&path) {
                        ctx.spawn_child(IndexerJob {
                            location_id: self.location_id,
                            root_path: path.to_sdpath()?,
                            mode: self.mode.clone(),
                            walked_paths: Default::default(),
                        }).await?;
                    }
                    
                    ctx.progress(Progress::structured(IndexerProgress {
                        phase: IndexPhase::Walking,
                        current_path: path.to_string_lossy().to_string(),
                        items_found: state.items_found,
                        dirs_remaining: state.dirs_remaining(),
                    }));
                }
                
                WalkEntry::File(metadata) => {
                    state.found_items.push(metadata);
                    
                    // Batch processing
                    if state.found_items.len() >= 1000 {
                        self.process_batch(&mut state, &ctx).await?;
                        ctx.checkpoint_with_state(&state).await?;
                    }
                }
            }
        }
        
        // Process remaining items
        if !state.found_items.is_empty() {
            self.process_batch(&mut state, &ctx).await?;
        }
        
        Ok(JobOutput::Indexed {
            total_files: state.total_files,
            total_dirs: state.total_dirs,
            total_bytes: state.total_bytes,
        })
    }
    
    async fn process_batch(&self, state: &mut IndexerState, ctx: &JobContext) -> Result<()> {
        let batch = std::mem::take(&mut state.found_items);
        
        // Save to database
        ctx.library_db().transaction(|tx| async {
            for item in batch {
                create_entry(&item, tx).await?;
            }
            Ok(())
        }).await?;
        
        state.processed_count += batch.len();
        ctx.progress(Progress::percentage(
            state.processed_count as f32 / state.estimated_total as f32
        ));
        
        Ok(())
    }
}

// State management for complex resumable operations
#[derive(Serialize, Deserialize)]
struct IndexerState {
    walk_state: WalkerState,
    found_items: Vec<FileMetadata>,
    processed_count: usize,
    total_files: u64,
    total_dirs: u64,
    total_bytes: u64,
    estimated_total: usize,
}
```

## Progress Reporting

### Type-Safe Progress API

```rust
pub enum Progress {
    /// Simple count-based progress
    Count { current: usize, total: usize },
    
    /// Percentage-based progress
    Percentage(f32),
    
    /// Indeterminate progress with message
    Indeterminate(String),
    
    /// Structured progress for complex jobs
    Structured(Box<dyn ProgressData>),
}

// Jobs can define custom progress types
#[derive(Serialize, Deserialize, ProgressData)]
pub struct IndexerProgress {
    pub phase: IndexPhase,
    pub current_path: String,
    pub items_found: usize,
    pub dirs_remaining: usize,
}

#[derive(Serialize, Deserialize)]
pub enum IndexPhase {
    Walking,
    Processing,
    GeneratingThumbnails,
    ExtractingMetadata,
}
```

## Job Context API

The `JobContext` provides all the capabilities a job needs:

```rust
pub struct JobContext {
    // Core functionality
    pub fn id(&self) -> JobId;
    pub fn library(&self) -> &Library;
    pub fn library_db(&self) -> &DatabaseConnection;
    
    // Progress reporting
    pub fn progress(&self, progress: Progress);
    pub fn add_warning(&self, warning: impl Into<String>);
    pub fn add_non_critical_error(&self, error: impl Into<JobError>);
    
    // Metrics
    pub fn metrics(&self) -> &JobMetrics;
    pub fn increment_bytes(&self, bytes: u64);
    
    // Control flow
    pub async fn check_interrupt(&self) -> Result<()>;
    pub async fn checkpoint(&self) -> Result<()>;
    pub async fn checkpoint_with_state<S: Serialize>(&self, state: &S) -> Result<()>;
    
    // Child jobs
    pub async fn spawn_child<J: Job>(&self, job: J) -> Result<JobHandle>;
    pub async fn wait_for_children(&self) -> Result<()>;
    
    // State management
    pub async fn load_state<S: DeserializeOwned>(&self) -> Result<Option<S>>;
    pub async fn save_state<S: Serialize>(&self, state: &S) -> Result<()>;
}
```

## Job Registration & Discovery

Using the `inventory` crate for zero-boilerplate registration:

```rust
// The #[derive(Job)] macro automatically generates this
inventory::submit! {
    JobRegistration::new::<FileCopyJob>()
}

// Job system discovers all jobs at runtime
pub fn discover_jobs() -> Vec<JobRegistration> {
    inventory::iter::<JobRegistration>()
        .cloned()
        .collect()
}
```

## Job Database Schema

Each library has its own `jobs.db`:

```sql
-- Active and queued jobs
CREATE TABLE jobs (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    state BLOB NOT NULL,  -- Serialized job state
    status TEXT NOT NULL, -- 'queued', 'running', 'paused', 'completed', 'failed'
    priority INTEGER DEFAULT 0,
    
    -- Progress tracking
    progress_type TEXT,
    progress_data BLOB,
    
    -- Relationships
    parent_job_id TEXT,
    
    -- Metrics
    started_at TIMESTAMP,
    completed_at TIMESTAMP,
    paused_at TIMESTAMP,
    
    -- Error tracking
    error_message TEXT,
    warnings BLOB,  -- JSON array
    non_critical_errors BLOB,  -- JSON array
    
    FOREIGN KEY (parent_job_id) REFERENCES jobs(id)
);

-- Completed job history (kept for 30 days)
CREATE TABLE job_history (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    status TEXT NOT NULL,
    started_at TIMESTAMP NOT NULL,
    completed_at TIMESTAMP NOT NULL,
    duration_ms INTEGER,
    output BLOB,  -- Serialized JobOutput
    metrics BLOB  -- Final metrics
);

-- Checkpoint data for resumable jobs
CREATE TABLE job_checkpoints (
    job_id TEXT PRIMARY KEY,
    checkpoint_data BLOB NOT NULL,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (job_id) REFERENCES jobs(id) ON DELETE CASCADE
);
```

## Integration with Task System

Jobs are executed as tasks:

```rust
impl<T: Job> Task<JobError> for JobTask<T> {
    fn id(&self) -> TaskId {
        self.job_id.into()
    }
    
    fn with_priority(&self) -> bool {
        self.priority > 0
    }
    
    async fn run(&mut self, interrupter: &Interrupter) -> Result<ExecStatus, JobError> {
        // Create job context with interrupter
        let ctx = JobContext::new(
            self.job_id,
            self.library.clone(),
            interrupter.clone(),
        );
        
        // Run the job
        match self.job.run(ctx).await {
            Ok(output) => {
                self.output = Some(output);
                Ok(ExecStatus::Done(()))
            }
            Err(JobError::Interrupted) => Ok(ExecStatus::Paused),
            Err(e) => Err(e),
        }
    }
}
```

## Job Lifecycle

### 1. Job Creation & Queueing

```rust
// Simple API for job dispatch
let job = FileCopyJob {
    sources: vec![source_path],
    destination: dest_path,
    options: Default::default(),
};

let handle = library.jobs().dispatch(job).await?;
```

### 2. Execution Flow

```
Queue → Schedule → Spawn Task → Execute → Checkpoint → Complete
                      ↓                        ↓
                   Interrupt              Save State
                      ↓                        ↓
                   Pause ←──────────────── Resume
```

### 3. Progress & Monitoring

```rust
// Subscribe to job updates
let mut updates = handle.subscribe();
while let Some(update) = updates.next().await {
    match update {
        JobUpdate::Progress(progress) => {
            // Update UI
        }
        JobUpdate::StateChanged(state) => {
            // Handle state changes
        }
        JobUpdate::Completed(output) => {
            // Job finished
        }
    }
}
```

## Advanced Features

### 1. Job Dependencies

```rust
#[derive(Job)]
#[job(name = "thumbnail_generation", depends_on = "indexer")]
pub struct ThumbnailJob {
    entry_ids: Vec<Uuid>,
}
```

### 2. Resource Constraints

```rust
#[derive(Job)]
#[job(
    name = "video_transcode",
    max_concurrent = 2,  // Only 2 transcodes at once
    requires_resources = ["gpu", "disk_space:10GB"]
)]
pub struct TranscodeJob {
    // ...
}
```

### 3. Scheduled Jobs

```rust
library.jobs()
    .schedule(CleanupJob::new())
    .every(Duration::hours(6))
    .starting_at(Local::now() + Duration::hours(1))
    .dispatch()
    .await?;
```

### 4. Job Composition

```rust
#[derive(Job)]
pub struct BackupJob {
    locations: Vec<LocationId>,
}

#[job_handler]
impl BackupJob {
    async fn run(&mut self, ctx: JobContext) -> JobResult {
        // Compose multiple sub-jobs
        for location in &self.locations {
            // Index first
            let indexer = ctx.spawn_child(IndexerJob::new(location)).await?;
            indexer.wait().await?;
            
            // Then generate thumbnails
            ctx.spawn_child(ThumbnailJob::for_location(location)).await?;
            
            // Finally upload
            ctx.spawn_child(UploadJob::for_location(location)).await?;
        }
        
        ctx.wait_for_children().await?;
        Ok(JobOutput::BackupComplete)
    }
}
```

## Implementation Plan

### Phase 1: Core Infrastructure
1. Create job-system crate with derive macro
2. Implement job registration with inventory
3. Create job database schema and migrations
4. Build JobContext API

### Phase 2: Basic Jobs
1. Port FileCopyJob as proof of concept
2. Implement progress reporting
3. Add job history tracking
4. Create job management UI

### Phase 3: Complex Jobs
1. Port IndexerJob with full state machine
2. Implement checkpoint/resume functionality
3. Add child job spawning
4. Performance optimization

### Phase 4: Advanced Features
1. Job scheduling system
2. Resource constraints
3. Job dependencies
4. Metrics and analytics

## Benefits Over Original System

1. **Minimal Boilerplate**: ~50 lines vs 500-1000 lines
2. **Auto-Registration**: No manual registry maintenance
3. **Type Safety**: Structured progress and outputs
4. **Flexibility**: Easy to add new job types
5. **Maintainability**: Clear separation of concerns
6. **Extensibility**: Can add jobs from any crate
7. **Developer Experience**: Intuitive API with good defaults

## Conclusion

This new job system design maintains all the power of the original while dramatically improving developer experience. By leveraging Rust's type system and building on the solid foundation of the task-system crate, we can provide a clean, extensible API that makes adding new jobs trivial while still supporting complex use cases like the indexer.