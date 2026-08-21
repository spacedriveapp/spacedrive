//! Canonical per-source directory layout.
//!
//! Everything a source owns on this machine lives in one directory:
//! `sources/<id>/` under the daemon data dir holds its session-restore
//! snapshot, its thumbnail cache, and its durable store, with the source
//! registry file beside the directories at `sources/sources.json`. Clients
//! never assume this layout — per-source paths surface through the
//! `core.ephemeral_status` query.

use anyhow::{Context, Result};
use std::{
	fs,
	path::{Path, PathBuf},
};
use uuid::Uuid;

const SOURCES_DIR: &str = "sources";
const REGISTRY_FILE: &str = "sources.json";
const SNAPSHOT_FILE: &str = "ephemeral.snapshot";
const THUMBS_FILE: &str = "thumbs.pvcache";
const SOURCE_DB_FILE: &str = "source.db";
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

	/// The source registry file.
	pub fn registry_file(&self) -> PathBuf {
		self.root.join(REGISTRY_FILE)
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

	/// A source's session-restore snapshot.
	pub fn snapshot_file(&self, id: Uuid) -> PathBuf {
		self.source_dir(id).join(SNAPSHOT_FILE)
	}

	/// A source's thumbnail hot-tier cache.
	pub fn thumbs_file(&self, id: Uuid) -> PathBuf {
		self.source_dir(id).join(THUMBS_FILE)
	}

	/// A source's durable store.
	pub fn source_db_file(&self, id: Uuid) -> PathBuf {
		self.source_dir(id).join(SOURCE_DB_FILE)
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
			let dest = if name == REGISTRY_FILE {
				self.registry_file()
			} else if let Some(stem) = name.strip_suffix(".snapshot") {
				let Ok(id) = Uuid::try_parse(stem) else {
					continue;
				};
				if let Err(err) = self.create_source_dir(id) {
					tracing::warn!("Could not prepare directory for source {id}: {err:#}");
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
		assert_eq!(dirs.registry_file(), dirs.root().join("sources.json"));
		let source_dir = dirs.source_dir(id);
		assert_eq!(source_dir, dirs.root().join(id.simple().to_string()));
		assert_eq!(
			dirs.snapshot_file(id),
			source_dir.join("ephemeral.snapshot")
		);
		assert_eq!(dirs.thumbs_file(id), source_dir.join("thumbs.pvcache"));
		assert_eq!(dirs.source_db_file(id), source_dir.join("source.db"));

		// Resolution creates nothing; creation is explicit.
		assert!(!source_dir.exists());
		dirs.create_source_dir(id).unwrap();
		assert!(source_dir.exists());
	}

	#[test]
	fn adopts_superseded_layout_once() {
		let data = tempfile::tempdir().unwrap();
		let legacy = data.path().join("cache").join("volume-index");
		fs::create_dir_all(&legacy).unwrap();

		let id = Uuid::now_v7();
		fs::write(legacy.join("sources.json"), b"{}").unwrap();
		fs::write(legacy.join(format!("{}.snapshot", id.simple())), b"snap").unwrap();
		fs::write(legacy.join("junk.tmp.abc"), b"partial").unwrap();
		fs::write(legacy.join("unrelated.txt"), b"keep").unwrap();

		let dirs = SourceDirs::under_data_dir(data.path()).unwrap();

		assert_eq!(fs::read(dirs.registry_file()).unwrap(), b"{}");
		assert_eq!(fs::read(dirs.snapshot_file(id)).unwrap(), b"snap");
		assert!(!legacy.join("junk.tmp.abc").exists());
		// Unrecognized files hold the directory open rather than being lost.
		assert!(legacy.join("unrelated.txt").exists());

		// A second pass moves nothing and changes nothing.
		fs::write(legacy.join("sources.json"), b"stale").unwrap();
		let dirs = SourceDirs::under_data_dir(data.path()).unwrap();
		assert_eq!(fs::read(dirs.registry_file()).unwrap(), b"{}");

		// With only adoptable files left, the directory itself goes.
		fs::remove_file(legacy.join("unrelated.txt")).unwrap();
		fs::remove_file(legacy.join("sources.json")).unwrap();
		SourceDirs::under_data_dir(data.path()).unwrap();
		assert!(!legacy.exists());
	}
}
