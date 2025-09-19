# Job System

The job system is one of the most significant improvements in Core v2, reducing the boilerplate required to implement new jobs from 500+ lines to approximately 50 lines while adding better type safety and persistence.

## Overview

The job system provides:
- **Zero-boilerplate registration** using derive macros
- **Automatic job discovery** at compile time
- **API-driven job dispatch** by name
- **Automatic serialization** using MessagePack
- **Database persistence** with resume capabilities
- **Type-safe progress reporting**
- **Graceful error handling** and recovery
- **Checkpointing** for long-running operations

## Architecture

```
#[derive(Job)] ── generates ──→ JobRegistration ── collected by ──→ JobRegistry
     │                               │                                    │
     └── implements ──→ JobHandler ───┼── stores ──→ create_fn ────────────┘
                           │          └── stores ──→ deserialize_fn
                           │
                           └── reports ──→ Progress ──→ EventBus

JobManager ─── uses ──→ JobRegistry ─── dispatches ──→ ErasedJob
     │                       │                            │
     └── manages ──→ JobDatabase ─── stores ──→ JobRecord ─┘
```

### Core Components

**JobRegistry** *(NEW)*
- Automatically discovers jobs using `inventory` crate
- Provides runtime job creation and dispatch
- Enables API-driven job execution by name
- Global registry accessible via `REGISTRY` static

**Derive Macro** *(NEW)*
- Zero-boilerplate job registration using `#[derive(Job)]`
- Generates `JobRegistration` and `ErasedJob` implementations
- Automatic compile-time registration via `inventory::submit!`

**JobManager**
- Orchestrates job execution and persistence
- Manages job lifecycle (dispatch, run, pause, cancel)
- Handles database operations and recovery
- Supports both direct dispatch and name-based dispatch

**Job Trait**
- Defines job metadata and behavior
- Minimal trait requiring only constants

**JobHandler Trait**
- Defines the actual job execution logic
- Handles progress reporting and checkpointing

**JobDatabase**
- SQLite storage for job state and history
- Automatic schema management
- Efficient querying and status tracking

## Automatic Job Registration

The job system uses a two-layer architecture for zero-boilerplate registration:

### Compile Time: Derive Macro
The `#[derive(Job)]` macro automatically generates:
- Job registration code using `inventory::submit!`
- `ErasedJob` trait implementation for type erasure
- Serialization/deserialization functions

### Runtime: Job Registry
The `JobRegistry` collects all registrations and provides:
- Job discovery by name (`job_names()`, `has_job()`)
- Dynamic job creation (`create_job()`, `deserialize_job()`)
- Schema introspection (`get_job_schema()`)

## Creating a Job

### 1. Define the Job Struct with Derive Macro

```rust
use serde::{Deserialize, Serialize};
use sd_core::infrastructure::jobs::prelude::*;

#[derive(Debug, Serialize, Deserialize, Job)]  // ← Job derive macro
pub struct FileCopyJob {
    pub sources: SdPathBatch,
    pub destination: SdPath,
    pub options: CopyOptions,

    // Internal state for resumption
    #[serde(skip)]
    completed_indices: Vec<usize>,
    #[serde(skip, default = "Instant::now")]
    started_at: Instant,
}
```

### 2. Implement the Job Trait

```rust
impl Job for FileCopyJob {
    const NAME: &'static str = "file_copy";
    const RESUMABLE: bool = true;
    const DESCRIPTION: Option<&'static str> = Some("Copy files between locations");
}

### 3. Implement the JobHandler Trait

```rust
#[async_trait::async_trait]
impl JobHandler for FileCopyJob {
    type Output = FileCopyOutput;

    async fn run(&mut self, ctx: JobContext<'_>) -> JobResult<Self::Output> {
        ctx.log(format!(
            "Starting copy operation on {} files",
            self.sources.paths.len()
        ));

        let total_files = self.sources.paths.len();
        let mut copied_count = 0;
        let mut total_bytes = 0u64;
        let mut failed_copies = Vec::new();

        // Group by device for efficient processing
        let by_device = self.sources.by_device();

        for (device_id, device_paths) in by_device {
            ctx.check_interrupt().await?;

            if device_id == self.destination.device_id {
                // Same device - efficient local copy
                self.process_same_device_copies(
                    device_paths.iter().collect(),
                    &ctx,
                    &mut copied_count,
                    &mut total_bytes,
                    &mut failed_copies,
                    total_files,
                ).await?;
            } else {
                // Cross-device copy
                self.process_cross_device_copies(
                    device_paths.iter().collect(),
                    &ctx,
                    &mut copied_count,
                    &mut total_bytes,
                    &mut failed_copies,
                    total_files,
                ).await?;
            }
        }

        ctx.log(format!(
            "Copy operation completed: {} copied, {} failed",
            copied_count,
            failed_copies.len()
        ));

        Ok(FileCopyOutput {
            copied_count,
            failed_count: failed_copies.len(),
            total_bytes,
            duration: self.started_at.elapsed(),
            failed_copies,
        })
    }
}
```

### 4. Define Output Type

```rust
#[derive(Debug, Serialize, Deserialize)]
pub struct FileCopyOutput {
    pub copied_count: usize,
    pub failed_count: usize,
    pub total_bytes: u64,
    pub duration: Duration,
    pub failed_copies: Vec<CopyError>,
}

impl From<FileCopyOutput> for JobOutput {
    fn from(output: FileCopyOutput) -> Self {
        JobOutput::FileCopy {
            copied_count: output.copied_count,
            total_bytes: output.total_bytes,
        }
    }
}
```

### 5. Add Constructor and Helper Methods

```rust
impl FileCopyJob {
    /// Create a new file copy job with sources and destination
    pub fn new(sources: SdPathBatch, destination: SdPath) -> Self {
        Self {
            sources,
            destination,
            options: Default::default(),
            completed_indices: Vec::new(),
            started_at: Instant::now(),
        }
    }

    /// Create an empty job (used by derive macro)
    pub fn empty() -> Self {
        Self {
            sources: SdPathBatch::new(Vec::new()),
            destination: SdPath::new(uuid::Uuid::new_v4(), PathBuf::new()),
            options: Default::default(),
            completed_indices: Vec::new(),
            started_at: Instant::now(),
        }
    }
}
```

That's it! The derive macro handles all the registration automatically.

## Job Registry

The `JobRegistry` provides runtime access to all registered jobs:

```rust
use sd_core::infrastructure::jobs::registry::REGISTRY;

// Discover all job types
let job_types = REGISTRY.job_names();
println!("Available jobs: {:?}", job_types);

// Get job schema for introspection
if let Some(schema) = REGISTRY.get_job_schema("file_copy") {
    println!("Schema: {:?}", schema);
}

// Create job from JSON (useful for APIs)
let job_data = serde_json::json!({
    "sources": ["/path/to/file1", "/path/to/file2"],
    "destination": "/path/to/dest"
});
let job = REGISTRY.create_job("file_copy", job_data)?;

// Deserialize job from binary data (for resumption)
let binary_data = rmp_serde::to_vec(&some_job)?;
let restored_job = REGISTRY.deserialize_job("file_copy", &binary_data)?;
```

### Registry Features

- **Automatic Discovery**: Uses `inventory` crate to collect all jobs at compile time
- **Type Safety**: Ensures only valid job types can be created
- **Schema Introspection**: Provides metadata about job parameters
- **Multiple Formats**: Supports both JSON (APIs) and MessagePack (persistence)

## Job Context

The `JobContext` provides essential capabilities during job execution:

```rust
impl<'a> JobContext<'a> {
    /// Log a message associated with this job
    pub fn log(&self, message: String) { /* ... */ }

    /// Report progress to subscribers
    pub fn progress(&self, progress: Progress) { /* ... */ }

    /// Check if the job should be interrupted
    pub async fn check_interrupt(&self) -> JobResult<()> { /* ... */ }

    /// Save current job state to database
    pub async fn checkpoint(&self) -> JobResult<()> { /* ... */ }

    /// Get job-specific data directory
    pub fn data_dir(&self) -> &Path { /* ... */ }
}
```

### Progress Reporting

Multiple progress types are supported:

```rust
pub enum Progress {
    /// Simple percentage (0.0 to 1.0)
    Percentage(f64),

    /// Structured progress with custom data
    Structured(serde_json::Value),

    /// Indeterminate progress
    Indeterminate,
}

// Usage examples:
ctx.progress(Progress::percentage(0.5));  // 50% complete

ctx.progress(Progress::structured(serde_json::json!({
    "current_file": "document.pdf",
    "files_processed": 150,
    "total_files": 500,
    "current_operation": "extracting_text"
})));
```

## Error Handling

Comprehensive error types for different failure scenarios:

```rust
#[derive(thiserror::Error, Debug)]
pub enum JobError {
    #[error("Job execution failed: {0}")]
    ExecutionFailed(String),

    #[error("Job was interrupted")]
    Interrupted,

    #[error("Database error: {0}")]
    Database(#[from] sea_orm::DbErr),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    Serialization(#[from] rmp_serde::encode::Error),
}

// Result type alias
pub type JobResult<T> = Result<T, JobError>;
```

## Job Outputs

Standardized output types for common operations:

```rust
#[derive(Debug, Serialize, Deserialize)]
pub enum JobOutput {
    /// File copy operation results
    FileCopy {
        copied_count: u64,
        total_bytes: u64,
    },

    /// Indexing operation results
    Indexed {
        total_files: u64,
        total_dirs: u64,
        total_bytes: u64,
    },

    /// Media processing results
    MediaProcessed {
        thumbnails_generated: u64,
        metadata_extracted: u64,
    },

    /// Custom operation results
    Custom(serde_json::Value),
}
```

## Job Management

### Creating and Running Jobs

```rust
use sd_core::infrastructure::jobs::manager::JobManager;

// Initialize job manager
let job_manager = JobManager::new(data_dir).await?;

// Method 1: Direct dispatch with job instance
let copy_job = FileCopyJob::new(sources, destination);
let handle = job_manager.dispatch(copy_job).await?;

// Method 2: API-driven dispatch by name
let job_params = serde_json::json!({
    "sources": ["/path/to/file1", "/path/to/file2"],
    "destination": "/path/to/dest"
});
let handle = job_manager.dispatch_by_name("file_copy", job_params).await?;

// Method 3: Dispatch with priority
let handle = job_manager.dispatch_with_priority(copy_job, JobPriority::HIGH).await?;

// Monitor job progress
let job_id = handle.id;
let status = handle.status();
let mut progress_updates = handle.progress_rx;

while let Ok(progress) = progress_updates.recv().await {
    println!("Progress: {:?}", progress);
}
```

### Job Discovery and Management

```rust
// List all available job types
let job_types = job_manager.list_job_types();
println!("Available jobs: {:?}", job_types);

// Get schema for a job type
if let Some(schema) = job_manager.get_job_schema("file_copy") {
    println!("Parameters: {:?}", schema);
}

// List running jobs
let running = job_manager.list_running_jobs().await;
println!("Currently running: {} jobs", running.len());

// List jobs by status
let completed = job_manager.list_jobs(Some(JobStatus::Completed)).await?;
let failed = job_manager.list_jobs(Some(JobStatus::Failed)).await?;

// Get detailed job information
if let Some(job_info) = job_manager.get_job_info(job_id).await? {
    println!("Job: {} - Status: {:?}", job_info.name, job_info.status);
}
```

### Job Lifecycle

```rust
pub enum JobStatus {
    Queued,      // Waiting to be executed
    Running,     // Currently executing
    Completed,   // Finished successfully
    Failed,      // Execution failed
    Cancelled,   // Cancelled by user
    Paused,      // Paused by user or system
}
```

### Database Schema

Jobs are persisted with the following schema:

```sql
CREATE TABLE jobs (
    id TEXT PRIMARY KEY,              -- UUID v4
    name TEXT NOT NULL,               -- Job type name
    data BLOB NOT NULL,               -- MessagePack serialized job
    status TEXT NOT NULL,             -- Current status
    progress REAL,                    -- Progress percentage (0.0-1.0)
    progress_data TEXT,               -- JSON progress details
    output TEXT,                      -- JSON output when completed
    error_message TEXT,               -- Error details if failed
    created_at TEXT NOT NULL,         -- ISO 8601 timestamp
    started_at TEXT,                  -- When execution began
    completed_at TEXT,                -- When execution finished
    last_checkpoint TEXT              -- Last checkpoint timestamp
);
```

## Example Jobs

### File Copy Job

Handles copying files with progress tracking and resume capabilities:

```rust
#[derive(Debug, Serialize, Deserialize)]
pub struct FileCopyJob {
    pub sources: Vec<SdPath>,
    pub destination: SdPath,
    pub copied_count: u64,
    pub total_bytes: u64,
    pub current_file: Option<String>,
}

impl FileCopyJob {
    pub fn new(sources: Vec<SdPath>, destination: SdPath) -> Self {
        Self {
            sources,
            destination,
            copied_count: 0,
            total_bytes: 0,
            current_file: None,
        }
    }
}
```

### Indexer Job

Scans directories and builds file metadata:

```rust
#[derive(Debug, Serialize, Deserialize)]
pub struct IndexerJob {
    pub library_id: Uuid,
    pub location: SdPath,
    pub index_mode: IndexMode,
    pub processed_count: u64,
    pub current_path: Option<PathBuf>,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum IndexMode {
    Metadata,    // File metadata only
    Content,     // Metadata + content hashes
    Deep,        // Full analysis + media data
}
```

## Job Resumption

The job system automatically resumes interrupted jobs on startup:

```rust
// Jobs are automatically discovered and resumed when JobManager starts
let job_manager = JobManager::new(data_dir).await?;

// Set library reference to enable resumption
job_manager.set_library(library).await;

// All interrupted jobs (Running/Paused status) are automatically resumed
// using the registry's deserialize_job() function
```

### How Resumption Works

1. **Startup Discovery**: JobManager scans database for interrupted jobs
2. **Registry Lookup**: Uses job name to find registration in registry
3. **Deserialization**: Calls `deserialize_fn` to recreate job instance
4. **State Restoration**: Job resumes from its last checkpointed state
5. **Execution**: Job continues from where it left off

### Resumption Requirements

- Job must implement `Serialize` + `Deserialize`
- Job must have `RESUMABLE = true` in `Job` trait
- Job state must be designed for partial completion
- Use `#[serde(skip)]` for non-persistent fields

```rust
#[derive(Debug, Serialize, Deserialize, Job)]
pub struct ResumableJob {
    // Persistent state (saved/restored)
    pub total_items: usize,
    pub processed_items: usize,

    // Transient state (recreated on resume)
    #[serde(skip)]
    pub current_connection: Option<Connection>,
    #[serde(skip, default = "Instant::now")]
    pub session_start: Instant,
}
```

## Testing

The job system includes comprehensive testing utilities:

```rust
#[tokio::test]
async fn test_job_serialization() {
    let job = FileCopyJob::new(sources, destination);

    // Test serialization round-trip
    let serialized = rmp_serde::to_vec(&job).unwrap();
    let deserialized: FileCopyJob = rmp_serde::from_slice(&serialized).unwrap();

    assert_eq!(job.sources.len(), deserialized.sources.len());
}

#[tokio::test]
async fn test_job_database_operations() {
    let job_manager = JobManager::new(temp_dir).await.unwrap();

    // Test job listing
    let jobs = job_manager.list_jobs(None).await.unwrap();
    assert!(jobs.is_empty());

    // Test status filtering
    let running = job_manager.list_jobs(Some(JobStatus::Running)).await.unwrap();
    assert!(running.is_empty());
}
```

## Integration with Core

Jobs integrate seamlessly with the Core system:

```rust
// Future integration pattern
impl Core {
    pub async fn copy_files(&self, sources: Vec<SdPath>, dest: SdPath) -> JobResult<JobId> {
        let job = FileCopyJob::new(sources, dest);
        self.jobs.queue(job).await
    }

    pub async fn index_location(&self, location_id: Uuid, mode: IndexMode) -> JobResult<JobId> {
        let location = self.libraries.get_location(location_id).await?;
        let job = IndexerJob::new(location.library_id, location.path.into(), mode);
        self.jobs.queue(job).await
    }
}
```

## Performance Considerations

### Serialization

- **MessagePack** provides compact binary serialization
- **50-80% smaller** than JSON for typical job data
- **Faster** serialization/deserialization than JSON

### Checkpointing

- **Configurable frequency** - balance between safety and performance
- **Incremental state saves** - only serialize changed data
- **Atomic writes** - prevent corruption during checkpoints

### Database Operations

- **SQLite WAL mode** - better concurrency for job operations
- **Prepared statements** - faster query execution
- **Connection pooling** - efficient resource usage

### Memory Management

- **Streaming processing** for large operations
- **Bounded queues** to prevent memory exhaustion
- **Resource cleanup** on job completion or failure

## Comparison with Original

| Feature | Original System | Core v2 System |
|---------|----------------|----------------|
| **Boilerplate** | 500-1000+ lines | ~50 lines |
| **Registration** | Manual macro registration | Automatic traits |
| **Serialization** | Custom implementation | Automatic with MessagePack |
| **Progress** | String-based messages | Type-safe structured data |
| **Persistence** | Complex state management | Automatic checkpointing |
| **Error Handling** | Inconsistent patterns | Standardized error types |
| **Testing** | Difficult to test | Comprehensive test utilities |
| **Performance** | Heavy trait objects | Efficient static dispatch |

The job system represents a significant leap forward in developer experience while maintaining all the power and flexibility needed for Spacedrive's file management operations.
