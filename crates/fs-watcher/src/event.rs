//! Filesystem event types
//!
//! Storage-agnostic event types that represent raw filesystem changes.
//! These events contain only paths and change kinds - no library IDs,
//! no database references, no routing decisions.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::SystemTime;

/// A filesystem change event
///
/// This is a normalized, platform-agnostic representation of a filesystem change.
/// Platform-specific handlers translate OS events into these normalized events.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FsEvent {
	/// The path affected by this event
	pub path: PathBuf,
	/// The kind of change
	pub kind: FsEventKind,
	/// When the event was detected
	pub timestamp: SystemTime,
	/// Whether the path is a directory (avoids extra fs::metadata calls downstream)
	/// Note: For Remove events, this may be None if the path no longer exists
	pub is_directory: Option<bool>,
}

impl FsEvent {
	/// Create a new filesystem event
	pub fn new(path: PathBuf, kind: FsEventKind) -> Self {
		Self {
			path,
			kind,
			timestamp: SystemTime::now(),
			is_directory: None,
		}
	}

	/// Create a new filesystem event with directory flag
	pub fn new_with_dir_flag(path: PathBuf, kind: FsEventKind, is_directory: bool) -> Self {
		Self {
			path,
			kind,
			timestamp: SystemTime::now(),
			is_directory: Some(is_directory),
		}
	}

	/// Create a create event
	pub fn create(path: PathBuf) -> Self {
		Self::new(path, FsEventKind::Create)
	}

	/// Create a create event for a directory
	pub fn create_dir(path: PathBuf) -> Self {
		Self::new_with_dir_flag(path, FsEventKind::Create, true)
	}

	/// Create a create event for a file
	pub fn create_file(path: PathBuf) -> Self {
		Self::new_with_dir_flag(path, FsEventKind::Create, false)
	}

	/// Create a modify event
	pub fn modify(path: PathBuf) -> Self {
		Self::new(path, FsEventKind::Modify)
	}

	/// Create a modify event for a file (directories typically don't get modify events)
	pub fn modify_file(path: PathBuf) -> Self {
		Self::new_with_dir_flag(path, FsEventKind::Modify, false)
	}

	/// Create a remove event
	pub fn remove(path: PathBuf) -> Self {
		Self::new(path, FsEventKind::Remove)
	}

	/// Create a rename event
	pub fn rename(from: PathBuf, to: PathBuf) -> Self {
		Self {
			path: to.clone(),
			kind: FsEventKind::Rename { from, to },
			timestamp: SystemTime::now(),
			is_directory: None,
		}
	}

	/// Create a rename event with directory flag
	pub fn rename_with_dir_flag(from: PathBuf, to: PathBuf, is_directory: bool) -> Self {
		Self {
			path: to.clone(),
			kind: FsEventKind::Rename { from, to },
			timestamp: SystemTime::now(),
			is_directory: Some(is_directory),
		}
	}

	/// Check if this event is for a directory
	pub fn is_dir(&self) -> Option<bool> {
		self.is_directory
	}

	/// Check if this event is for a file
	pub fn is_file(&self) -> Option<bool> {
		self.is_directory.map(|d| !d)
	}
}

/// The kind of filesystem change
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum FsEventKind {
	/// A file or directory was created
	Create,
	/// A file or directory was modified
	Modify,
	/// A file or directory was removed
	Remove,
	/// A file or directory was renamed/moved
	Rename {
		/// The original path
		from: PathBuf,
		/// The new path
		to: PathBuf,
	},
}

impl FsEventKind {
	/// Check if this is a create event
	pub fn is_create(&self) -> bool {
		matches!(self, Self::Create)
	}

	/// Check if this is a modify event
	pub fn is_modify(&self) -> bool {
		matches!(self, Self::Modify)
	}

	/// Check if this is a remove event
	pub fn is_remove(&self) -> bool {
		matches!(self, Self::Remove)
	}

	/// Check if this is a rename event
	pub fn is_rename(&self) -> bool {
		matches!(self, Self::Rename { .. })
	}
}

/// Raw event from notify crate before platform processing
#[derive(Debug, Clone)]
pub struct RawNotifyEvent {
	/// The kind of event from notify
	pub kind: RawEventKind,
	/// Paths affected by the event
	pub paths: Vec<PathBuf>,
	/// Timestamp when received
	pub timestamp: SystemTime,
}

/// Raw event kinds from notify
#[derive(Debug, Clone)]
pub enum RawEventKind {
	/// Create event
	Create,
	/// Modify event
	Modify,
	/// Remove event
	Remove,
	/// Rename event (platform-specific semantics)
	Rename,
	/// Other/unknown event type
	Other(String),
}

impl RawNotifyEvent {
	/// Create from a notify event
	pub fn from_notify(event: notify::Event) -> Self {
		use notify::event::{ModifyKind, RenameMode};
		use notify::EventKind;

		let kind = match event.kind {
			EventKind::Create(_) => RawEventKind::Create,
			EventKind::Modify(ModifyKind::Name(RenameMode::Any)) => RawEventKind::Rename,
			EventKind::Modify(ModifyKind::Name(RenameMode::From)) => RawEventKind::Rename,
			EventKind::Modify(ModifyKind::Name(RenameMode::To)) => RawEventKind::Rename,
			EventKind::Modify(ModifyKind::Name(RenameMode::Both)) => RawEventKind::Rename,
			EventKind::Modify(_) => RawEventKind::Modify,
			EventKind::Remove(_) => RawEventKind::Remove,
			other => RawEventKind::Other(format!("{:?}", other)),
		};

		Self {
			kind,
			paths: event.paths,
			timestamp: SystemTime::now(),
		}
	}

	/// Get the primary path for this event
	pub fn primary_path(&self) -> Option<&PathBuf> {
		self.paths.first()
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_event_creation() {
		let path = PathBuf::from("/test/file.txt");

		let event = FsEvent::create(path.clone());
		assert!(event.kind.is_create());
		assert_eq!(event.path, path);
		assert!(event.is_directory.is_none());

		let event = FsEvent::modify(path.clone());
		assert!(event.kind.is_modify());

		let event = FsEvent::remove(path.clone());
		assert!(event.kind.is_remove());
	}

	#[test]
	fn test_directory_flag() {
		let path = PathBuf::from("/test/dir");

		let event = FsEvent::create_dir(path.clone());
		assert!(event.kind.is_create());
		assert_eq!(event.is_dir(), Some(true));
		assert_eq!(event.is_file(), Some(false));

		let event = FsEvent::create_file(path.clone());
		assert!(event.kind.is_create());
		assert_eq!(event.is_dir(), Some(false));
		assert_eq!(event.is_file(), Some(true));

		// Generic create has no flag
		let event = FsEvent::create(path.clone());
		assert!(event.is_dir().is_none());
	}

	#[test]
	fn test_rename_event() {
		let from = PathBuf::from("/test/old.txt");
		let to = PathBuf::from("/test/new.txt");

		let event = FsEvent::rename(from.clone(), to.clone());
		assert!(event.kind.is_rename());

		if let FsEventKind::Rename { from: f, to: t } = &event.kind {
			assert_eq!(f, &from);
			assert_eq!(t, &to);
		} else {
			panic!("Expected rename event");
		}

		// Test rename with directory flag
		let event = FsEvent::rename_with_dir_flag(from.clone(), to.clone(), true);
		assert_eq!(event.is_dir(), Some(true));
	}
}
