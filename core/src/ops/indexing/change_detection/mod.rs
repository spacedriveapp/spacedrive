//! # Change Detection
//!
//! Watcher events become `Change`s, which carry inode-based move detection so a
//! rename is a move rather than a delete followed by a create.
//!
//! Changes route to `ArenaWriter`, which writes the volume index, its snapshot
//! and its source store from one change record.

pub mod handler;
pub mod types;

pub use handler::{
	apply_batch, build_dir_entry, handle_create, handle_modify, handle_remove, handle_rename,
	path_exists_safe, should_filter_path, ChangeHandler,
};
pub use types::{Change, ChangeConfig, ChangeMetadata, ChangeType, EntryRef};
