<!--CREATED: 2025-06-18-->
# Spacedrive Job System v2

## Overview

The new job system provides a minimal-boilerplate framework for defining and executing background tasks in Spacedrive. Built on top of the battle-tested `task-system` crate, it offers powerful features like automatic persistence, progress tracking, and graceful interruption.

## Quick Start

### 1. Define a Job

```rust
use spacedrive_jobs::prelude::*;
use serde::{Serialize, Deserialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct MyJob {
    input_path: PathBuf,
    output_path: PathBuf,
}

impl Job for MyJob {
    const NAME: &'static str = "my_job";
    const RESUMABLE: bool = true;
}

#[async_trait]
impl JobHandler for MyJob {
    type Output = MyJobOutput;
    
    async fn run(&mut self, ctx: JobContext) -> JobResult<Self::Output> {
        // Your job logic here
        ctx.progress(Progress::indeterminate("Processing..."));
        
        // Check for interruption
        ctx.check_interrupt().await?;
        
        // Do work...
        let result = process_file(&self.input_path).await?;
        
        Ok(MyJobOutput { 
            items_processed: result.count 
        })
    }
}
```

### 2. Dispatch the Job

```rust
let job = MyJob {
    input_path: "/path/to/input".into(),
    output_path: "/path/to/output".into(),
};

let handle = library.jobs().dispatch(job).await?;
```

### 3. Monitor Progress

```rust
let mut updates = handle.subscribe();
while let Some(update) = updates.next().await {
    match update {
        JobUpdate::Progress(p) => println!("Progress: {}", p),
        JobUpdate::Completed(output) => println!("Done: {:?}", output),
        JobUpdate::Failed(e) => eprintln!("Failed: {}", e),
        _ => {}
    }
}
```

## Features

### Minimal Boilerplate
- Just implement two traits: `Job` and `JobHandler`
- ~50 lines for a complete job vs 500-1000 in the old system
- No manual registration required

### Automatic Persistence
- Jobs automatically save state at checkpoints
- Resume from exactly where they left off after crashes
- Per-library job database

### Rich Progress Tracking
- Count-based: "3/10 files"
- Percentage-based: "45.2%"
- Bytes-based: "1.5 GB / 3.2 GB"
- Custom structured progress for complex jobs

### Full Control
- Pause/resume running jobs
- Cancel with cleanup
- Priority execution
- Child job spawning

### Observability
- Real-time progress updates
- Detailed metrics (bytes, items, duration)
- Warning and non-critical error tracking
- Job history with configurable retention

## Architecture

```
┌─────────────────────────┐
│     Your Job Code       │  <- You write this (50 lines)
├─────────────────────────┤
│    Job System Layer     │  <- Handles persistence, progress, lifecycle
├─────────────────────────┤
│   Task System Layer     │  <- Provides execution, parallelism, interruption  
├─────────────────────────┤
│     Worker Pool         │  <- CPU-optimized thread pool
└─────────────────────────┘
```

## Advanced Examples

### Resumable Job with State

```rust
#[derive(Serialize, Deserialize)]
struct ProcessingJob {
    files: Vec<PathBuf>,
    #[serde(skip)]
    processed_indices: Vec<usize>,
}

impl JobHandler for ProcessingJob {
    async fn run(&mut self, ctx: JobContext) -> JobResult<Output> {
        // Load saved state if resuming
        if let Some(indices) = ctx.load_state::<Vec<usize>>().await? {
            self.processed_indices = indices;
        }
        
        for (i, file) in self.files.iter().enumerate() {
            if self.processed_indices.contains(&i) {
                continue; // Skip already processed
            }
            
            ctx.check_interrupt().await?;
            
            process_file(file).await?;
            self.processed_indices.push(i);
            
            // Save progress
            ctx.checkpoint_with_state(&self.processed_indices).await?;
        }
        
        Ok(Output::default())
    }
}
```

### Custom Progress Types

```rust
#[derive(Serialize, JobProgress)]
struct ConversionProgress {
    current_file: String,
    files_done: usize,
    total_files: usize,
    current_file_percent: f32,
}

impl JobHandler for VideoConverter {
    async fn run(&mut self, ctx: JobContext) -> JobResult<Output> {
        ctx.progress(Progress::structured(ConversionProgress {
            current_file: "video.mp4".into(),
            files_done: 1,
            total_files: 10,
            current_file_percent: 0.45,
        }));
        
        // Progress is automatically serialized and sent to subscribers
    }
}
```

### Job Composition

```rust
impl JobHandler for BatchProcessor {
    async fn run(&mut self, ctx: JobContext) -> JobResult<Output> {
        // Spawn child jobs
        for chunk in self.data.chunks(1000) {
            let child = ChunkProcessor { data: chunk.to_vec() };
            ctx.spawn_child(child).await?;
        }
        
        // Wait for all children to complete
        ctx.wait_for_children().await?;
        
        Ok(Output::default())
    }
}
```

## Comparison with Original System

| Feature | Old System | New System |
|---------|------------|------------|
| Lines to define a job | 500-1000 | ~50 |
| Registration | Manual in 3 places | Automatic |
| Can forget to register | Yes (runtime panic) | No |
| Type safety | Dynamic dispatch heavy | Fully typed |
| Progress reporting | String-based | Structured + typed |
| Extensibility | Core only | Any crate |
| Learning curve | Steep | Gentle |

## Implementation Status

- [x] Core job traits and types
- [x] Job manager and executor  
- [x] Database schema and persistence
- [x] Progress tracking
- [x] Task system integration
- [x] Basic job examples (copy, indexer)
- [ ] Derive macro (currently manual implementation)
- [ ] Job scheduling (cron-like)
- [ ] Resource constraints
- [ ] Job dependencies DAG

## Future Plans

1. **Derive Macro**: Automatic implementation of boilerplate
2. **Job Scheduling**: Run jobs on schedules or triggers  
3. **Resource Management**: CPU/memory/disk constraints
4. **Job Marketplace**: Share job definitions as plugins
5. **Distributed Execution**: Run jobs across devices

The new job system dramatically simplifies job creation while maintaining all the power needed for complex operations like indexing millions of files.