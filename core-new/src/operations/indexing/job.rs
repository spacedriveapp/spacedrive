//! Main indexer job implementation

use crate::{
    infrastructure::jobs::prelude::*,
    shared::types::SdPath,
};
use serde::{Deserialize, Serialize};
use std::time::Duration;
use uuid::Uuid;

use super::{
    state::{IndexerState, IndexerProgress, IndexerStats, IndexError, Phase},
    metrics::{IndexerMetrics, PhaseTimer},
    phases,
};

/// Indexing mode determines the depth of indexing
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum IndexMode {
    /// Just filesystem metadata (fastest)
    Shallow,
    /// Generate content identities (moderate)
    Content,
    /// Full indexing with thumbnails and text extraction (slowest)
    Deep,
}

/// Indexer job - discovers and indexes files in a location
#[derive(Debug, Serialize, Deserialize)]
pub struct IndexerJob {
    pub location_id: Uuid,
    pub root_path: SdPath,
    pub mode: IndexMode,
    
    // Resumable state
    #[serde(skip_serializing_if = "Option::is_none")]
    state: Option<IndexerState>,
    
    // Performance tracking
    #[serde(skip)]
    timer: Option<PhaseTimer>,
    #[serde(skip)]
    db_operations: (u64, u64), // (reads, writes)
    #[serde(skip)]
    batch_info: (u64, usize), // (count, total_size)
}

impl Job for IndexerJob {
    const NAME: &'static str = "indexer";
    const RESUMABLE: bool = true;
    const DESCRIPTION: Option<&'static str> = Some("Index files in a location");
}

impl JobProgress for IndexerProgress {}

#[async_trait::async_trait]
impl JobHandler for IndexerJob {
    type Output = IndexerOutput;
    
    async fn run(&mut self, ctx: JobContext<'_>) -> JobResult<Self::Output> {
        // Initialize timer
        if self.timer.is_none() {
            self.timer = Some(PhaseTimer::new());
        }
        
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
        
        // Get local path for operations
        let root_path = self.root_path.as_local_path()
            .ok_or_else(|| JobError::execution("Location root path is not local".to_string()))?;
        
        // Main state machine loop
        loop {
            ctx.check_interrupt().await?;
            
            match state.phase.clone() {
                Phase::Discovery => {
                    phases::run_discovery_phase(state, &ctx, root_path).await?;
                    
                    // Track batch info
                    self.batch_info.0 = state.entry_batches.len() as u64;
                    self.batch_info.1 = state.entry_batches.iter().map(|b| b.len()).sum();
                    
                    // Start processing timer
                    if let Some(timer) = &mut self.timer {
                        timer.start_processing();
                    }
                }
                
                Phase::Processing => {
                    phases::run_processing_phase(
                        self.location_id,
                        state,
                        &ctx,
                        self.mode,
                        root_path,
                    ).await?;
                    
                    // Update DB operation counts
                    self.db_operations.1 += state.entry_batches.len() as u64 * 100; // Estimate
                }
                
                Phase::Aggregation => {
                    phases::run_aggregation_phase(
                        self.location_id,
                        state,
                        &ctx,
                    ).await?;
                    
                    // Start content timer
                    if let Some(timer) = &mut self.timer {
                        timer.start_content();
                    }
                }
                
                Phase::ContentIdentification => {
                    if self.mode >= IndexMode::Content {
                        phases::run_content_phase(state, &ctx).await?;
                        self.db_operations.1 += state.entries_for_content.len() as u64;
                    } else {
                        ctx.log("Skipping content identification phase (mode=Shallow)");
                        state.phase = Phase::Complete;
                    }
                }
                
                Phase::Complete => break,
            }
            
            // Checkpoint after each phase
            ctx.checkpoint().await?;
        }
        
        // Calculate final metrics
        let metrics = if let Some(timer) = &self.timer {
            IndexerMetrics::calculate(
                &state.stats,
                timer,
                self.db_operations,
                self.batch_info,
            )
        } else {
            IndexerMetrics::default()
        };
        
        // Log summary
        ctx.log(&metrics.format_summary());
        
        // Generate final output
        Ok(IndexerOutput {
            location_id: self.location_id,
            stats: state.stats,
            duration: state.started_at.elapsed(),
            errors: state.errors.clone(),
            metrics: Some(metrics),
        })
    }
    
    async fn on_resume(&mut self, ctx: &JobContext<'_>) -> JobResult {
        // State is already loaded from serialization
        if let Some(state) = &self.state {
            ctx.log(format!("Resuming indexer in {:?} phase", state.phase));
            ctx.log(format!("Progress: {} files, {} dirs, {} errors so far", 
                state.stats.files, state.stats.dirs, state.stats.errors));
            
            // Reinitialize timer for resumed job
            self.timer = Some(PhaseTimer::new());
        }
        Ok(())
    }
    
    async fn on_pause(&mut self, ctx: &JobContext<'_>) -> JobResult {
        ctx.log("Pausing indexer job - state will be preserved");
        Ok(())
    }
    
    async fn on_cancel(&mut self, ctx: &JobContext<'_>) -> JobResult {
        ctx.log("Cancelling indexer job");
        if let Some(state) = &self.state {
            ctx.log(format!("Final stats: {} files, {} dirs indexed before cancellation",
                state.stats.files, state.stats.dirs));
        }
        Ok(())
    }
}

impl IndexerJob {
    /// Create a new indexer job
    pub fn new(location_id: Uuid, root_path: SdPath, mode: IndexMode) -> Self {
        Self {
            location_id,
            root_path,
            mode,
            state: None,
            timer: None,
            db_operations: (0, 0),
            batch_info: (0, 0),
        }
    }
    
    /// Create a shallow indexer job (metadata only)
    pub fn shallow(location_id: Uuid, root_path: SdPath) -> Self {
        Self::new(location_id, root_path, IndexMode::Shallow)
    }
    
    /// Create a content indexer job (with CAS IDs)
    pub fn with_content(location_id: Uuid, root_path: SdPath) -> Self {
        Self::new(location_id, root_path, IndexMode::Content)
    }
    
    /// Create a deep indexer job (full processing)
    pub fn deep(location_id: Uuid, root_path: SdPath) -> Self {
        Self::new(location_id, root_path, IndexMode::Deep)
    }
}

/// Job output with comprehensive results
#[derive(Debug, Serialize, Deserialize)]
pub struct IndexerOutput {
    pub location_id: Uuid,
    pub stats: IndexerStats,
    pub duration: Duration,
    pub errors: Vec<IndexError>,
    pub metrics: Option<IndexerMetrics>,
}

impl From<IndexerOutput> for JobOutput {
    fn from(output: IndexerOutput) -> Self {
        JobOutput::Indexed {
            total_files: output.stats.files,
            total_dirs: output.stats.dirs,
            total_bytes: output.stats.bytes,
        }
    }
}

// TODO: Job registration needs to be fixed
// crate::register_job!(IndexerJob);