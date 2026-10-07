//! The identity a source store carries with it.
//!
//! A store's records, content evidence and assertions are only useful to a
//! library that knows which source they describe. The registry row knows,
//! but the row lives in `library.db` and the store can be elsewhere: on the
//! drive it describes, or left behind in the data directory after the source
//! was removed from the library. `source.json` beside `data.db` is the store's
//! own statement of what it is, so re-adding the same scope finds the catalog
//! it already has instead of minting a fresh identity over an empty store.
//!
//! The descriptor is evidence, not authority. Adopting one requires that it
//! was written by this library for this volume and this path within it; a
//! descriptor copied from another library or another drive is ignored and a
//! new source starts beside it. Mutable processing intent lives in the
//! registry row and the assertion layer; the descriptor mirrors the settings
//! for inspection and is rewritten whenever they change.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::sources::{SourceConfig, SourceRecord};
use crate::infra::db::entities::source::FILESYSTEM_DATA_TYPE;

/// The descriptor's file name within a store directory.
pub const DESCRIPTOR_FILE: &str = "source.json";

/// Bumped when a field changes meaning. A newer descriptor is never adopted
/// by an older build, which cannot judge its binding.
pub const DESCRIPTOR_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceDescriptor {
	pub version: u32,
	pub source_id: Uuid,
	/// The library that registered the source. Adoption is scoped to it.
	pub library_id: Uuid,
	pub data_type: String,
	pub name: String,
	/// The volume the source is anchored to, and its path within that
	/// volume; the absolute root when there is no volume.
	pub volume_uuid: Option<Uuid>,
	pub relative_root: String,
	pub config: SourceConfig,
	pub created_at: chrono::DateTime<chrono::Utc>,
}

impl SourceDescriptor {
	pub fn for_record(record: &SourceRecord, library_id: Uuid) -> Self {
		Self {
			version: DESCRIPTOR_VERSION,
			source_id: record.id,
			library_id,
			data_type: FILESYSTEM_DATA_TYPE.to_string(),
			name: record.name.clone(),
			volume_uuid: record.volume_uuid,
			relative_root: record.relative_root.clone(),
			config: record.config.clone(),
			created_at: record.created_at,
		}
	}

	/// Whether this descriptor describes `binding` for `library_id`: the
	/// same library, the same volume and the same path within it. A
	/// descriptor from another library or another drive fails, whatever its
	/// path says, because the path alone does not prove the store is of
	/// these files.
	pub fn binds(&self, library_id: Uuid, volume_uuid: Option<Uuid>, relative_root: &str) -> bool {
		self.version <= DESCRIPTOR_VERSION
			&& self.data_type == FILESYSTEM_DATA_TYPE
			&& self.library_id == library_id
			&& self.volume_uuid == volume_uuid
			&& self.relative_root == relative_root
	}

	/// Write the descriptor beside the store, replacing it atomically so a
	/// crash mid-write leaves the previous descriptor rather than half of
	/// this one.
	pub async fn write(&self, store_dir: &Path) -> std::io::Result<()> {
		tokio::fs::create_dir_all(store_dir).await?;
		let json = serde_json::to_vec_pretty(self).map_err(std::io::Error::other)?;
		let path = store_dir.join(DESCRIPTOR_FILE);
		let tmp = store_dir.join(format!("{DESCRIPTOR_FILE}.tmp"));
		tokio::fs::write(&tmp, json).await?;
		tokio::fs::rename(&tmp, &path).await
	}

	/// Read the descriptor in a store directory, or `None` when there is
	/// none or it cannot be parsed. Unreadable is treated as absent: a store
	/// that cannot say what it is cannot be adopted.
	pub async fn read(store_dir: &Path) -> Option<Self> {
		let bytes = tokio::fs::read(store_dir.join(DESCRIPTOR_FILE))
			.await
			.ok()?;
		serde_json::from_slice(&bytes).ok()
	}

	/// The source id of the store under `stores_dir` whose descriptor binds
	/// to this library, volume and path, and the directory it was found in.
	/// The newest registration wins when several bind, which happens when a
	/// source was removed and re-added with its catalog deleted in between.
	pub async fn find_bound(
		stores_dir: &Path,
		library_id: Uuid,
		volume_uuid: Option<Uuid>,
		relative_root: &str,
	) -> Option<(Uuid, PathBuf)> {
		let mut entries = tokio::fs::read_dir(stores_dir).await.ok()?;
		let mut best: Option<(Self, PathBuf)> = None;
		while let Ok(Some(entry)) = entries.next_entry().await {
			let dir = entry.path();
			let Some(descriptor) = Self::read(&dir).await else {
				continue;
			};
			if !descriptor.binds(library_id, volume_uuid, relative_root) {
				continue;
			}
			if best
				.as_ref()
				.is_none_or(|(current, _)| descriptor.created_at > current.created_at)
			{
				best = Some((descriptor, dir));
			}
		}
		best.map(|(descriptor, dir)| (descriptor.source_id, dir))
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use chrono::Utc;

	fn record(id: u128, volume: Option<u128>, relative_root: &str) -> SourceRecord {
		SourceRecord {
			id: Uuid::from_u128(id),
			name: "Photos".to_string(),
			root: PathBuf::from("/Volumes/Archive").join(relative_root),
			relative_root: relative_root.to_string(),
			volume_uuid: volume.map(Uuid::from_u128),
			created_at: Utc::now(),
			last_seen_at: Utc::now(),
			record_count: None,
			directory_count: None,
			total_bytes: None,
			content_count: None,
			unique_bytes: None,
			config: SourceConfig::default(),
		}
	}

	/// A descriptor binds the store to one library, one volume and one path.
	/// Any of the three differing is a different source, however familiar
	/// the path looks.
	#[tokio::test]
	async fn a_descriptor_binds_to_library_volume_and_path() {
		let library = Uuid::from_u128(100);
		let dir = tempfile::tempdir().unwrap();
		let store = dir.path().join("abc");
		SourceDescriptor::for_record(&record(1, Some(7), "Photos"), library)
			.write(&store)
			.await
			.unwrap();

		let read = SourceDescriptor::read(&store).await.unwrap();
		assert_eq!(read.source_id, Uuid::from_u128(1));
		assert!(read.binds(library, Some(Uuid::from_u128(7)), "Photos"));
		assert!(!read.binds(Uuid::from_u128(101), Some(Uuid::from_u128(7)), "Photos"));
		assert!(!read.binds(library, Some(Uuid::from_u128(8)), "Photos"));
		assert!(!read.binds(library, None, "Photos"));
		assert!(!read.binds(library, Some(Uuid::from_u128(7)), "Photos/2024"));
	}

	/// Scanning a stores directory finds the bound store among unrelated
	/// ones, and ignores a directory with no descriptor or a broken one.
	#[tokio::test]
	async fn find_bound_picks_the_store_of_this_scope() {
		let library = Uuid::from_u128(100);
		let dir = tempfile::tempdir().unwrap();
		SourceDescriptor::for_record(&record(1, Some(7), "Photos"), library)
			.write(&dir.path().join("one"))
			.await
			.unwrap();
		SourceDescriptor::for_record(&record(2, Some(7), "Video"), library)
			.write(&dir.path().join("two"))
			.await
			.unwrap();
		std::fs::create_dir_all(dir.path().join("three")).unwrap();
		std::fs::write(dir.path().join("three").join(DESCRIPTOR_FILE), b"{").unwrap();

		let found =
			SourceDescriptor::find_bound(dir.path(), library, Some(Uuid::from_u128(7)), "Video")
				.await
				.unwrap();
		assert_eq!(found.0, Uuid::from_u128(2));
		assert_eq!(found.1, dir.path().join("two"));
		assert!(SourceDescriptor::find_bound(
			dir.path(),
			library,
			Some(Uuid::from_u128(7)),
			"Music"
		)
		.await
		.is_none());
		assert!(SourceDescriptor::find_bound(
			dir.path().join("missing").as_path(),
			library,
			None,
			""
		)
		.await
		.is_none());
	}
}
