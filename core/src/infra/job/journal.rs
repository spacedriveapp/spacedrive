//! # The operation journal
//!
//! What a job did to the filesystem, as effects with enough to reverse
//! them: a file created, moved from one place to another, trashed with
//! where it went, replaced with where the previous bytes went, or removed
//! for good. A job records effects as it goes, in order, against its own
//! id, so an interrupted job's journal is exact up to the interruption.
//! Undo is an action over a journal, and each effect carries the state of
//! its result, so undo can refuse an effect whose subject changed since.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use specta::Type;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Effect {
	Created {
		path: PathBuf,
		subject: Option<Subject>,
	},
	Moved {
		from: PathBuf,
		to: PathBuf,
		subject: Option<Subject>,
	},
	/// `to` is where the item went, where the platform reports it.
	Trashed {
		from: PathBuf,
		to: Option<PathBuf>,
		subject: Option<Subject>,
	},
	/// `previous` is where the bytes that were at `path` went, where they
	/// were kept.
	Replaced {
		path: PathBuf,
		previous: Option<PathBuf>,
		subject: Option<Subject>,
	},
	/// Gone for good.
	Removed { path: PathBuf },
	/// The filesystem attributes of a path, before and after.
	Attributes {
		path: PathBuf,
		from: Attributes,
		to: Attributes,
	},
}

/// The state of an effect's result when it was recorded: what undo checks
/// against before touching it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct Subject {
	pub size: u64,
	pub mtime_ms: i64,
	pub is_dir: bool,
	/// Creation time, where the filesystem reports one. A directory's mtime
	/// moves with every child written into it, so this is what tells a
	/// job's folder from one that later took its place.
	#[serde(default)]
	pub created_ms: Option<i64>,
}

impl Subject {
	pub fn of(meta: &std::fs::Metadata) -> Self {
		Self {
			size: if meta.is_dir() { 0 } else { meta.len() },
			mtime_ms: meta.modified().ok().map(millis).unwrap_or(0),
			is_dir: meta.is_dir(),
			created_ms: meta.created().ok().map(millis),
		}
	}

	/// Whether the path holds what the subject recorded.
	///
	/// A job that creates a folder journals it before filling it, and each
	/// child moves the folder's mtime, so a directory is matched on its kind
	/// and its creation time instead. A journal row with no creation time
	/// (an older row, or a filesystem that keeps none) matches on kind.
	pub async fn still_holds(&self, path: &Path) -> bool {
		let Ok(meta) = tokio::fs::symlink_metadata(path).await else {
			return false;
		};
		if !self.is_dir {
			return Self::of(&meta) == *self;
		}
		if !meta.is_dir() {
			return false;
		}
		match (self.created_ms, meta.created().ok().map(millis)) {
			(Some(recorded), Some(now)) => recorded == now,
			_ => true,
		}
	}
}

fn millis(time: std::time::SystemTime) -> i64 {
	time.duration_since(std::time::UNIX_EPOCH)
		.map(|elapsed| elapsed.as_millis() as i64)
		.unwrap_or(0)
}

/// What an attribute change sets.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, Type)]
pub struct Attributes {
	/// Unix permission bits.
	pub mode: Option<u32>,
	/// Modification time, unix milliseconds.
	pub modified_ms: Option<i64>,
	pub hidden: Option<bool>,
}

impl Effect {
	pub fn created(path: PathBuf, meta: Option<&std::fs::Metadata>) -> Self {
		Self::Created {
			path,
			subject: meta.map(Subject::of),
		}
	}

	pub fn moved(from: PathBuf, to: PathBuf, meta: Option<&std::fs::Metadata>) -> Self {
		Self::Moved {
			from,
			to,
			subject: meta.map(Subject::of),
		}
	}

	pub fn trashed(from: PathBuf, to: Option<PathBuf>, meta: Option<&std::fs::Metadata>) -> Self {
		Self::Trashed {
			from,
			to,
			subject: meta.map(Subject::of),
		}
	}

	pub fn replaced(
		path: PathBuf,
		previous: Option<PathBuf>,
		meta: Option<&std::fs::Metadata>,
	) -> Self {
		Self::Replaced {
			path,
			previous,
			subject: meta.map(Subject::of),
		}
	}

	pub fn removed(path: PathBuf) -> Self {
		Self::Removed { path }
	}

	/// Whether the effect can be reversed from what it recorded.
	pub fn reversible(&self) -> bool {
		match self {
			Self::Created { .. } | Self::Moved { .. } | Self::Attributes { .. } => true,
			Self::Trashed { to, .. } => to.is_some(),
			Self::Replaced { previous, .. } => previous.is_some(),
			Self::Removed { .. } => false,
		}
	}

	/// The path the effect left its result at, where it left one.
	pub fn result(&self) -> Option<&Path> {
		match self {
			Self::Created { path, .. }
			| Self::Replaced { path, .. }
			| Self::Attributes { path, .. } => Some(path),
			Self::Moved { to, .. } => Some(to),
			Self::Trashed { to, .. } => to.as_deref(),
			Self::Removed { .. } => None,
		}
	}
}

/// One effect as the journal holds it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct Recorded {
	pub sequence: i64,
	pub effect: Effect,
	pub recorded_at: chrono::DateTime<chrono::Utc>,
}

/// What a journal amounts to, for a list of jobs.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct JournalSummary {
	pub effects: u64,
	pub reversible: u64,
}

#[cfg(test)]
mod tests {
	use super::*;

	/// A folder journaled before it is filled still holds: its mtime moved
	/// with the child, its kind did not.
	#[tokio::test]
	async fn a_directory_still_holds_after_a_child_lands_in_it() {
		let dir = tempfile::tempdir().expect("tempdir");
		let folder = dir.path().join("made");
		std::fs::create_dir(&folder).expect("folder");
		let subject = Subject::of(&std::fs::metadata(&folder).expect("meta"));
		std::thread::sleep(std::time::Duration::from_millis(20));
		std::fs::write(folder.join("child.txt"), b"c").expect("child");
		assert!(subject.still_holds(&folder).await);
		assert!(!subject.still_holds(&folder.join("child.txt")).await);

		// A folder that took the journaled one's place is not it, where the
		// filesystem keeps a creation time to tell them apart by.
		if subject.created_ms.is_some() {
			std::fs::remove_dir_all(&folder).expect("remove");
			std::thread::sleep(std::time::Duration::from_millis(20));
			std::fs::create_dir(&folder).expect("replacement");
			assert!(!subject.still_holds(&folder).await);
		}

		let file = dir.path().join("f.txt");
		std::fs::write(&file, b"abc").expect("file");
		let subject = Subject::of(&std::fs::metadata(&file).expect("meta"));
		assert!(subject.still_holds(&file).await);
		std::fs::write(&file, b"abcd").expect("rewrite");
		assert!(!subject.still_holds(&file).await);
	}
}
