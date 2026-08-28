//! Counting a subtree without keeping it.
//!
//! What the walk keeps and what the map has to account for are two different
//! questions. A directory the rules turn back at still holds bytes, and a map
//! that leaves them out shows a drive with hundreds of gigabytes missing and no
//! indication of where they went. So the walk descends anyway, counts, and
//! keeps one entry standing for the whole subtree.
//!
//! Traversal is what it costs either way. No filesystem keeps a running total
//! per directory, so the numbers have to be counted whatever the policy is;
//! what a summary saves is the retention, which is where the memory goes.

use crate::ops::indexing::ephemeral::Rollup;
use serde::{Deserialize, Serialize};
use specta::Type;
use std::path::{Path, PathBuf};

/// Levels below a volume root that a summarised walk keeps in full.
///
/// Depth is a proxy for size, and it is the one a single pass can afford: the
/// alternative is counting a subtree to decide whether to walk it, and then
/// walking everything large a second time. Seven levels covers the structure a
/// person recognises on a drive and is where the measured cost stays in tens of
/// megabytes rather than gigabytes.
pub const SUMMARY_DEPTH: usize = 7;

/// What a walk keeps of what it visits.
///
/// Every field is off by default, which is the walk that keeps what it accepts
/// and drops the rest: a browse, or a location index.
#[derive(Debug, Clone, Default, Serialize, Deserialize, Type)]
pub struct Retention {
	/// Keep a directory the rules turn back at, with a count standing in for
	/// its contents. Without this a drive shows hundreds of gigabytes missing
	/// and no indication of where they went.
	pub summarise_rejected: bool,
	/// Levels below the root past which a directory is counted rather than
	/// walked into.
	pub depth: Option<usize>,
	/// Roots another walk owns. They share this arena, so their entries and
	/// every rollup above them arrive whether or not this walk descends;
	/// walking them again would be the same tree built twice.
	pub covered: Vec<PathBuf>,
}

/// What to do with a directory the walk has reached.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Descent {
	/// Read it, and its children after it.
	Walk,
	/// Count it and keep the total in place of its contents.
	Summarise,
	/// Leave it to whoever is already walking it.
	Skip,
}

impl Retention {
	/// A walk that keeps everything it accepts and nothing else.
	pub fn everything() -> Self {
		Self::default()
	}

	/// A source's own walk: full fidelity, and an accounting for the
	/// directories the rules hold back.
	pub fn source() -> Self {
		Self {
			summarise_rejected: true,
			..Self::default()
		}
	}

	/// The background map of a drive: structure near the root, counts below it,
	/// and nothing where another walk is already working.
	pub fn map(covered: Vec<PathBuf>) -> Self {
		Self {
			summarise_rejected: true,
			depth: Some(SUMMARY_DEPTH),
			covered,
		}
	}

	/// What this walk does with `path`, a directory under `root`.
	pub fn at(&self, path: &Path, root: &Path) -> Descent {
		if self.covered.iter().any(|owned| path.starts_with(owned)) {
			return Descent::Skip;
		}
		let too_deep = self.depth.is_some_and(|depth| {
			path.strip_prefix(root)
				.map(|relative| relative.components().count() > depth)
				.unwrap_or(false)
		});
		if too_deep {
			Descent::Summarise
		} else {
			Descent::Walk
		}
	}
}

/// Total bytes and file count beneath a directory.
///
/// Symlinks are counted as neither, and never followed, so a link back up the
/// tree cannot make the count run forever.
pub async fn count_subtree(root: PathBuf) -> Rollup {
	tokio::task::spawn_blocking(move || {
		let mut totals = Rollup::default();
		let mut stack = vec![root];

		while let Some(dir) = stack.pop() {
			let Ok(entries) = std::fs::read_dir(&dir) else {
				continue;
			};
			for entry in entries.flatten() {
				let Ok(file_type) = entry.file_type() else {
					continue;
				};
				if file_type.is_dir() {
					stack.push(entry.path());
				} else if file_type.is_file() {
					let size = entry.metadata().map(|meta| meta.len()).unwrap_or(0);
					totals += Rollup::file(size);
				}
			}
		}

		totals
	})
	.await
	.unwrap_or_default()
}

#[cfg(test)]
mod tests {
	use super::*;

	fn summarised(covered: &[&str]) -> Retention {
		Retention {
			summarise_rejected: true,
			depth: Some(2),
			covered: covered.iter().map(PathBuf::from).collect(),
		}
	}

	#[test]
	fn everything_walks_everything() {
		let root = Path::new("/vol");
		assert_eq!(
			Retention::everything().at(Path::new("/vol/a/b/c/d/e"), root),
			Descent::Walk
		);
	}

	#[test]
	fn depth_decides_what_becomes_a_count() {
		let root = Path::new("/vol");
		let retention = summarised(&[]);
		assert_eq!(
			retention.at(Path::new("/vol/Applications"), root),
			Descent::Walk
		);
		assert_eq!(
			retention.at(Path::new("/vol/Applications/Thing.app"), root),
			Descent::Walk
		);
		assert_eq!(
			retention.at(Path::new("/vol/Applications/Thing.app/Contents"), root),
			Descent::Summarise
		);
	}

	#[test]
	fn a_root_another_walk_owns_is_left_alone() {
		let root = Path::new("/vol");
		let retention = summarised(&["/vol/Users/me"]);
		assert_eq!(
			retention.at(Path::new("/vol/Users/me/a/b/c/d"), root),
			Descent::Skip
		);
		assert_eq!(
			retention.at(Path::new("/vol/Users/other/a/b/c/d"), root),
			Descent::Summarise
		);
	}
}
