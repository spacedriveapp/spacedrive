# PhaseProcessor Pattern Design Specification

## Overview

A universal phase processor pattern that can work with all Spacedrive jobs, providing:
1. Unified phase state management
2. Resumable phase execution
3. Progress tracking
4. Error handling
5. Checkpointing
6. Batch support (optional)

---

## Core Concepts

### Phase Definition

```rust
pub trait Phase: Send + Sync + 'static {
    /// Phase identifier
    fn name(&self) -> &'static str;
    
    /// Execute phase logic
    async fn execute(&mut self, ctx: &PhaseExecutionContext<'_>) -> PhaseResult<PhaseOutput>;
    
    /// Optional: cleanup on success
    async fn on_success(&mut self, _ctx: &PhaseExecutionContext<'_>) -> PhaseResult {
        Ok(())
    }
    
    /// Optional: rollback on failure
    async fn on_failure(&mut self, _ctx: &PhaseExecutionContext<'_>) -> PhaseResult {
        Ok(())
    }
}
```

### Phase Output

```rust
pub enum PhaseOutput {
    /// Continue to next phase
    Continue,
    /// Skip to specific phase by name
    SkipTo(String),
    /// Job is complete
    Complete,
}
```

### Phase Result

```rust
pub type PhaseResult<T = ()> = Result<T, PhaseError>;

pub enum PhaseError {
    /// Fatal error - stops job
    Fatal(String),
    /// Non-fatal error - continues but recorded
    NonFatal(String),
    /// Pause job (checkpointing happens automatically)
    Paused,
    /// Cancel job
    Cancelled,
}
```

---

## PhaseProcessor Implementation

### State Manager

```rust
pub struct PhaseProcessor {
    phases: Vec<Box<dyn Phase>>,
    current_phase_index: usize,
    phase_history: Vec<PhaseExecution>,
    resumable: bool,
}

pub struct PhaseExecution {
    phase_name: String,
    started_at: Instant,
    completed_at: Option<Instant>,
    status: PhaseStatus,
    error: Option<String>,
}

pub enum PhaseStatus {
    Pending,
    Running,
    Completed,
    Skipped,
    Failed,
    Paused,
}
```

### Execution Context

```rust
pub struct PhaseExecutionContext<'a> {
    /// Parent job context
    job_ctx: &'a JobContext<'a>,
    /// Current phase name
    phase_name: &'a str,
    /// Accumulated phase-level errors
    errors: Arc<Mutex<Vec<String>>>,
    /// Progress reporter
    progress_reporter: Box<dyn ProgressReporter + Send + Sync>,
}

impl<'a> PhaseExecutionContext<'a> {
    // Delegates to JobContext
    pub fn library(&self) -> &Library { ... }
    pub fn library_db(&self) -> &DatabaseConnection { ... }
    pub async fn check_interrupt(&self) -> JobResult { ... }
    pub fn log(&self, msg: impl Into<String>) { ... }
    pub fn log_error(&self, msg: impl Into<String>) { ... }
    pub async fn checkpoint(&self) -> JobResult { ... }
    
    // Phase-specific
    pub fn add_non_fatal_error(&self, error: String) { ... }
    pub fn get_errors(&self) -> Vec<String> { ... }
    pub fn report_progress(&self, progress: PhaseProgress) { ... }
    pub fn atomic_operation_complete(&self, completed: u64, total: u64) { ... }
}
```

### Progress Reporter

```rust
pub trait ProgressReporter: Send + Sync {
    fn report(&self, progress: PhaseProgress);
    fn atomic_operation(&self, completed: u64, total: u64);
}

pub struct PhaseProgress {
    pub phase_name: String,
    pub progress_percent: f32,
    pub message: String,
    pub atomic_unit_count: u64,
    pub atomic_unit_total: u64,
    pub estimated_remaining: Option<Duration>,
    pub errors_count: u64,
}
```

---

## Usage Patterns

### Pattern 1: Simple Linear Phases

```rust
struct MyJob { ... }

impl JobHandler for MyJob {
    async fn run(&mut self, ctx: JobContext<'_>) -> JobResult<Self::Output> {
        let processor = PhaseProcessor::new()
            .add_phase(DiscoveryPhase::new())
            .add_phase(ProcessingPhase::new())
            .add_phase(CompletionPhase::new());
        
        processor.run(&ctx).await?;
        // Gather output...
        Ok(output)
    }
}
```

### Pattern 2: Conditional Phases

```rust
let mut processor = PhaseProcessor::new()
    .add_phase(DiscoveryPhase::new());

if config.mode >= ContentMode {
    processor = processor.add_phase(ContentPhase::new());
}

processor = processor.add_phase(CompletionPhase::new());
processor.run(&ctx).await?;
```

### Pattern 3: Batch Processing

```rust
struct ProcessingPhase {
    items: Vec<Item>,
    batch_size: usize,
}

impl Phase for ProcessingPhase {
    async fn execute(&mut self, ctx: &PhaseExecutionContext<'_>) -> PhaseResult<PhaseOutput> {
        for (batch_idx, batch) in self.items.chunks(self.batch_size).enumerate() {
            ctx.check_interrupt().await?;
            
            for item in batch {
                self.process_item(item, ctx).await?;
                ctx.atomic_operation_complete(1, self.items.len() as u64);
            }
            
            // Checkpoint after each batch
            ctx.checkpoint().await?;
        }
        
        Ok(PhaseOutput::Continue)
    }
}
```

### Pattern 4: Mode-Driven Phases

```rust
struct IndexerPhaseProcessor {
    mode: IndexMode,
    phases: Vec<Box<dyn Phase>>,
}

impl IndexerPhaseProcessor {
    fn new(mode: IndexMode) -> Self {
        let mut phases: Vec<Box<dyn Phase>> = vec![
            Box::new(DiscoveryPhase::new()),
            Box::new(ProcessingPhase::new()),
        ];
        
        if mode >= Content {
            phases.push(Box::new(ContentPhase::new()));
        }
        
        if mode >= Deep {
            phases.push(Box::new(DeepAnalysisPhase::new()));
        }
        
        Self { mode, phases }
    }
}
```

---

## Applying to Existing Jobs

### Indexer Job

**Phases:**
1. `DiscoveryPhase` - Walk directories (conditional on scope)
2. `ProcessingPhase` - Batch process entries
3. `AggregationPhase` - Calculate directory sizes (skip if ephemeral)
4. `ContentPhase` - Generate content IDs (conditional on mode)
5. `CompletePhase` - Finalize

**State:**
```rust
pub struct IndexerPhaseState {
    pub dirs_to_walk: VecDeque<PathBuf>,
    pub pending_entries: Vec<DirEntry>,
    pub entry_batches: Vec<Vec<DirEntry>>,
    pub entries_for_content: Vec<(i32, PathBuf)>,
    pub entry_id_cache: HashMap<PathBuf, i32>,
    pub existing_entries: HashMap<PathBuf, (i32, Option<u64>, Option<SystemTime>)>,
    pub stats: IndexerStats,
    pub errors: Vec<IndexError>,
}
```

### FileCopy Job

**Phases:**
1. `InitPhase` - Validate inputs
2. `DatabaseQueryPhase` - Get size estimates
3. `PreparationPhase` - Calculate totals
4. `CopyingPhase` - Execute copies (one source at a time)
5. `CompletePhase` - Report results

**State:**
```rust
pub struct CopyPhaseState {
    pub completed_indices: Vec<usize>,
    pub current_index: usize,
    pub bytes_copied: u64,
    pub total_bytes: u64,
    pub failed_copies: Vec<CopyError>,
}
```

### Delete Job

**Phases:**
1. `ValidatePhase` - Check targets exist
2. `DeletePhase` - Execute deletions using strategy
3. `CompletePhase` - Report results

**State:**
```rust
pub struct DeletePhaseState {
    pub completed_deletions: Vec<usize>,
    pub deleted_count: usize,
    pub failed_deletions: Vec<DeleteError>,
}
```

### Duplicate Detection

**Phases:**
1. `CollectionPhase` - Walk and collect files
2. `GroupByPhase` - Hash files by size
3. `AnalysisPhase` - Mode-specific comparison
4. `CompletePhase` - Report duplicates

**State:**
```rust
pub struct DuplicatePhaseState {
    pub collected_files: Vec<FileInfo>,
    pub size_groups: HashMap<u64, Vec<FileInfo>>,
    pub duplicate_groups: Vec<DuplicateGroup>,
}
```

### Validation Job

**Phases:**
1. `CollectionPhase` - Walk and collect files
2. `ValidationPhase` - Validate each file (mode determines checks)
3. `CompletePhase` - Report issues

**State:**
```rust
pub struct ValidationPhaseState {
    pub files_to_validate: Vec<FileValidationInfo>,
    pub validated_count: usize,
    pub issues: Vec<ValidationIssue>,
}
```

### Thumbnail Job

**Phases:**
1. `DiscoveryPhase` - Find entries needing thumbnails
2. `ProcessingPhase` - Generate batches with variants
3. `CleanupPhase` - Remove orphans
4. `CompletePhase` - Report results

**State:**
```rust
pub struct ThumbnailPhaseState {
    pub pending_entries: Vec<ThumbnailEntry>,
    pub batches: Vec<Vec<ThumbnailEntry>>,
    pub current_batch_index: usize,
    pub generated_count: u64,
    pub skipped_count: u64,
    pub error_messages: Vec<String>,
}
```

---

## Advanced Features

### Atomic Operation Tracking

```rust
pub struct AtomicOperationContext {
    name: String,
    started_at: Instant,
    completed: Arc<AtomicU64>,
    total: u64,
}

impl AtomicOperationContext {
    pub fn increment(&self, count: u64) {
        self.completed.fetch_add(count, Ordering::Relaxed);
    }
    
    pub fn current_progress(&self) -> f32 {
        self.completed.load(Ordering::Relaxed) as f32 / self.total as f32
    }
}
```

### Nested Phases

```rust
pub trait Phase {
    async fn execute(&mut self, ctx: &PhaseExecutionContext<'_>) -> PhaseResult<PhaseOutput>;
}

pub struct NestedPhaseGroup {
    sub_phases: Vec<Box<dyn Phase>>,
    current_index: usize,
}

impl Phase for NestedPhaseGroup {
    async fn execute(&mut self, ctx: &PhaseExecutionContext<'_>) -> PhaseResult<PhaseOutput> {
        while self.current_index < self.sub_phases.len() {
            self.sub_phases[self.current_index].execute(ctx).await?;
            self.current_index += 1;
        }
        Ok(PhaseOutput::Continue)
    }
}
```

### Phase Composition

```rust
pub trait ComposablePhase: Phase {
    fn then(self, next: Box<dyn Phase>) -> PhaseChain {
        PhaseChain::new()
            .add_phase(Box::new(self))
            .add_phase(next)
    }
}

pub struct PhaseChain {
    phases: Vec<Box<dyn Phase>>,
}
```

---

## State Serialization Strategy

### Phase State Traits

```rust
pub trait PhaseState: Send + Sync + Serialize + DeserializeOwned {
    /// Serialize to bytes
    fn serialize(&self) -> JobResult<Vec<u8>> {
        rmp_serde::to_vec(self)
            .map_err(|e| JobError::serialization(e.to_string()))
    }
    
    /// Deserialize from bytes
    fn deserialize(data: &[u8]) -> JobResult<Self> {
        rmp_serde::from_slice(data)
            .map_err(|e| JobError::serialization(e.to_string()))
    }
}
```

### Full Job State

```rust
pub struct PhaseProcessorState {
    pub current_phase_index: usize,
    pub phase_states: HashMap<String, Vec<u8>>, // Serialized phase states
    pub phase_history: Vec<PhaseExecution>,
}

impl Serialize for PhaseProcessorState { ... }
impl DeserializeOwned for PhaseProcessorState { ... }
```

---

## Error Recovery Strategy

```rust
pub enum RecoveryStrategy {
    /// Restart entire phase
    RestartPhase,
    /// Retry from last checkpoint
    RetryFromCheckpoint,
    /// Skip to next phase (data loss possible)
    SkipPhase,
    /// Abort job
    Abort,
}

impl Phase {
    async fn execute_with_recovery(
        &mut self,
        ctx: &PhaseExecutionContext<'_>,
        recovery: RecoveryStrategy,
    ) -> PhaseResult<PhaseOutput> {
        match self.execute(ctx).await {
            Ok(output) => Ok(output),
            Err(e) => match recovery {
                RecoveryStrategy::RestartPhase => {
                    ctx.log_error("Restarting phase after error");
                    self.execute(ctx).await
                }
                RecoveryStrategy::SkipPhase => {
                    ctx.log_error("Skipping phase after error");
                    Ok(PhaseOutput::Continue)
                }
                _ => Err(e),
            }
        }
    }
}
```

---

## Summary: Key Advantages

1. **Universal Pattern**: Works for all existing jobs
2. **Clear Separation**: Each phase is independent and testable
3. **Resumability**: Phase state persists across sessions
4. **Flexible Composition**: Conditional phases, nested phases, etc.
5. **Progress Tracking**: Granular atomic operation tracking
6. **Error Isolation**: Phase-level vs job-level errors
7. **Concurrency**: Supports concurrent operations within phases
8. **Scalability**: Works from small to large job scopes
9. **DRY**: Shared infrastructure across all jobs
10. **Maintainability**: Clear phase boundaries reduce complexity

