//! # Spacedrive's File Indexing System
//!
//! `core::ops::indexing` owns the volume index: one walker maps every
//! attached drive into an in-memory arena, record identity is assigned by
//! each source's store so a file keeps its UUID across walks, and watchers
//! keep the map current. A source is a retained scope over that map; its
//! store holds the durable generation beside the assertions no ingest can
//! rebuild. There is one writer and one read map. The old persistent
//! location pipeline and its database entries are gone.

pub mod action;
pub mod arena;
pub mod change_detection;
pub mod collections;
pub mod content_identity;
pub mod descriptor;
pub mod handlers;
pub mod input;
pub mod job;
pub mod lens;
pub mod metadata;
pub mod metrics;
pub mod name;
pub mod nodes;
pub mod phases;
pub mod progress;
pub mod registry;
pub mod responder;
pub mod rules;
pub mod snapshot;
pub mod sources;
pub mod startup;
pub mod state;
pub mod store;
pub mod summary;
pub mod types;
pub mod verify_content;
pub mod volume_index;
pub mod writer;

pub use action::IndexingAction;
pub use arena::{Arena, ArenaStats};
pub use change_detection::{
	apply_batch as apply_change_batch, Change, ChangeConfig, ChangeHandler, ChangeType, EntryRef,
};
pub use handlers::FsEventHandler;
pub use input::IndexInput;
pub use job::{IndexScope, IndexerJob, IndexerJobConfig, IndexerOutput};
pub use metadata::{extract_metadata, get_inode, is_hidden_path, EntryMetadata};
pub use metrics::IndexerMetrics;
pub use name::NameCache;
pub use nodes::NodeArena;
pub use registry::NameRegistry;
pub use sources::{SourceConfig, SourceRecord, SourceRegistry, VolumeAnchor};
pub use store::{DuplicateCopy, SourceCounts, SourceStore};
pub use types::{
	EntryId, FileNode, FileType, MaybeEntryId, NameRef, NodeState, PackedMetadata, Rollup,
};
pub use volume_index::VolumeIndex;
pub use writer::{ArenaChange, ArenaWriter, Mutation, Notify, Seen};

pub use rules::{
	build_default_ruler, IndexerRule, IndexerRuler, RuleKind, RulePerKind, RuleToggles,
	RulerDecision,
};
pub use state::{IndexPhase, IndexerProgress, IndexerState, IndexerStats};

#[cfg(test)]
mod tests;
