//! Production-ready indexing system for Spacedrive
//! 
//! This module implements a sophisticated file indexing system with:
//! - Multi-phase processing (discovery, processing, content identification)
//! - Full resumability with checkpoint support
//! - Incremental indexing with change detection
//! - Efficient batch processing
//! - Comprehensive error handling
//! - Performance monitoring and metrics

pub mod job;
pub mod state;
pub mod entry;
pub mod filters;
pub mod metrics;
pub mod phases;
pub mod progress;
pub mod change_detection;
pub mod persistence;

// Re-exports for convenience
pub use job::{
    IndexerJob, IndexMode, IndexScope, IndexPersistence, 
    IndexerJobConfig, EphemeralIndex, EphemeralContentIdentity,
    IndexerOutput
};
pub use state::{IndexerState, IndexerProgress, IndexPhase, IndexerStats};
pub use entry::{EntryProcessor, EntryMetadata};
pub use filters::should_skip_path;
pub use metrics::IndexerMetrics;
pub use persistence::{IndexPersistence as PersistenceTrait, PersistenceFactory};

// Rules system will be integrated here in the future
// pub mod rules;