//! # Change Detection
//!
//! Tracks filesystem changes through two complementary subsystems: batch
//! detection during indexer jobs (`detector`) and real-time handling of watcher
//! events (`handler`). Both produce the same `Change` type and share inode-based
//! move detection, so a file moved while the indexer is running behaves
//! identically to one moved while the watcher is active.
//!
//! Changes route to either `DatabaseAdapter` (database writes for
//! managed locations) or `MemoryAdapter` (in-memory updates for browsing
//! sessions). This split keeps browsed directories responsive without
//! polluting the database with temporary entries.

pub mod detector;
pub mod handler;
pub mod persistent;
pub mod types;

pub use detector::ChangeDetector;
pub use handler::{
	apply_batch, build_dir_entry, handle_create, handle_modify, handle_remove, handle_rename,
	path_exists_safe, should_filter_path, ChangeHandler,
};
pub use persistent::{DatabaseAdapter, DatabaseAdapterForJob};
pub use types::{Change, ChangeConfig, ChangeMetadata, ChangeType, EntryRef};
