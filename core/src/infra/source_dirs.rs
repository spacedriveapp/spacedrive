//! Canonical on-disk layout for what a machine keeps about its drives.
//!
//! Two directories, because two different things are being stored. A *medium*
//! is a drive: `mediums/<id>/` holds the arena's session-restore snapshot and
//! its thumbnail cache, both of which describe the whole drive whatever is
//! registered over it. A *source* is a scope over one: `sources/<id>/` holds
//! its durable store (whose file name belongs to `sd_store::SourceManager`)
//! and its streamed block cache.
//!
//! Keeping them apart is what lets a source nest inside another. Both would
//! otherwise want the same directory for the same drive's map, and the inner
//! one would end up with a second copy of it.
//!
//! These are machine-local artifacts; the registration itself is library
//! metadata and lives in the `sources` table. Clients never assume this
//! layout — per-source paths surface through the `core.ephemeral_status`
//! query.

use anyhow::{Context, Result};
use std::{
	fs,
	path::{Path, PathBuf},
};
use uuid::Uuid;

const SOURCES_DIR: &str = "sources";
const MEDIUMS_DIR: &str = "mediums";
const SNAPSHOT_FILE: &str = "ephemeral.snapshot";
const THUMBS_FILE: &str = "thumbs.pvcache";
const BLOCKS_DIR: &str = "blocks";

/// Resolves the on-disk layout for per-source storage.
#[derive(Debug, Clone)]
pub struct SourceDirs {
	root: PathBuf,
}

impl SourceDirs {
	/// A layout rooted at an explicit directory, created if absent.
	pub fn new(root: PathBuf) -> Result<Self> {
		fs::create_dir_all(&root)
			.with_context(|| format!("create sources directory {}", root.display()))?;
		Ok(Self { root })
	}

	/// The layout under a daemon data directory (`<data>/sources`), adopting
	/// any files still in the superseded cache location.
	pub fn under_data_dir(data_dir: &Path) -> Result<Self> {
		let dirs = Self::new(data_dir.join(SOURCES_DIR))?;
		dirs.adopt_files_from(&data_dir.join("cache").join("volume-index"));
		Ok(dirs)
	}

	/// The layout under the default data directory.
	pub fn from_default_data_dir() -> Result<Self> {
		Self::under_data_dir(&crate::config::default_data_dir()?)
	}

	pub fn root(&self) -> &Path {
		&self.root
	}

	/// A source's directory. Resolution only; see [`Self::create_source_dir`].
	pub fn source_dir(&self, id: Uuid) -> PathBuf {
		self.root.join(id.simple().to_string())
	}

	/// A source's directory, created on demand.
	pub fn create_source_dir(&self, id: Uuid) -> Result<PathBuf> {
		let dir = self.source_dir(id);
		fs::create_dir_all(&dir)
			.with_context(|| format!("create source directory {}", dir.display()))?;
		Ok(dir)
	}

	/// A medium's directory, created on demand.
	pub fn create_medium_dir(&self, id: Uuid) -> Result<PathBuf> {
		let dir = self.medium_dir(id);
		fs::create_dir_all(&dir)
			.with_context(|| format!("create medium directory {}", dir.display()))?;
		Ok(dir)
	}

	/// A medium's directory: the drive's map and everything derived from it.
	pub fn medium_dir(&self, id: Uuid) -> PathBuf {
		self.root
			.parent()
			.map(|parent| parent.join(MEDIUMS_DIR))
			.unwrap_or_else(|| self.root.join(MEDIUMS_DIR))
			.join(id.simple().to_string())
	}

	/// A medium's session-restore snapshot: the arena's durable copy.
	pub fn snapshot_file(&self, id: Uuid) -> PathBuf {
		self.medium_dir(id).join(SNAPSHOT_FILE)
	}

	/// A medium's thumbnail hot-tier cache. Keyed here rather than per source
	/// because a tile is a rendering of the drive's contents, and two sources
	/// over one drive should not each decode it.
	pub fn thumbs_file(&self, id: Uuid) -> PathBuf {
		self.medium_dir(id).join(THUMBS_FILE)
	}

	/// A source's streamed block cache. Inside the source's directory so
	/// that forgetting a source takes its blocks with it, and a detached
	/// drive's cache scopes itself.
	pub fn blocks_dir(&self, id: Uuid) -> PathBuf {
		self.source_dir(id).join(BLOCKS_DIR)
	}

	/// Ids that currently have a directory in this layout, whether or not
	/// they are registered — the block cache rebuilds its ledger from this.
	pub fn source_ids(&self) -> Vec<Uuid> {
		let Ok(entries) = fs::read_dir(&self.root) else {
			return Vec::new();
		};
		entries
			.flatten()
			.filter_map(|entry| {
				let name = entry.file_name();
				Uuid::try_parse(name.to_str()?).ok()
			})
			.collect()
	}

	/// Move the registry file and per-source snapshots out of a superseded
	/// directory into this layout. Best-effort per file: a file whose
	/// destination already exists stays where it is, leftovers from
	/// interrupted writes are removed, unrecognized files are ignored, and
	/// the directory itself is removed once empty.
	fn adopt_files_from(&self, dir: &Path) {
		let Ok(entries) = fs::read_dir(dir) else {
			return;
		};
		for entry in entries.flatten() {
			let path = entry.path();
			let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
				continue;
			};
			if name.contains(".tmp") {
				let _ = fs::remove_file(&path);
				continue;
			}
			let dest = if let Some(stem) = name.strip_suffix(".snapshot") {
				let Ok(id) = Uuid::try_parse(stem) else {
					continue;
				};
				if let Err(err) = self.create_medium_dir(id) {
					tracing::warn!("Could not prepare directory for medium {id}: {err:#}");
					continue;
				}
				self.snapshot_file(id)
			} else {
				continue;
			};
			if dest.exists() {
				continue;
			}
			match move_file(&path, &dest) {
				Ok(()) => tracing::info!("Adopted {} into {}", path.display(), dest.display()),
				Err(err) => tracing::warn!("Could not adopt {}: {err}", path.display()),
			}
		}
		let _ = fs::remove_dir(dir);
	}
}

/// Rename, falling back to copy-and-remove for cross-device moves.
fn move_file(from: &Path, to: &Path) -> std::io::Result<()> {
	match fs::rename(from, to) {
		Ok(()) => Ok(()),
		Err(_) => {
			fs::copy(from, to)?;
			fs::remove_file(from)
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn layout_resolves_per_source() {
		let data = tempfile::tempdir().unwrap();
		let dirs = SourceDirs::under_data_dir(data.path()).unwrap();
		let id = Uuid::now_v7();

		assert_eq!(dirs.root(), data.path().join("sources"));
		let source_dir = dirs.source_dir(id);
		assert_eq!(source_dir, dirs.root().join(id.simple().to_string()));

		// The map of a drive and what is kept off it are different directories,
		// so a source nested in another does not want the same one.
		let medium_dir = dirs.medium_dir(id);
		assert_eq!(
			medium_dir,
			data.path().join("mediums").join(id.simple().to_string())
		);
		assert_eq!(
			dirs.snapshot_file(id),
			medium_dir.join("ephemeral.snapshot")
		);
		assert_eq!(dirs.thumbs_file(id), medium_dir.join("thumbs.pvcache"));

		// Resolution creates nothing; creation is explicit.
		assert!(!source_dir.exists());
		assert!(!medium_dir.exists());
		dirs.create_source_dir(id).unwrap();
		dirs.create_medium_dir(id).unwrap();
		assert!(source_dir.exists());
		assert!(medium_dir.exists());
	}

	#[test]
	fn adopts_superseded_layout_once() {
		let data = tempfile::tempdir().unwrap();
		let legacy = data.path().join("cache").join("volume-index");
		fs::create_dir_all(&legacy).unwrap();

		let id = Uuid::now_v7();
		fs::write(legacy.join(format!("{}.snapshot", id.simple())), b"snap").unwrap();
		fs::write(legacy.join("junk.tmp.abc"), b"partial").unwrap();
		fs::write(legacy.join("unrelated.txt"), b"keep").unwrap();

		let dirs = SourceDirs::under_data_dir(data.path()).unwrap();

		assert_eq!(fs::read(dirs.snapshot_file(id)).unwrap(), b"snap");
		assert!(!legacy.join("junk.tmp.abc").exists());
		// Unrecognized files hold the directory open rather than being lost.
		assert!(legacy.join("unrelated.txt").exists());

		// A second pass moves nothing and changes nothing.
		fs::write(legacy.join(format!("{}.snapshot", id.simple())), b"stale").unwrap();
		let dirs = SourceDirs::under_data_dir(data.path()).unwrap();
		assert_eq!(fs::read(dirs.snapshot_file(id)).unwrap(), b"snap");

		// With only adoptable files left, the directory itself goes.
		fs::remove_file(legacy.join("unrelated.txt")).unwrap();
		fs::remove_file(legacy.join(format!("{}.snapshot", id.simple()))).unwrap();
		SourceDirs::under_data_dir(data.path()).unwrap();
		assert!(!legacy.exists());
	}
}
