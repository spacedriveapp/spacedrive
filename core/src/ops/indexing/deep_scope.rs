//! # Deep scope
//!
//! The subtrees of a source whose file contents are read: content identity,
//! the content text index and the passes that follow them. The walk and the
//! records always cover the whole source, so browsing, search by name and
//! the analyser see everything; reading bytes stays where a person keeps
//! things.
//!
//! The scope is the roots of the source's spaces with content indexing on,
//! less the roots of those with it off (`docs/core/design/spaces.md`), plus the source's default while it has
//! one. The home folder's default is its standard user folders, which
//! [`crate::ops::spaces::store::adopt_deep_scope`] turns into spaces; any other
//! source reads its whole root, because a source someone added by hand was
//! added to be read. A list set before spaces, in the store's `deep_scope`
//! assertion, is the default until it is adopted the same way. A drive that
//! startup only maps has no store, so nothing reads its contents.

use std::{
	collections::BTreeMap,
	path::{Path, PathBuf},
};

use sd_store::DeepScope;

/// What the source at `root` reads: its default or kept subtrees, then every
/// space's root, read or skipped as its content indexing says. A space
/// decides for its own folder even where the default would read it.
pub async fn deep_scope(db: &sd_store::SourceDb, root: &Path) -> sd_store::Result<DeepScope> {
	let base = match db.deep_scope().await? {
		Some(kept) => kept,
		None => default_scope(root).await?,
	};
	let mut marked: BTreeMap<String, bool> =
		base.into_iter().map(|subtree| (subtree, true)).collect();
	for space in db.spaces().await? {
		marked.insert(space.root, space.content_indexing);
	}
	let (read, skip): (Vec<_>, Vec<_>) = marked.into_iter().partition(|(_, read)| *read);
	Ok(DeepScope {
		read: read.into_iter().map(|(subtree, _)| subtree).collect(),
		skip: skip.into_iter().map(|(subtree, _)| subtree).collect(),
	})
}

/// What the source at `root` reads before its spaces say otherwise.
pub async fn default_scope(root: &Path) -> sd_store::Result<Vec<String>> {
	let root = root.to_path_buf();
	tokio::task::spawn_blocking(move || {
		// Compared by identity, not spelling: on macOS a home source is
		// rooted at its volume's spelling, /System/Volumes/Data/Users/me,
		// while the platform names it /Users/me.
		let home = dirs::home_dir()
			.and_then(|home| std::fs::canonicalize(home).ok())
			.filter(|home| same_file::is_same_file(home, &root).unwrap_or(false));
		default_subtrees(home.as_deref(), &standard_folders())
	})
	.await
	.map_err(|e| sd_store::Error::Other(e.to_string()))
}

/// What a source keeps before anyone chose: the standard user folders when
/// the source is the home folder, at `home` as the platform spells it, and
/// the whole root for anything else.
fn default_subtrees(home: Option<&Path>, folders: &[PathBuf]) -> Vec<String> {
	let Some(home) = home else {
		return vec![String::new()];
	};
	let mut subtrees: Vec<String> = folders
		.iter()
		.filter_map(|folder| folder.strip_prefix(home).ok())
		.map(slash_path)
		.filter(|relative| !relative.is_empty())
		.collect();
	subtrees.sort();
	subtrees.dedup();
	subtrees
}

/// The operating system's standard user folders that exist: Desktop,
/// Documents, Downloads, Pictures, Movies or Videos, Music and Public,
/// resolved by the platform (known folders on macOS and Windows, XDG user
/// dirs on Linux) rather than by name. On macOS, iCloud Drive's Desktop and
/// Documents resolve to the same paths.
pub fn standard_folders() -> Vec<PathBuf> {
	[
		dirs::desktop_dir(),
		dirs::document_dir(),
		dirs::download_dir(),
		dirs::picture_dir(),
		dirs::video_dir(),
		dirs::audio_dir(),
		dirs::public_dir(),
	]
	.into_iter()
	.flatten()
	.filter_map(|dir| std::fs::canonicalize(dir).ok())
	.collect()
}

/// `subtrees` with `subtree` kept. A subtree already covered changes
/// nothing, and one that covers kept subtrees replaces them.
pub fn keep(subtrees: &[String], subtree: &str) -> Vec<String> {
	if subtrees.iter().any(|kept| covers(kept, subtree)) {
		return subtrees.to_vec();
	}
	let mut kept: Vec<String> = subtrees
		.iter()
		.filter(|kept| !covers(subtree, kept))
		.cloned()
		.collect();
	kept.push(subtree.to_string());
	kept.sort();
	kept
}

/// Whether `outer` is `inner` or one of its ancestors.
fn covers(outer: &str, inner: &str) -> bool {
	outer.is_empty()
		|| inner == outer
		|| inner
			.strip_prefix(outer)
			.is_some_and(|rest| rest.starts_with('/'))
}

/// A relative path in the store's spelling: `/`-separated on every platform.
pub fn slash_path(path: &Path) -> String {
	path.components()
		.map(|component| component.as_os_str().to_string_lossy())
		.collect::<Vec<_>>()
		.join("/")
}

#[cfg(test)]
mod tests {
	use super::*;

	fn list(items: &[&str]) -> Vec<String> {
		items.iter().map(|s| s.to_string()).collect()
	}

	#[test]
	fn the_home_folder_keeps_its_standard_folders_and_other_sources_their_root() {
		let home = Path::new("/home/ada");
		let folders = [
			home.join("Documents"),
			home.join("Desktop"),
			home.to_path_buf(),
			PathBuf::from("/srv/media"),
		];
		assert_eq!(
			default_subtrees(Some(home), &folders),
			["Desktop", "Documents"]
		);
		assert_eq!(default_subtrees(None, &folders), [""]);
	}

	#[test]
	fn keeping_a_folder_folds_into_what_already_covers_it() {
		let kept = list(&["Desktop", "Documents"]);
		assert_eq!(keep(&kept, "Documents/thesis"), kept);
		assert_eq!(
			keep(&kept, "Projects"),
			["Desktop", "Documents", "Projects"]
		);
		assert_eq!(
			keep(
				&list(&["Projects/a", "Projects/b", "Projectsx"]),
				"Projects"
			),
			["Projects", "Projectsx"]
		);
		assert_eq!(keep(&kept, ""), [""]);
		assert_eq!(keep(&list(&[""]), "Desktop"), [""]);
	}
}
