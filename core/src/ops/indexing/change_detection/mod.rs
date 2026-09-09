//! # Change Detection
//!
//! Tracks filesystem changes through two complementary subsystems: batch
//! detection during indexer jobs (`detector`) and real-time handling of watcher
//! events (`handler`). Both produce the same `Change` type and share inode-based
//! move detection, so a file moved while the indexer is running behaves
//! identically to one moved while the watcher is active.
//!
//! Changes route to `ArenaWriter`, which writes the volume index, its snapshot
//! and its source store from one change record.

pub mod detector;
pub mod handler;
pub mod types;

pub use detector::ChangeDetector;
pub use handler::{
	apply_batch, build_dir_entry, handle_create, handle_modify, handle_remove, handle_rename,
	path_exists_safe, should_filter_path, ChangeHandler,
};
pub use types::{Change, ChangeConfig, ChangeMetadata, ChangeType, EntryRef};
