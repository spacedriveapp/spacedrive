---
id: JOB-003
title: Parallel Task Execution from Jobs
status: To Do
assignee: jamiepine
parent: JOB-000
priority: High
tags: [jobs, task-system, performance, parallelism]
whitepaper: Section 4.4.6
design_doc: workbench/JOB_PARALLEL_EXECUTION_SPEC.md
last_updated: 2025-10-14
---

## Description

Enable jobs to spawn parallel tasks on the task-system's multi-threaded worker pool, dramatically improving performance for I/O-bound operations like file copying, thumbnail generation, and media indexing.

**Key Design:** Jobs access `TaskDispatcher` via `JobContext` (similar to `ctx.library()`, `ctx.networking_service()`), allowing them to spawn child tasks that execute in parallel across available worker threads.

**Expected Impact:** 4-10x faster file operations depending on concurrency level.

## Problem

Current job system runs each job as a single task, processing work sequentially. For operations like copying 100 files, this leaves CPU cores idle and storage I/O underutilized.

**Current:** 100 files × 500ms = 50 seconds (sequential)
**Target:** 100 files / 10 workers = 5 seconds (10x faster)

## Solution Architecture

```
JobManager creates JobExecutor with TaskDispatcher
  ↓
JobContext exposes ctx.task_dispatcher()
  ↓
Job spawns parallel tasks via dispatcher
  ↓
Tasks execute on multi-threaded worker pool
```

**Why via Context, not Job storage?**

- Jobs are serialized to database (dispatcher is not serializable)
- JobExecutor already has task system access
- Consistent with existing patterns (library, volume_manager, etc.)
- No breaking changes to #[derive(Job)] macro

## Implementation Phases

### Phase 1: Core Integration (JOB-003a)

Enable jobs to access task dispatcher via context.

**Changes:**

1. Add `task_dispatcher` field to `JobExecutorState`
2. Update `JobExecutor::new()` to accept dispatcher parameter
3. Add `task_dispatcher` field to `JobContext`
4. Add `ctx.task_dispatcher()` accessor method
5. Update `JobManager::dispatch()` to pass dispatcher
6. Update `#[derive(Job)]` macro if needed

**Files:**

- core/src/infra/job/executor.rs
- core/src/infra/job/context.rs
- core/src/infra/job/manager.rs

**Acceptance Criteria:**

- [ ] Jobs can call `ctx.task_dispatcher()` and get valid dispatcher
- [ ] Integration test shows job spawning parallel tasks
- [ ] No breaking changes to existing jobs

### Phase 2: FileCopy Proof of Concept (JOB-003b)

Migrate FileCopyJob to use parallel execution.

**Changes:**

1. Create `CopyFileTask` implementing `Task<JobError>`
2. Update `FileCopyJob::run()` to use `dispatcher.dispatch_many()`
3. Implement progress aggregation from parallel tasks
4. Maintain resumability (track completed file indices)
5. Handle partial failures gracefully

**Files:**

- core/src/ops/files/copy/job.rs
- core/src/ops/files/copy/task.rs (new)

**Acceptance Criteria:**

- [ ] FileCopyJob spawns parallel copy tasks
- [ ] Performance improvement: 4-8x faster for 100+ files
- [ ] Job remains resumable after interruption
- [ ] Partial failures don't stop entire job
- [ ] Progress reporting works correctly

### Phase 3: Documentation & Patterns

Document the pattern for other developers.

**Deliverables:**

- [ ] Add parallel execution guide to job system docs
- [ ] Update job implementation template
- [ ] Code examples in developer documentation
- [ ] Integration test demonstrating pattern

### Phase 4: Expand to Other Operations (Future)

Apply pattern to other I/O-bound jobs:

- [ ] Thumbnail generation (highly parallel)
- [ ] Media metadata extraction
- [ ] File deletion (batch operations)
- [ ] Hash calculation (CPU-bound parallelism)

### Phase 5: Resource Management (Future)

Add centralized resource limits to prevent system overload.

**Features:**

- Global resource pools (I/O, CPU, Network, DB)
- `LimitedTaskDispatcher` wrapper with semaphores
- Priority-aware resource allocation
- Dynamic limit adjustment based on system load

**Note:** Resource limiting deferred to later phase after proving parallel execution concept.

## Technical Details

### Example: FileCopyJob with Parallel Tasks

```rust
#[async_trait]
impl JobHandler for FileCopyJob {
    async fn run(&mut self, ctx: JobContext<'_>) -> JobResult<Self::Output> {
        // Get dispatcher from context
        let dispatcher = ctx.task_dispatcher();

        // Create parallel copy tasks
        let tasks: Vec<_> = self.sources.paths.iter()
            .enumerate()
            .filter(|(idx, _)| !self.completed_indices.contains(idx))
            .map(|(idx, source)| CopyFileTask {
                id: TaskId::new_v4(),
                index: idx,
                source: source.clone(),
                destination: self.destination.clone(),
                options: self.options.clone(),
            })
            .collect();

        // Dispatch all tasks - task system handles distribution
        let handles = dispatcher.dispatch_many(tasks).await?;

        // Wait for completion and track progress
        for (completed, handle) in handles.into_iter().enumerate() {
            ctx.check_interrupt().await?;

            match handle.await {
                Ok(TaskStatus::Done(_)) => {
                    self.completed_indices.push(completed);
                    ctx.progress(/* ... */);
                }
                Ok(TaskStatus::Error(e)) => {
                    // Handle individual task failure
                }
                _ => {}
            }

            if (completed + 1) % 10 == 0 {
                ctx.checkpoint().await?;
            }
        }

        Ok(FileCopyOutput { /* ... */ })
    }
}
```

### Example: CopyFileTask

```rust
struct CopyFileTask {
    id: TaskId,
    index: usize,
    source: SdPath,
    destination: SdPath,
    options: CopyOptions,
}

#[async_trait]
impl Task<JobError> for CopyFileTask {
    fn id(&self) -> TaskId { self.id }

    async fn run(&mut self, interrupter: &Interrupter) -> Result<ExecStatus, JobError> {
        // Check interruption
        interrupter.try_check_interrupt()?;

        // Execute copy strategy
        let strategy = CopyStrategyRouter::select_strategy(/* ... */).await;
        let bytes_copied = strategy.execute_simple(/* ... */).await?;

        Ok(ExecStatus::Done(/* output */))
    }
}
```

## Benefits

1. **True Parallelism**: Tasks distributed across all CPU cores with work-stealing
2. **No Architecture Changes**: Leverages existing task-system infrastructure
3. **Backward Compatible**: Existing sequential jobs continue to work
4. **Simple Implementation**: No wrappers, adapters, or special traits needed
5. **Proven Pattern**: Based on Spacedrive v1's task-system design

## Performance Expectations

**File Copy (100 files, 1MB each, SSD):**

- Sequential: 100 files × 20ms = 2000ms
- Parallel (10 concurrent): 10 batches × 20ms = 200ms
- **10x faster!**

**Real-world (Mixed sizes, 10GB total):**

- Sequential: ~102s
- Parallel: ~12s
- **8.5x faster!**

## References

- Design spec: workbench/JOB_PARALLEL_EXECUTION_SPEC.md
- Original pattern: crates/task-system/tests/common/jobs.rs (SampleJob)
- Related: FILE-001 (File Copy Job), FILE-003 (Cloud File Operations)

## Notes

This replaces the over-engineered approach from `JOB_TASK_COMPOSITION_API.md`. Key insight: **Jobs are orchestrators, tasks are workers.** Jobs don't need special task types - they just need ability to spawn standard tasks.
