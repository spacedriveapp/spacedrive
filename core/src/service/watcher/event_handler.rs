//! Event handling for file system changes

use crate::infra::event::{Event, FsRawEventKind};
use notify::{
	event::{ModifyKind, RenameMode},
	Event as NotifyEvent, EventKind,
};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::SystemTime;
use uuid::Uuid;

/// Wrapper for file system events with additional metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WatcherEvent {
	/// The file system event kind
	pub kind: WatcherEventKind,
	/// Paths affected by the event
	pub paths: Vec<PathBuf>,
	/// Timestamp when the event was received
	pub timestamp: SystemTime,
	/// Additional attributes from the file system
	pub attrs: Vec<String>,
}

/// Types of file system events we handle
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WatcherEventKind {
	/// File or directory was created
	Create,
	/// File or directory was modified
	Modify,
	/// File or directory was removed
	Remove,
	/// File or directory was renamed/moved (from, to)
	Rename { from: PathBuf, to: PathBuf },
	/// Catch-all for other events
	Other(String),
}

impl WatcherEvent {
	/// Convert from notify's Event to our WatcherEvent
	pub fn from_notify_event(event: NotifyEvent) -> Self {
		let kind = match event.kind {
			EventKind::Create(_) => WatcherEventKind::Create,
			EventKind::Modify(ModifyKind::Name(RenameMode::Any)) => {
				WatcherEventKind::Other("rename".to_string())
			}
			EventKind::Modify(_) => WatcherEventKind::Modify,
			EventKind::Remove(_) => WatcherEventKind::Remove,
			other => WatcherEventKind::Other(format!("{:?}", other)),
		};

		let attrs = vec![format!("{:?}", event.attrs)];

		Self {
			kind,
			paths: event.paths,
			timestamp: SystemTime::now(),
			attrs,
		}
	}

	/// Convert to raw FS event for the event bus (without DB entry IDs)
	pub fn to_raw_event(&self, library_id: Uuid) -> Option<Event> {
		match &self.kind {
			WatcherEventKind::Create => self.primary_path().cloned().map(|p| Event::FsRawChange {
				library_id,
				kind: FsRawEventKind::Create { path: p },
			}),
			WatcherEventKind::Modify => self.primary_path().cloned().map(|p| Event::FsRawChange {
				library_id,
				kind: FsRawEventKind::Modify { path: p },
			}),
			WatcherEventKind::Remove => self.primary_path().cloned().map(|p| Event::FsRawChange {
				library_id,
				kind: FsRawEventKind::Remove { path: p },
			}),
			WatcherEventKind::Rename { from, to } => Some(Event::FsRawChange {
				library_id,
				kind: FsRawEventKind::Rename {
					from: from.clone(),
					to: to.clone(),
				},
			}),
			WatcherEventKind::Other(_) => None,
		}
	}

	/// Check if this event should be processed (filter out temporary files, etc.)
	///
	/// TODO: Replace this hardcoded filtering with the indexer rules engine.
	/// The rules engine in `crate::ops::indexing::rules` already handles system files,
	/// hidden files, git patterns, and more via configurable `RuleToggles`. This would
	/// allow users to customize watcher filtering through the same UI as indexer rules.
	/// See: `build_default_ruler()`, `NO_SYSTEM_FILES`, `NO_HIDDEN`, `NO_DEV_DIRS`
	pub fn should_process(&self) -> bool {
		for path in &self.paths {
			let path_str = path.to_string_lossy();

			// Skip temporary files
			if path_str.contains(".tmp")
				|| path_str.contains(".temp")
				|| path_str.contains("~")
				|| path_str.ends_with(".swp")
				|| path_str.contains(".DS_Store")
				|| path_str.contains("Thumbs.db")
			{
				return false;
			}

			// Skip hidden files starting with dot (except .gitignore, etc.)
			if let Some(file_name) = path.file_name() {
				let name = file_name.to_string_lossy();
				if name.starts_with('.') && !is_important_dotfile(&name) {
					return false;
				}
			}
		}

		true
	}

	/// Get the primary path for this event
	pub fn primary_path(&self) -> Option<&PathBuf> {
		self.paths.first()
	}
}

/// Check if a dotfile is important enough to track
fn is_important_dotfile(name: &str) -> bool {
	matches!(
		name,
		".gitignore"
			| ".gitkeep"
			| ".gitattributes"
			| ".editorconfig"
			| ".env" | ".env.local"
			| ".nvmrc"
			| ".node-version"
			| ".python-version"
			| ".dockerignore"
			| ".eslintrc"
			| ".prettierrc"
	)
}

#[cfg(test)]
mod tests {
	use super::*;
	use std::path::Path;

	#[test]
	fn test_should_process_filtering() {
		// Should process normal files
		let event = WatcherEvent {
			kind: WatcherEventKind::Create,
			paths: vec![PathBuf::from("/test/file.txt")],
			timestamp: SystemTime::now(),
			attrs: vec![],
		};
		assert!(event.should_process());

		// Should skip temporary files
		let event = WatcherEvent {
			kind: WatcherEventKind::Create,
			paths: vec![PathBuf::from("/test/file.tmp")],
			timestamp: SystemTime::now(),
			attrs: vec![],
		};
		assert!(!event.should_process());

		// Should skip .DS_Store
		let event = WatcherEvent {
			kind: WatcherEventKind::Create,
			paths: vec![PathBuf::from("/test/.DS_Store")],
			timestamp: SystemTime::now(),
			attrs: vec![],
		};
		assert!(!event.should_process());

		// Should process important dotfiles
		let event = WatcherEvent {
			kind: WatcherEventKind::Create,
			paths: vec![PathBuf::from("/test/.gitignore")],
			timestamp: SystemTime::now(),
			attrs: vec![],
		};
		assert!(event.should_process());
	}

	#[test]
	fn test_primary_path() {
		let event = WatcherEvent {
			kind: WatcherEventKind::Create,
			paths: vec![
				PathBuf::from("/test/file1.txt"),
				PathBuf::from("/test/file2.txt"),
			],
			timestamp: SystemTime::now(),
			attrs: vec![],
		};

		assert_eq!(
			event.primary_path(),
			Some(&PathBuf::from("/test/file1.txt"))
		);
	}
}
