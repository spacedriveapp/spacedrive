//! Canonical on-disk layout for what a machine keeps about its drives.
//!
//! Two directories, because two different things are being stored. A *volume
//! index* is a drive's map: `volumes/<id>/` holds its session-restore snapshot
//! and its thumbnail cache, both of which describe the whole drive whatever is
//! registered over it. A *source* is a scope over one: `sources/<id>/` holds
//! its durable store (whose file name belongs to `sd_store::SourceManager`),
//! its thumbnail sidecars and its streamed block cache.
//!
//! Keeping them apart is what lets a source nest inside another. Both would
//! otherwise want the same directory for the same drive's map, and the inner
//! one would end up with a second copy of it.
//!
//! These are machine-local artifacts; the registration itself is library
//! metadata and lives in the `sources` table. Clients never assume this
//! layout — per-source paths surface through the `core.index_status`
//! query.

use anyhow::{Context, Result};
use std::{
	fs,
	path::{Path, PathBuf},
};
use uuid::Uuid;

const SOURCES_DIR: &str = "sources";
const VOLUMES_DIR: &str = "volumes";
const SNAPSHOT_FILE: &str = "ephemeral.snapshot";
const THUMBS_FILE: &str = "thumbs.pvcache";
const THUMBSTRIPS_DIR: &str = "thumbstrips";
const SIDECARS_FILE: &str = "sidecars.db";
const BLOCKS_DIR: &str = "blocks";
const OFFLINE_COPY_FILE: &str = "offline-copy.db";
const OFFLINE_COPY_MANIFEST: &str = "offline-copy.json";
const OFFLINE_COPY_PART: &str = "offline-copy.db.part";

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

	/// A volume index's directory, created on demand.
	pub fn create_volume_dir(&self, id: Uuid) -> Result<PathBuf> {
		let dir = self.volume_dir(id);
		fs::create_dir_all(&dir)
			.with_context(|| format!("create volume index directory {}", dir.display()))?;
		Ok(dir)
	}

	/// A volume index's directory: the drive's map and everything derived
	/// from it.
	pub fn volume_dir(&self, id: Uuid) -> PathBuf {
		self.root
			.parent()
			.map(|parent| parent.join(VOLUMES_DIR))
			.unwrap_or_else(|| self.root.join(VOLUMES_DIR))
			.join(id.simple().to_string())
	}

	/// A volume index's session-restore snapshot: the arena's durable copy.
	pub fn snapshot_file(&self, id: Uuid) -> PathBuf {
		self.volume_dir(id).join(SNAPSHOT_FILE)
	}

	/// A volume index's thumbnail hot-tier cache. Keyed here rather than per source
	/// because a tile is a rendering of the drive's contents, and two sources
	/// over one drive should not each decode it.
	pub fn thumbs_file(&self, id: Uuid) -> PathBuf {
		self.volume_dir(id).join(THUMBS_FILE)
	}

	/// On-demand video scrub sheets keyed by record and content version. They
	/// follow the volume index because the record identity and invalidation
	/// evidence come from that map, just like hot thumbnail tiles.
	pub fn thumbstrip_file(&self, id: Uuid, record_id: Uuid, version: u64) -> PathBuf {
		self.volume_dir(id)
			.join(THUMBSTRIPS_DIR)
			.join(record_id.simple().to_string())
			.join(format!("{version}.png"))
	}

	/// A source's thumbnail sidecars: every tile it has baked, kept durably.
	/// Per source rather than per drive, unlike the hot cache, because they
	/// are what a peer replicating the source copies.
	pub fn sidecars_file(&self, id: Uuid) -> PathBuf {
		self.source_dir(id).join(SIDECARS_FILE)
	}

	/// The library's replica of a store placed on its source: a consistent
	/// copy of the on-drive `data.db`, kept here so the catalog answers
	/// while the drive is away. Its own file name, never `data.db`, so the
	/// in-library layout of a source whose store lives on the drive is
	/// never mistaken for a store this machine writes or adopts.
	pub fn offline_copy_file(&self, id: Uuid) -> PathBuf {
		self.source_dir(id).join(OFFLINE_COPY_FILE)
	}

	/// What the offline copy was taken from and when, beside the copy.
	pub fn offline_copy_manifest(&self, id: Uuid) -> PathBuf {
		self.source_dir(id).join(OFFLINE_COPY_MANIFEST)
	}

	/// Where a copy is written before it is validated and renamed into
	/// place, so a failed export never replaces a good copy.
	pub fn offline_copy_part(&self, id: Uuid) -> PathBuf {
		self.source_dir(id).join(OFFLINE_COPY_PART)
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
				if let Err(err) = self.create_volume_dir(id) {
					tracing::warn!("Could not prepare directory for volume index {id}: {err:#}");
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
		let volume_dir = dirs.volume_dir(id);
		assert_eq!(
			volume_dir,
			data.path().join("volumes").join(id.simple().to_string())
		);
		assert_eq!(
			dirs.snapshot_file(id),
			volume_dir.join("ephemeral.snapshot")
		);
		assert_eq!(dirs.thumbs_file(id), volume_dir.join("thumbs.pvcache"));
		assert_eq!(
			dirs.thumbstrip_file(id, Uuid::from_u128(7), 42),
			volume_dir
				.join("thumbstrips")
				.join(Uuid::from_u128(7).simple().to_string())
				.join("42.png")
		);

		// Resolution creates nothing; creation is explicit.
		assert!(!source_dir.exists());
		assert!(!volume_dir.exists());
		dirs.create_source_dir(id).unwrap();
		dirs.create_volume_dir(id).unwrap();
		assert!(source_dir.exists());
		assert!(volume_dir.exists());
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
