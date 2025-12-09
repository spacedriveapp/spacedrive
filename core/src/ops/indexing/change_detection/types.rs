//! Shared types for change detection and handling.
//!
//! This module defines the common vocabulary used by both:
//! - The detector (batch scanning during indexer jobs)
//! - The handler (real-time response to watcher events)

use crate::ops::indexing::state::EntryKind;
use std::path::PathBuf;
use std::time::SystemTime;
use uuid::Uuid;

/// A detected or reported filesystem change.
///
/// This enum represents changes that can come from either:
/// - The `ChangeDetector` during batch indexing scans
/// - The file watcher via `FsEvent` conversion
#[derive(Debug, Clone)]
pub enum Change {
	/// New file/directory (not in storage).
	New(PathBuf),

	/// File/directory modified (content or metadata changed).
	Modified {
		path: PathBuf,
		entry_id: i32,
		old_modified: Option<SystemTime>,
		new_modified: Option<SystemTime>,
	},

	/// File/directory moved or renamed (same inode, different path).
	Moved {
		old_path: PathBuf,
		new_path: PathBuf,
		entry_id: i32,
		inode: u64,
	},

	/// File/directory deleted (existed in storage but not on disk).
	Deleted { path: PathBuf, entry_id: i32 },
}

impl Change {
	/// Get the primary path affected by this change.
	pub fn path(&self) -> &PathBuf {
		match self {
			Change::New(path) => path,
			Change::Modified { path, .. } => path,
			Change::Moved { new_path, .. } => new_path,
			Change::Deleted { path, .. } => path,
		}
	}

	/// Get the change type for event emission.
	pub fn change_type(&self) -> ChangeType {
		match self {
			Change::New(_) => ChangeType::Created,
			Change::Modified { .. } => ChangeType::Modified,
			Change::Moved { .. } => ChangeType::Moved,
			Change::Deleted { .. } => ChangeType::Deleted,
		}
	}

	/// Create a Change from an FsEvent (for watcher integration).
	/// Note: These variants don't have entry_ids since they come from the watcher.
	pub fn from_fs_event(event: sd_fs_watcher::FsEvent) -> Self {
		use sd_fs_watcher::FsEventKind;

		match event.kind {
			FsEventKind::Create => Change::New(event.path),
			FsEventKind::Modify => Change::Modified {
				path: event.path,
				entry_id: 0, // Placeholder - handler will look up real ID
				old_modified: None,
				new_modified: None,
			},
			FsEventKind::Remove => Change::Deleted {
				path: event.path,
				entry_id: 0, // Placeholder - handler will look up real ID
			},
			FsEventKind::Rename { from, to } => Change::Moved {
				old_path: from,
				new_path: to,
				entry_id: 0, // Placeholder - handler will look up real ID
				inode: 0,
			},
		}
	}
}

/// Metadata about a change, populated during detection.
#[derive(Debug, Clone)]
pub struct ChangeMetadata {
	pub size: u64,
	pub modified: Option<SystemTime>,
	pub inode: Option<u64>,
	pub kind: EntryKind,
}

/// Type of change for event emission and logging.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChangeType {
	Created,
	Modified,
	Moved,
	Deleted,
}

/// Reference to an entry in either persistent or ephemeral storage.
///
/// Provides a uniform way to refer to entries regardless of storage backend.
/// Persistent entries have database IDs; ephemeral entries have synthetic IDs.
#[derive(Debug, Clone)]
pub struct EntryRef {
	/// For persistent: database entry ID. For ephemeral: synthetic ID.
	pub id: i32,
	/// UUID for sync and event emission.
	pub uuid: Option<Uuid>,
	/// Full filesystem path.
	pub path: PathBuf,
	/// Entry kind (file/directory/symlink).
	pub kind: EntryKind,
}

impl EntryRef {
	pub fn is_directory(&self) -> bool {
		self.kind == EntryKind::Directory
	}
}

/// Configuration for change handling operations.
pub struct ChangeConfig<'a> {
	pub rule_toggles: crate::ops::indexing::rules::RuleToggles,
	pub location_root: &'a std::path::Path,
	pub volume_backend: Option<&'a std::sync::Arc<dyn crate::volume::VolumeBackend>>,
}
