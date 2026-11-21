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

impl StagedReceive {
	/// Flush the partial and rename it to `destination` only if nothing is
	/// there.
	///
	/// A taken name answers `AlreadyExists` and leaves the partial staged, so
	/// the caller can commit it under another name.
	pub(crate) async fn commit_new(&mut self, destination: &Path) -> std::io::Result<()> {
		fs::OpenOptions::new()
			.write(true)
			.open(&self.partial)
			.await?
			.sync_all()
			.await?;
		let (from, to) = (self.partial.clone(), destination.to_path_buf());
		tokio::task::spawn_blocking(move || rename_noreplace(&from, &to))
			.await
			.map_err(|e| std::io::Error::other(e.to_string()))??;
		self.settled = true;
		if let Some(entry) = self.journal_entry.take() {
			let _ = fs::remove_file(entry).await;
		}
		Ok(())
	}
}

/// Rename `from` to `to`, failing with `AlreadyExists` instead of replacing
/// what is at `to`.
///
/// Linux and macOS check and rename in one call. A filesystem that does not
/// support that call, and every other platform, check first and then rename,
/// which leaves a window in which another process could create `to`.
pub(crate) fn rename_noreplace(from: &Path, to: &Path) -> std::io::Result<()> {
	#[cfg(any(target_os = "linux", target_os = "macos"))]
	{
		use std::{ffi::CString, os::unix::ffi::OsStrExt};

		let from_c = CString::new(from.as_os_str().as_bytes())?;
		let to_c = CString::new(to.as_os_str().as_bytes())?;
		#[cfg(target_os = "linux")]
		let rc = unsafe {
			libc::renameat2(
				libc::AT_FDCWD,
				from_c.as_ptr(),
				libc::AT_FDCWD,
				to_c.as_ptr(),
				libc::RENAME_NOREPLACE as _,
			)
		};
		#[cfg(target_os = "macos")]
		let rc = unsafe { libc::renamex_np(from_c.as_ptr(), to_c.as_ptr(), libc::RENAME_EXCL) };
		if rc == 0 {
			return Ok(());
		}
		let error = std::io::Error::last_os_error();
		if !matches!(
			error.raw_os_error(),
			Some(libc::EINVAL) | Some(libc::ENOSYS) | Some(libc::ENOTSUP)
		) {
			return Err(error);
		}
	}
	if std::fs::symlink_metadata(to).is_ok() {
		return Err(std::io::Error::new(
			std::io::ErrorKind::AlreadyExists,
			format!("{} exists", to.display()),
		));
	}
	std::fs::rename(from, to)
}

impl Drop for StagedReceive {
	fn drop(&mut self) {
		let partial = (!self.settled).then(|| std::mem::take(&mut self.partial));
		let entry = self.journal_entry.take();
		if partial.is_none() && entry.is_none() {
			return;
		}
		// Unlinking can stall on slow or network storage, so it runs on the
		// blocking pool. A cleanup that never runs, because the runtime shut
		// down first, keeps its journal entry for the next start's sweep.
		match tokio::runtime::Handle::try_current() {
			Ok(runtime) => {
				runtime.spawn_blocking(move || discard(partial, entry));
			}
			Err(_) => discard(partial, entry),
		}
	}
}

/// Remove an unfinished partial, then the journal entry that names it.
///
/// The entry outlives a failed removal so the next start's sweep retries it
/// once the filesystem answers again.
fn discard(partial: Option<PathBuf>, entry: Option<PathBuf>) {
	if let Some(partial) = partial {
		if let Err(e) = std::fs::remove_file(&partial) {
			if e.kind() != std::io::ErrorKind::NotFound {
				warn!(
					path = %partial.display(),
					error = %e,
					"Failed to remove an unfinished receive"
				);
				return;
			}
		}
	}
	if let Some(entry) = entry {
		let _ = std::fs::remove_file(entry);
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
pub(crate) mod tests {
	use super::*;

	/// Wait for a drop's cleanup, which runs on the blocking pool.
	pub(crate) async fn eventually(what: &str, check: impl Fn() -> bool) {
		for _ in 0..200 {
			if check() {
				return;
			}
			tokio::time::sleep(std::time::Duration::from_millis(10)).await;
		}
		panic!("timed out waiting for {}", what);
	}

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

		eventually("the partial and its journal entry to go", || {
			!partial.exists() && std::fs::read_dir(&journal).unwrap().count() == 0
		})
		.await;
		assert!(!destination.exists());
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
