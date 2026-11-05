//! # Staged receives
//!
//! A file received from a peer is written to a hidden sibling of its
//! destination and renamed into place only after the sender's completion
//! message has been checked against it. The final path then holds either the
//! complete, verified file or whatever was there before, never a truncated
//! one. The staging name comes from the local copy's `partial_path`, so a
//! partial looks the same whichever path wrote it, and staying in the
//! destination's directory keeps the rename on one filesystem.
//!
//! Peer transfers do not resume: a broken transfer is sent again from the
//! first byte under a new transfer id. A partial therefore has no use once its
//! transfer stops. Dropping a `StagedReceive` that was never committed removes
//! its partial, which covers errors, cancelled jobs and closed streams. A
//! process that is killed runs no destructors, so each partial is also named in
//! a journal entry under the data directory, and `sweep` removes what those
//! entries name on the next start.

use std::path::{Path, PathBuf};

use tokio::fs;
use tracing::{debug, warn};
use uuid::Uuid;

use crate::ops::files::copy::strategy::partial_path;

/// Directory under the data directory that names every open partial.
pub(crate) const JOURNAL_DIR: &str = "receiving";

/// A partial file for one receive, removed unless it is committed.
#[derive(Debug)]
pub(crate) struct StagedReceive {
	partial: PathBuf,
	journal_entry: Option<PathBuf>,
	settled: bool,
}

impl StagedReceive {
	/// Create an empty, exclusively opened partial beside `destination`.
	///
	/// The journal entry is written before the partial exists, so a kill
	/// between the two leaves an entry naming nothing rather than a partial
	/// nothing names.
	pub(crate) async fn create(
		destination: &Path,
		journal_dir: Option<&Path>,
	) -> std::io::Result<Self> {
		if let Some(parent) = destination.parent() {
			fs::create_dir_all(parent).await?;
		}
		let partial = partial_path(destination);

		let journal_entry = match journal_dir {
			Some(dir) => {
				fs::create_dir_all(dir).await?;
				let entry = dir.join(Uuid::new_v4().simple().to_string());
				fs::write(&entry, partial.to_string_lossy().as_bytes()).await?;
				Some(entry)
			}
			None => None,
		};

		let created = fs::OpenOptions::new()
			.write(true)
			.create_new(true)
			.open(&partial)
			.await;
		if let Err(e) = created {
			// Nothing was created under the name, so nothing may be removed.
			if let Some(entry) = journal_entry {
				let _ = fs::remove_file(entry).await;
			}
			return Err(e);
		}

		Ok(Self {
			partial,
			journal_entry,
			settled: false,
		})
	}

	/// Where the received bytes are written until the commit.
	pub(crate) fn path(&self) -> &Path {
		&self.partial
	}

	/// Flush the partial to disk and rename it over `destination`.
	///
	/// Call this only after the size and hash have been verified: the rename
	/// replaces whatever is at the destination.
	pub(crate) async fn commit(mut self, destination: &Path) -> std::io::Result<()> {
		fs::OpenOptions::new()
			.write(true)
			.open(&self.partial)
			.await?
			.sync_all()
			.await?;
		fs::rename(&self.partial, destination).await?;
		self.settled = true;
		if let Some(entry) = self.journal_entry.take() {
			let _ = fs::remove_file(entry).await;
		}
		Ok(())
	}
}

impl Drop for StagedReceive {
	fn drop(&mut self) {
		// Drop cannot await, and one unlink is cheap enough to run inline.
		if !self.settled {
			if let Err(e) = std::fs::remove_file(&self.partial) {
				if e.kind() != std::io::ErrorKind::NotFound {
					warn!(
						path = %self.partial.display(),
						error = %e,
						"Failed to remove an unfinished receive"
					);
				}
			}
		}
		if let Some(entry) = self.journal_entry.take() {
			let _ = std::fs::remove_file(entry);
		}
	}
}

/// Remove the partials that receives killed with their process left behind.
///
/// Run this before the handler accepts transfers, so it never races a
/// receive that has just written its entry. An entry that names anything
/// other than a staging file is dropped without touching the file, so a
/// corrupt journal cannot delete a user's data.
pub(crate) async fn sweep(journal_dir: &Path) {
	let mut entries = match fs::read_dir(journal_dir).await {
		Ok(entries) => entries,
		Err(e) if e.kind() == std::io::ErrorKind::NotFound => return,
		Err(e) => {
			warn!(dir = %journal_dir.display(), error = %e, "Failed to read the receive journal");
			return;
		}
	};
	while let Ok(Some(entry)) = entries.next_entry().await {
		let entry_path = entry.path();
		if let Ok(named) = fs::read_to_string(&entry_path).await {
			let partial = PathBuf::from(named);
			if is_staging_name(&partial) {
				match fs::remove_file(&partial).await {
					Ok(()) => debug!(path = %partial.display(), "Removed an abandoned receive"),
					Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
					Err(e) => {
						// Keep the entry so the next start tries again.
						warn!(
							path = %partial.display(),
							error = %e,
							"Failed to remove an abandoned receive"
						);
						continue;
					}
				}
			}
		}
		let _ = fs::remove_file(&entry_path).await;
	}
}

fn is_staging_name(path: &Path) -> bool {
	path.file_name()
		.and_then(|name| name.to_str())
		.is_some_and(|name| name.starts_with(".sd-") && name.ends_with(".sdpart"))
}

#[cfg(test)]
mod tests {
	use super::*;

	#[tokio::test]
	async fn uncommitted_receive_removes_its_partial() {
		let dir = tempfile::tempdir().unwrap();
		let journal = dir.path().join("journal");
		let destination = dir.path().join("file.bin");

		let staged = StagedReceive::create(&destination, Some(&journal))
			.await
			.unwrap();
		let partial = staged.path().to_path_buf();
		assert!(partial.exists());
		drop(staged);

		assert!(!partial.exists());
		assert!(!destination.exists());
		assert_eq!(std::fs::read_dir(&journal).unwrap().count(), 0);
	}

	#[tokio::test]
	async fn commit_replaces_destination_and_clears_journal() {
		let dir = tempfile::tempdir().unwrap();
		let journal = dir.path().join("journal");
		let destination = dir.path().join("file.bin");
		std::fs::write(&destination, b"old").unwrap();

		let staged = StagedReceive::create(&destination, Some(&journal))
			.await
			.unwrap();
		std::fs::write(staged.path(), b"new contents").unwrap();
		assert_eq!(std::fs::read(&destination).unwrap(), b"old");
		staged.commit(&destination).await.unwrap();

		assert_eq!(std::fs::read(&destination).unwrap(), b"new contents");
		// Only the destination and the journal directory remain.
		assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 2);
		assert_eq!(std::fs::read_dir(&journal).unwrap().count(), 0);
	}

	#[tokio::test]
	async fn sweep_removes_partials_a_killed_receive_left() {
		let dir = tempfile::tempdir().unwrap();
		let journal = dir.path().join("journal");
		let destination = dir.path().join("file.bin");

		let staged = StagedReceive::create(&destination, Some(&journal))
			.await
			.unwrap();
		let partial = staged.path().to_path_buf();
		// A killed process runs no destructors.
		std::mem::forget(staged);
		assert!(partial.exists());

		sweep(&journal).await;

		assert!(!partial.exists());
		assert_eq!(std::fs::read_dir(&journal).unwrap().count(), 0);
	}

	#[tokio::test]
	async fn sweep_leaves_files_that_are_not_partials() {
		let dir = tempfile::tempdir().unwrap();
		let journal = dir.path().join("journal");
		std::fs::create_dir_all(&journal).unwrap();
		let precious = dir.path().join("precious.txt");
		std::fs::write(&precious, b"keep").unwrap();
		std::fs::write(journal.join("entry"), precious.to_string_lossy().as_bytes()).unwrap();

		sweep(&journal).await;

		assert_eq!(std::fs::read(&precious).unwrap(), b"keep");
		assert_eq!(std::fs::read_dir(&journal).unwrap().count(), 0);
	}
}
