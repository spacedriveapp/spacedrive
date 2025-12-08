//! # Spacedrive's File Indexing System
//!
//! `core::ops::indexing` provides a multi-phase indexing pipeline that turns
//! raw filesystem paths into searchable database entries. The system handles
//! both persistent locations (managed directories) and ephemeral browsing sessions
//! (external drives, network shares), ensuring every file gets a stable UUID for
//! sync and user data attachment.
//!
//! ## Example
//! ```rust,no_run
//! use spacedrive_core::ops::indexing::{IndexerJob, IndexerJobConfig, IndexMode};
//! use spacedrive_core::domain::addressing::SdPath;
//! use uuid::Uuid;
//!
//! # async fn example(library: &spacedrive_core::library::Library, location_id: Uuid, path: SdPath) -> Result<(), Box<dyn std::error::Error>> {
//! let config = IndexerJobConfig::new(location_id, path, IndexMode::Content);
//! let job = IndexerJob::new(config);
//! library.jobs().dispatch(job).await?;
//! # Ok(())
//! # }
//! ```

pub mod action;
pub mod change_detection;
pub mod ctx;
pub mod entry;
pub mod ephemeral;
pub mod handler;
pub mod hierarchy;
pub mod input;
pub mod job;
pub mod metrics;
pub mod path_resolver;
pub mod persistence;
pub mod phases;
pub mod processor;
pub mod progress;
pub mod responder;
pub mod rules;
pub mod state;
pub mod verify;

pub use action::IndexingAction;
pub use ctx::{IndexingCtx, ResponderCtx};
pub use entry::{EntryMetadata, EntryProcessor};
pub use ephemeral::EphemeralIndexCache;
pub use handler::{
	apply_batch as apply_change_batch, ChangeConfig, ChangeHandler, ChangeType, EntryRef,
	EphemeralChangeHandler, PersistentChangeHandler,
};
pub use hierarchy::HierarchyQuery;
pub use input::IndexInput;
pub use job::{
	EphemeralIndex, EphemeralIndexStats, IndexMode, IndexPersistence, IndexScope, IndexerJob,
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
pub use verify::{IndexVerifyAction, IndexVerifyInput, IndexVerifyOutput, IntegrityReport};

#[cfg(test)]
mod tests;
