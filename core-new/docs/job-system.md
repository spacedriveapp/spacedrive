# Job System

The job system is one of the most significant improvements in Core v2, reducing the boilerplate required to implement new jobs from 500+ lines to approximately 50 lines while adding better type safety and persistence.

## Overview

The job system provides:
- **Minimal boilerplate** for job implementation
- **Automatic serialization** using MessagePack
- **Database persistence** with resume capabilities  
- **Type-safe progress reporting**
- **Graceful error handling** and recovery
- **Checkpointing** for long-running operations

## Architecture

```
JobManager ─── manages ──→ JobDatabase ─── stores ──→ JobRecord
     │                           │                         │
     └── executes ──→ Job ── implements ──→ JobHandler ─────┘
                      │
                      └── reports ──→ Progress ──→ EventBus
```

### Core Components

**JobManager**
- Orchestrates job execution and persistence
- Manages job lifecycle (queue, run, pause, cancel)
- Handles database operations and recovery

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

## Creating a Job

### 1. Define the Job Struct

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct FileCopyJob {
    pub sources: Vec<SdPath>,
    pub destination: SdPath,
    pub copied_count: u64,
    pub total_bytes: u64,
    pub current_file: Option<String>,
}
```

### 2. Implement the Job Trait

```rust
use sd_core_new::infrastructure::jobs::traits::Job;

impl Job for FileCopyJob {
    const NAME: &'static str = "file_copy";
    const RESUMABLE: bool = true;
    const DESCRIPTION: Option<&'static str> = Some("Copy files between locations");
}
```

### 3. Implement the JobHandler Trait

```rust
use sd_core_new::infrastructure::jobs::{
    traits::JobHandler,
    context::JobContext,
    error::{JobError, JobResult},
    output::JobOutput,
    progress::Progress,
};

#[async_trait::async_trait]
impl JobHandler for FileCopyJob {
    type Output = CopyOutput;

    async fn run(&mut self, ctx: JobContext<'_>) -> JobResult<Self::Output> {
        ctx.log(format!("Starting copy of {} files", self.sources.len()));

        for (index, source) in self.sources.iter().enumerate() {
            // Check for interruption
            ctx.check_interrupt().await?;
            
            // Update current file
            self.current_file = Some(source.path().display().to_string());
            
            // Report progress
            ctx.progress(Progress::percentage(
                index as f64 / self.sources.len() as f64
            ));
            
            // Perform the copy operation
            match self.copy_file(source, &self.destination).await {
                Ok(bytes_copied) => {
                    self.copied_count += 1;
                    self.total_bytes += bytes_copied;
                    ctx.log(format!("Copied: {}", source.path().display()));
                }
                Err(e) => {
                    ctx.log(format!("Failed to copy {}: {}", source.path().display(), e));
                    return Err(JobError::from(e));
                }
            }
            
            // Checkpoint every 10 files
            if index % 10 == 9 {
                ctx.checkpoint().await?;
            }
        }

        ctx.log("Copy operation completed successfully");
        
        Ok(CopyOutput {
            copied_count: self.copied_count,
            total_bytes: self.total_bytes,
        })
    }
}
```

### 4. Define Output Type

```rust
#[derive(Debug, Serialize, Deserialize)]
pub struct CopyOutput {
    pub copied_count: u64,
    pub total_bytes: u64,
}

impl From<CopyOutput> for JobOutput {
    fn from(output: CopyOutput) -> Self {
        JobOutput::FileCopy {
            copied_count: output.copied_count,
            total_bytes: output.total_bytes,
        }
    }
}
```

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
use sd_core_new::infrastructure::jobs::manager::JobManager;

// Initialize job manager
let job_manager = JobManager::new(data_dir).await?;

// Create a job
let copy_job = FileCopyJob::new(sources, destination);

// Queue the job (future: when integrated with Core)
let job_id = job_manager.queue_job(copy_job).await?;

// Check job status
let status = job_manager.get_job_status(job_id).await?;
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