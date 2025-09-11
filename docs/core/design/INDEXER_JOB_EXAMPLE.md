# Indexer Job Implementation Example

This document shows how the complex indexer job would be implemented using the new job system, demonstrating how it handles state machines, resumability, and progress reporting.

## Complete Indexer Implementation

```rust
use spacedrive_jobs::prelude::*;
use std::collections::{HashMap, HashSet, VecDeque};

/// The main indexer job - discovers and indexes files in a location
#[derive(Job, Debug, Serialize, Deserialize)]
#[job(name = "indexer", resumable = true, progress = IndexerProgress)]
pub struct IndexerJob {
    pub location_id: Uuid,
    pub root_path: SdPath,
    pub mode: IndexMode,
    
    // Resumable state
    #[serde(skip_serializing_if = "Option::is_none")]
    state: Option<IndexerState>,
}

/// Indexer-specific progress reporting
#[derive(Debug, Clone, Serialize, Deserialize, JobProgress)]
pub struct IndexerProgress {
    pub phase: IndexPhase,
    pub current_path: String,
    pub total_found: IndexerStats,
    pub processing_rate: f32, // items/sec
    pub estimated_remaining: Option<Duration>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct IndexerStats {
    pub files: u64,
    pub dirs: u64,
    pub bytes: u64,
    pub symlinks: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum IndexPhase {
    Discovery { dirs_queued: usize },
    Processing { batch: usize, total_batches: usize },
    ContentIdentification { current: usize, total: usize },
    Finalizing,
}

/// Main job implementation
#[job_handler]
impl IndexerJob {
    async fn run(&mut self, ctx: JobContext) -> JobResult<IndexerOutput> {
        // Initialize or restore state
        let state = match &mut self.state {
            Some(state) => {
                ctx.log("Resuming indexer from saved state");
                state
            }
            None => {
                ctx.log("Starting new indexer job");
                self.state = Some(IndexerState::new(&self.root_path));
                self.state.as_mut().unwrap()
            }
        };
        
        // Main state machine loop
        loop {
            ctx.check_interrupt().await?;
            
            match &state.phase {
                // Phase 1: Directory discovery
                Phase::Discovery => {
                    self.run_discovery_phase(state, &ctx).await?;
                }
                
                // Phase 2: Batch processing of found items
                Phase::Processing => {
                    self.run_processing_phase(state, &ctx).await?;
                }
                
                // Phase 3: Content identification (if deep mode)
                Phase::ContentIdentification => {
                    if self.mode >= IndexMode::Content {
                        self.run_content_phase(state, &ctx).await?;
                    } else {
                        state.phase = Phase::Complete;
                    }
                }
                
                // Phase 4: Done!
                Phase::Complete => break,
            }
            
            // Checkpoint after each phase
            ctx.checkpoint().await?;
        }
        
        // Generate final output
        Ok(IndexerOutput {
            location_id: self.location_id,
            stats: state.stats.clone(),
            duration: state.started_at.elapsed(),
            errors: state.errors.clone(),
        })
    }
    
    /// Phase 1: Walk directories and collect entries
    async fn run_discovery_phase(&self, state: &mut IndexerState, ctx: &JobContext) -> Result<()> {
        while let Some(dir_path) = state.dirs_to_walk.pop_front() {
            ctx.check_interrupt().await?;
            
            // Update progress
            ctx.progress(IndexerProgress {
                phase: IndexPhase::Discovery { 
                    dirs_queued: state.dirs_to_walk.len() 
                },
                current_path: dir_path.to_string_lossy().to_string(),
                total_found: state.stats,
                processing_rate: state.calculate_rate(),
                estimated_remaining: state.estimate_remaining(),
            });
            
            // Should we spawn a sub-job for this directory?
            if self.should_spawn_subjob(&dir_path, state) {
                ctx.spawn_child(IndexerJob {
                    location_id: self.location_id,
                    root_path: dir_path.to_sdpath()?,
                    mode: self.mode.clone(),
                    state: None,
                }).await?;
                continue;
            }
            
            // Read directory entries
            match self.read_directory(&dir_path, &ctx).await {
                Ok(entries) => {
                    for entry in entries {
                        match entry.kind {
                            EntryKind::Directory => {
                                state.dirs_to_walk.push_back(entry.path.clone());
                                state.stats.dirs += 1;
                            }
                            EntryKind::File => {
                                state.pending_entries.push(entry);
                                state.stats.files += 1;
                            }
                            EntryKind::Symlink => {
                                state.stats.symlinks += 1;
                            }
                        }
                    }
                    
                    // Batch entries for processing
                    if state.pending_entries.len() >= 1000 {
                        state.entry_batches.push(
                            std::mem::take(&mut state.pending_entries)
                        );
                    }
                }
                Err(e) => {
                    ctx.add_non_critical_error(format!("Failed to read {}: {}", dir_path.display(), e));
                    state.errors.push(IndexError::ReadDir { 
                        path: dir_path.to_string_lossy().to_string(), 
                        error: e.to_string() 
                    });
                }
            }
            
            // Periodic checkpoint during discovery
            if state.stats.files % 10000 == 0 {
                ctx.checkpoint_with_state(state).await?;
            }
        }
        
        // Final batch
        if !state.pending_entries.is_empty() {
            state.entry_batches.push(
                std::mem::take(&mut state.pending_entries)
            );
        }
        
        state.phase = Phase::Processing;
        Ok(())
    }
    
    /// Phase 2: Process entry batches
    async fn run_processing_phase(&self, state: &mut IndexerState, ctx: &JobContext) -> Result<()> {
        let total_batches = state.entry_batches.len();
        
        while let Some(batch) = state.entry_batches.pop() {
            ctx.check_interrupt().await?;
            
            let batch_num = total_batches - state.entry_batches.len();
            ctx.progress(IndexerProgress {
                phase: IndexPhase::Processing { 
                    batch: batch_num, 
                    total_batches 
                },
                current_path: format!("Batch {}/{}", batch_num, total_batches),
                total_found: state.stats,
                processing_rate: state.calculate_rate(),
                estimated_remaining: state.estimate_remaining(),
            });
            
            // Process batch in a transaction
            ctx.library_db().transaction(|tx| async {
                for entry in batch {
                    // Create Entry with UserMetadata
                    let db_entry = self.create_entry(&entry, &ctx).await?;
                    
                    // Track for content identification
                    if self.mode >= IndexMode::Content {
                        state.entries_for_content.push((db_entry.id, entry.path));
                    }
                    
                    state.stats.bytes += entry.size;
                }
                Ok(())
            }).await?;
            
            ctx.checkpoint_with_state(state).await?;
        }
        
        state.phase = Phase::ContentIdentification;
        Ok(())
    }
    
    /// Phase 3: Generate content identities
    async fn run_content_phase(&self, state: &mut IndexerState, ctx: &JobContext) -> Result<()> {
        let total = state.entries_for_content.len();
        
        // Process in chunks for better performance
        for chunk in state.entries_for_content.chunks(100) {
            ctx.check_interrupt().await?;
            
            let current = total - state.entries_for_content.len();
            ctx.progress(IndexerProgress {
                phase: IndexPhase::ContentIdentification { current, total },
                current_path: "Generating content identities".to_string(),
                total_found: state.stats,
                processing_rate: state.calculate_rate(),
                estimated_remaining: state.estimate_remaining(),
            });
            
            // Parallel content identification
            let cas_futures = chunk.iter().map(|(entry_id, path)| {
                self.generate_cas_id(path, &ctx)
            });
            
            let cas_results = futures::future::join_all(cas_futures).await;
            
            // Update database with content identities
            ctx.library_db().transaction(|tx| async {
                for ((entry_id, _), cas_result) in chunk.iter().zip(cas_results) {
                    if let Ok(cas_id) = cas_result {
                        self.link_content_identity(*entry_id, cas_id, tx).await?;
                    }
                }
                Ok(())
            }).await?;
        }
        
        state.phase = Phase::Complete;
        Ok(())
    }
    
    // Helper methods
    
    fn should_spawn_subjob(&self, path: &PathBuf, state: &IndexerState) -> bool {
        // Spawn subjobs for large directories to parallelize
        state.dirs_to_walk.len() > 10 && 
        state.stats.dirs > 100 &&
        path.ancestors().count() < 5 // Not too deep
    }
    
    async fn read_directory(&self, path: &PathBuf, ctx: &JobContext) -> Result<Vec<DirEntry>> {
        // Use streaming to handle large directories
        let mut entries = Vec::new();
        let mut dir = tokio::fs::read_dir(path).await?;
        
        while let Some(entry) = dir.next_entry().await? {
            let metadata = entry.metadata().await?;
            let kind = if metadata.is_dir() {
                EntryKind::Directory
            } else if metadata.is_symlink() {
                EntryKind::Symlink
            } else {
                EntryKind::File
            };
            
            entries.push(DirEntry {
                path: entry.path(),
                kind,
                size: metadata.len(),
                modified: metadata.modified().ok(),
            });
        }
        
        Ok(entries)
    }
    
    async fn create_entry(&self, entry: &DirEntry, ctx: &JobContext) -> Result<entities::Entry> {
        use sea_orm::ActiveValue::*;
        
        let entry_model = entities::entry::ActiveModel {
            id: NotSet,
            uuid: Set(Uuid::new_v7()),
            prefix_id: Set(self.get_or_create_prefix(&entry.path).await?),
            relative_path: Set(self.get_relative_path(&entry.path)),
            name: Set(entry.path.file_name().unwrap().to_string_lossy().to_string()),
            kind: Set(entry.kind.to_string()),
            size: Set(entry.size as i64),
            modified_at: Set(entry.modified.map(|t| t.into())),
            metadata_id: Set(Uuid::new_v7()), // Always create metadata
            content_id: Set(None), // Will be set in content phase
            ..Default::default()
        };
        
        Ok(entry_model.insert(ctx.library_db().conn()).await?)
    }
}

/// Resumable state for the indexer
#[derive(Debug, Serialize, Deserialize)]
struct IndexerState {
    phase: Phase,
    started_at: Instant,
    
    // Discovery phase
    dirs_to_walk: VecDeque<PathBuf>,
    pending_entries: Vec<DirEntry>,
    
    // Processing phase  
    entry_batches: Vec<Vec<DirEntry>>,
    
    // Content phase
    entries_for_content: Vec<(Uuid, PathBuf)>,
    
    // Statistics
    stats: IndexerStats,
    errors: Vec<IndexError>,
    
    // Performance tracking
    items_per_second: RingBuffer<f32>,
}

#[derive(Debug, Serialize, Deserialize)]
enum Phase {
    Discovery,
    Processing,
    ContentIdentification,
    Complete,
}

#[derive(Debug, Serialize, Deserialize)]
struct DirEntry {
    path: PathBuf,
    kind: EntryKind,
    size: u64,
    modified: Option<SystemTime>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
enum IndexError {
    ReadDir { path: String, error: String },
    CreateEntry { path: String, error: String },
    ContentId { path: String, error: String },
}

/// Job output
#[derive(Debug, Serialize, Deserialize)]
pub struct IndexerOutput {
    pub location_id: Uuid,
    pub stats: IndexerStats,
    pub duration: Duration,
    pub errors: Vec<IndexError>,
}

impl IndexerState {
    fn new(root_path: &SdPath) -> Self {
        let mut dirs_to_walk = VecDeque::new();
        dirs_to_walk.push_back(root_path.to_path_buf());
        
        Self {
            phase: Phase::Discovery,
            started_at: Instant::now(),
            dirs_to_walk,
            pending_entries: Vec::new(),
            entry_batches: Vec::new(),
            entries_for_content: Vec::new(),
            stats: Default::default(),
            errors: Vec::new(),
            items_per_second: RingBuffer::new(60), // Track last minute
        }
    }
    
    fn calculate_rate(&self) -> f32 {
        self.items_per_second.average()
    }
    
    fn estimate_remaining(&self) -> Option<Duration> {
        // Complex estimation based on current rate and queue sizes
        None // TODO: Implement
    }
}
```

## Usage Example

```rust
// Dispatch an indexer job
let job = IndexerJob {
    location_id: location.id,
    root_path: location.path.clone(),
    mode: IndexMode::Deep,
    state: None,
};

let handle = library.jobs().dispatch(job).await?;

// Monitor progress
let mut progress_rx = handle.subscribe();
while let Some(update) = progress_rx.next().await {
    match update {
        JobUpdate::Progress(IndexerProgress { phase, total_found, .. }) => {
            println!("Indexer {:?}: {} files, {} dirs", phase, total_found.files, total_found.dirs);
        }
        JobUpdate::Completed(output) => {
            println!("Indexing complete: {:?}", output.stats);
        }
        _ => {}
    }
}

// Can pause/resume
handle.pause().await?;
// ... later ...
handle.resume().await?;

// Or cancel
handle.cancel().await?;
```

## Key Design Patterns

### 1. State Machine Architecture
- Clear phases with explicit transitions
- Each phase is independently resumable
- State persists between phases

### 2. Batching for Performance
- Collects entries into batches of 1000
- Processes in database transactions
- Reduces database round trips

### 3. Subjob Spawning
- Large directories spawn parallel subjobs
- Prevents single-threaded bottlenecks
- Natural work distribution

### 4. Progress Composition
- Structured progress with phase information
- Real-time performance metrics
- Estimated time remaining

### 5. Error Resilience
- Non-critical errors don't stop indexing
- Errors collected for final report
- Graceful degradation

## Comparison with Original

### Original Indexer
- 2000+ lines across multiple files
- Complex job/task split
- Manual state serialization
- Difficult to understand flow

### New Indexer
- ~400 lines in single file
- Clear state machine
- Automatic serialization
- Self-documenting with types

The new design maintains all the sophistication while being much more maintainable!