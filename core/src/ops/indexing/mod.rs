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
pub mod change_detection;
pub mod ctx;
pub mod entry;
pub mod hierarchy;
pub mod input;
pub mod job;
pub mod metrics;
pub mod path_resolver;
pub mod persistence;
pub mod phases;
pub mod progress;
pub mod responder;
pub mod rules;
pub mod state;

// Re-exports for convenience
pub use action::IndexingAction;
pub use ctx::{IndexingCtx, ResponderCtx};
pub use entry::{EntryMetadata, EntryProcessor};
pub use hierarchy::HierarchyQuery;
pub use input::IndexInput;
pub use job::{
	EphemeralContentIdentity, EphemeralIndex, IndexMode, IndexPersistence, IndexScope, IndexerJob,
	IndexerJobConfig, IndexerOutput,
};
pub use metrics::IndexerMetrics;
pub use path_resolver::PathResolver;
pub use persistence::{IndexPersistence as PersistenceTrait, PersistenceFactory};
pub use rules::{
	build_default_ruler, IndexerRule, IndexerRuler, RuleKind, RulePerKind, RuleToggles,
	RulerDecision,
};
pub use state::{IndexPhase, IndexerProgress, IndexerState, IndexerStats};

// Rules system will be integrated here in the future
// pub mod rules;

#[cfg(test)]
mod tests;
