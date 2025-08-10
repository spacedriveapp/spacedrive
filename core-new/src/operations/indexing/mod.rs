//! Production-ready indexing system for Spacedrive
//!
//! This module implements a sophisticated file indexing system with:
//! - Multi-phase processing (discovery, processing, content identification)
//! - Full resumability with checkpoint support
//! - Incremental indexing with change detection
//! - Efficient batch processing
//! - Comprehensive error handling
//! - Performance monitoring and metrics

pub mod action;
pub mod job;
pub mod state;
pub mod entry;
pub mod filters;
pub mod rules;
pub mod metrics;
pub mod phases;
pub mod progress;
pub mod change_detection;
pub mod persistence;
pub mod hierarchy;
pub mod path_resolver;

// Re-exports for convenience
pub use job::{
    IndexerJob, IndexMode, IndexScope, IndexPersistence,
    IndexerJobConfig, EphemeralIndex, EphemeralContentIdentity,
    IndexerOutput
};
pub use state::{IndexerState, IndexerProgress, IndexPhase, IndexerStats};
pub use entry::{EntryProcessor, EntryMetadata};
pub use filters::should_skip_path;
pub use rules::{
    IndexerRuler,
    IndexerRule,
    RulePerKind,
    RuleKind,
    RulerDecision,
    RuleToggles,
    build_default_ruler,
};
pub use metrics::IndexerMetrics;
pub use persistence::{IndexPersistence as PersistenceTrait, PersistenceFactory};
pub use action::IndexingAction;
pub use hierarchy::HierarchyQuery;
pub use path_resolver::PathResolver;

// Rules system will be integrated here in the future
// pub mod rules;

#[cfg(test)]
mod tests;