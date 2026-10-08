//! # Restore trash pruning
//!
//! A `restore --replace` parks what it displaces under
//! `restore-trash/<library>-<time>/` instead of deleting it, so a restore
//! that turned out wrong can be undone by hand. Left alone those copies
//! accumulate a whole library's stores per restore, so a successful restore
//! prunes the directory down to the newest few entries for that library.
//! The entry the restore just wrote is never pruned, whatever the limit:
//! it is the only way back from the restore that is still fresh.

use std::path::{Path, PathBuf};
use uuid::Uuid;

/// Remove this library's oldest trash entries beyond `keep`, never
/// `just_created`, and return the paths removed.
///
/// Entries are `<library id>-<UTC time>` directories, so their names sort
/// in creation order and nothing has to be read to rank them. Other
/// libraries' entries and anything not matching the pattern are left alone.
/// A removal that fails is logged and skipped: pruning is housekeeping
/// after a restore that already succeeded, and must not turn that success
/// into an error.
pub async fn prune(
	trash_root: &Path,
	library_id: Uuid,
	keep: u32,
	just_created: Option<&Path>,
) -> Vec<PathBuf> {
	let prefix = format!("{}-", library_id.simple());
	let mut entries = Vec::new();
	let Ok(mut dir) = tokio::fs::read_dir(trash_root).await else {
		return entries;
	};
	while let Ok(Some(entry)) = dir.next_entry().await {
		let name = entry.file_name();
		let Some(name) = name.to_str() else {
			continue;
		};
		if !name.starts_with(&prefix) || !entry.path().is_dir() {
			continue;
		}
		entries.push(entry.path());
	}
	entries.sort();
	entries.reverse();

	let mut removed = Vec::new();
	for path in entries.into_iter().skip(keep as usize) {
		if just_created.is_some_and(|current| current == path) {
			continue;
		}
		match tokio::fs::remove_dir_all(&path).await {
			Ok(()) => {
				tracing::info!(
					library = %library_id,
					path = %path.display(),
					"pruned displaced library state from restore-trash"
				);
				removed.push(path);
			}
			Err(error) => tracing::warn!(
				library = %library_id,
				path = %path.display(),
				%error,
				"could not prune restore-trash entry; leaving it"
			),
		}
	}
	removed
}

#[cfg(test)]
mod tests {
	use super::prune;
	use std::path::{Path, PathBuf};
	use uuid::Uuid;

	fn entry(root: &Path, id: Uuid, stamp: &str) -> PathBuf {
		let dir = root.join(format!("{}-{stamp}", id.simple()));
		std::fs::create_dir_all(dir.join("library")).unwrap();
		std::fs::write(dir.join("library").join("library.json"), "{}").unwrap();
		dir
	}

	#[tokio::test]
	async fn keeps_newest_per_library_and_the_current_entry() {
		let tmp = tempfile::tempdir().unwrap();
		let a = Uuid::now_v7();
		let b = Uuid::now_v7();
		let a1 = entry(tmp.path(), a, "20261001T000000Z");
		let a2 = entry(tmp.path(), a, "20261002T000000Z");
		let a3 = entry(tmp.path(), a, "20261003T000000Z");
		let a4 = entry(tmp.path(), a, "20261004T000000Z");
		let b1 = entry(tmp.path(), b, "20260901T000000Z");
		let stray_file = tmp.path().join(format!("{}-notadir", a.simple()));
		std::fs::write(&stray_file, "x").unwrap();
		let unrelated = tmp.path().join("something-else");
		std::fs::create_dir(&unrelated).unwrap();

		let removed = prune(tmp.path(), a, 2, Some(&a4)).await;

		assert_eq!(removed, vec![a2.clone(), a1.clone()]);
		assert!(!a1.exists());
		assert!(!a2.exists());
		assert!(a3.exists());
		assert!(a4.exists());
		assert!(b1.exists(), "another library's entries are untouched");
		assert!(stray_file.exists());
		assert!(unrelated.exists());
	}

	#[tokio::test]
	async fn zero_keeps_only_the_current_entry() {
		let tmp = tempfile::tempdir().unwrap();
		let a = Uuid::now_v7();
		let old = entry(tmp.path(), a, "20261001T000000Z");
		let current = entry(tmp.path(), a, "20261002T000000Z");

		let removed = prune(tmp.path(), a, 0, Some(&current)).await;

		assert_eq!(removed, vec![old.clone()]);
		assert!(!old.exists());
		assert!(current.exists());
	}

	#[tokio::test]
	async fn missing_trash_dir_and_nothing_to_prune_are_quiet() {
		let tmp = tempfile::tempdir().unwrap();
		let a = Uuid::now_v7();
		assert!(prune(&tmp.path().join("absent"), a, 2, None)
			.await
			.is_empty());
		let only = entry(tmp.path(), a, "20261001T000000Z");
		assert!(prune(tmp.path(), a, 2, Some(&only)).await.is_empty());
		assert!(only.exists());
	}
}
