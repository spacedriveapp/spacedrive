//! # Ephemeral Sidecar Cache
//!
//! In-memory cache tracking ephemeral sidecars (thumbnails, previews, etc.) for
//! ephemeral locations. Unlike managed sidecars stored by content hash in the
//! library folder, ephemeral sidecars use entry UUIDs and live in the system
//! temp directory.
//!
//! ## Storage Structure
//!
//! ```text
//! /tmp/spacedrive-ephemeral-{library_id}/
//! └── sidecars/
//!     └── entry/
//!         └── {entry_uuid}/
//!             ├── thumbs/
//!             │   ├── grid@1x.webp
//!             │   └── detail@2x.webp
//!             ├── previews/
//!             │   └── video.mp4
//!             └── transcript/
//!                 └── audio.txt
//! ```
//!
//! ## Design Rationale
//!
//! Ephemeral sidecars are stored in temp because they're regenerable derivatives
//! of files that might not persist (user browsing a network share). The cache
//! avoids filesystem queries by tracking existence in memory, since sidecars
//! are only accessed by the current session.

use parking_lot::RwLock;
use std::{
	collections::{HashMap, HashSet},
	path::{Path, PathBuf},
};
use uuid::Uuid;

/// In-memory cache of ephemeral sidecar existence
///
/// Tracks which sidecars exist without filesystem queries. All paths use
/// entry UUIDs (from `EphemeralIndex.entry_uuids`) as identifiers since
/// ephemeral entries lack content hashes.
pub struct EphemeralSidecarCache {
	/// entry_uuid -> kind -> variant -> exists
	entries: RwLock<HashMap<Uuid, HashMap<String, HashSet<String>>>>,
	/// Temp directory root for this library
	temp_root: PathBuf,
	/// Library ID
	library_id: Uuid,
}

impl EphemeralSidecarCache {
	/// Create a new ephemeral sidecar cache
	///
	/// Creates the temp directory structure if it doesn't exist. The directory
	/// is specific to this library to prevent conflicts across libraries and
	/// enable per-library cleanup.
	pub fn new(library_id: Uuid) -> std::io::Result<Self> {
		let temp_root = std::env::temp_dir()
			.join(format!("spacedrive-ephemeral-{}", library_id))
			.join("sidecars");

		std::fs::create_dir_all(&temp_root)?;

		Ok(Self {
			entries: RwLock::new(HashMap::new()),
			temp_root,
			library_id,
		})
	}

	/// Check if a sidecar exists (in-memory, no I/O)
	///
	/// Returns true if the sidecar was previously generated and cached.
	/// Does not verify filesystem existence, trusting the cache to be
	/// authoritative for this session.
	pub fn has(&self, entry_uuid: &Uuid, kind: &str, variant: &str) -> bool {
		let entries = self.entries.read();
		entries
			.get(entry_uuid)
			.and_then(|kinds| kinds.get(kind))
			.map_or(false, |variants| variants.contains(variant))
	}

	/// Record that a sidecar was generated
	///
	/// Updates the in-memory cache to reflect a newly generated sidecar.
	/// Should be called immediately after writing the sidecar to disk.
	pub fn insert(&self, entry_uuid: Uuid, kind: String, variant: String) {
		let mut entries = self.entries.write();
		entries
			.entry(entry_uuid)
			.or_insert_with(HashMap::new)
			.entry(kind)
			.or_insert_with(HashSet::new)
			.insert(variant);
	}

	/// Get the filesystem path for a sidecar
	///
	/// Computes the path without checking existence. The kind is pluralized
	/// (e.g., "thumb" -> "thumbs") except for "transcript" which stays singular
	/// to match English grammar.
	pub fn compute_path(
		&self,
		entry_uuid: &Uuid,
		kind: &str,
		variant: &str,
		format: &str,
	) -> PathBuf {
		let kind_dir = if kind == "transcript" {
			kind.to_string()
		} else {
			format!("{}s", kind)
		};

		self.temp_root
			.join("entry")
			.join(entry_uuid.to_string())
			.join(&kind_dir)
			.join(format!("{}.{}", variant, format))
	}

	/// Get the directory for all sidecars of an entry
	///
	/// Used by listing operations to enumerate what sidecars exist for a
	/// specific entry without needing to know kinds/variants upfront.
	pub fn compute_entry_dir(&self, entry_uuid: &Uuid) -> PathBuf {
		self.temp_root.join("entry").join(entry_uuid.to_string())
	}

	/// Bootstrap: scan temp directory and populate cache
	///
	/// Called on initialization to recover ephemeral sidecars from previous
	/// sessions. The temp directory persists across app restarts, so this
	/// reuses thumbnails without regenerating them.
	pub async fn scan_existing(&self) -> std::io::Result<usize> {
		let entry_dir = self.temp_root.join("entry");

		if !tokio::fs::try_exists(&entry_dir).await? {
			return Ok(0);
		}

		let mut count = 0;
		let mut entries = self.entries.write();

		let mut read_dir = tokio::fs::read_dir(&entry_dir).await?;
		while let Some(entry_uuid_dir) = read_dir.next_entry().await? {
			let entry_uuid = match Uuid::parse_str(&entry_uuid_dir.file_name().to_string_lossy()) {
				Ok(uuid) => uuid,
				Err(_) => continue,
			};

			let mut kind_dir = tokio::fs::read_dir(entry_uuid_dir.path()).await?;
			while let Some(kind_entry) = kind_dir.next_entry().await? {
				let kind_name = kind_entry.file_name().to_string_lossy().to_string();
				let kind = kind_name.trim_end_matches('s');

				let mut variant_files = tokio::fs::read_dir(kind_entry.path()).await?;
				while let Some(variant_file) = variant_files.next_entry().await? {
					let filename = variant_file.file_name().to_string_lossy().to_string();
					if let Some((variant, _format)) = filename.rsplit_once('.') {
						entries
							.entry(entry_uuid)
							.or_insert_with(HashMap::new)
							.entry(kind.to_string())
							.or_insert_with(HashSet::new)
							.insert(variant.to_string());
						count += 1;
					}
				}
			}
		}

		Ok(count)
	}

	/// Cleanup: remove all ephemeral sidecars for this library
	///
	/// Deletes the entire temp directory tree. Called when clearing the
	/// ephemeral cache or when the library is closed. Safe to call multiple
	/// times since missing directories are ignored.
	pub async fn clear_all(&self) -> std::io::Result<usize> {
		let mut entries = self.entries.write();
		let count = entries.len();
		entries.clear();

		let root = self.temp_root.parent().unwrap();
		if tokio::fs::try_exists(root).await? {
			tokio::fs::remove_dir_all(root).await?;
		}

		Ok(count)
	}

	/// Cleanup orphaned sidecars for entries no longer in the index
	///
	/// Scans the temp directory and removes sidecars for entries that don't
	/// exist in the provided set of valid UUIDs. Called during bootstrap to
	/// clean up stale data from deleted or moved files.
	pub async fn cleanup_orphans(&self, valid_uuids: &HashSet<Uuid>) -> std::io::Result<usize> {
		let entry_dir = self.temp_root.join("entry");

		if !tokio::fs::try_exists(&entry_dir).await? {
			return Ok(0);
		}

		let mut removed = 0;
		let mut entries = self.entries.write();

		let mut read_dir = tokio::fs::read_dir(&entry_dir).await?;
		while let Some(entry_uuid_dir) = read_dir.next_entry().await? {
			if let Ok(entry_uuid) = Uuid::parse_str(&entry_uuid_dir.file_name().to_string_lossy()) {
				if !valid_uuids.contains(&entry_uuid) {
					tokio::fs::remove_dir_all(entry_uuid_dir.path()).await?;
					entries.remove(&entry_uuid);
					removed += 1;
				}
			}
		}

		Ok(removed)
	}

	/// Get the library ID this cache is for
	pub fn library_id(&self) -> Uuid {
		self.library_id
	}

	/// Get the temp root directory
	pub fn temp_root(&self) -> &Path {
		&self.temp_root
	}

	/// Get statistics about cached sidecars
	pub fn stats(&self) -> EphemeralSidecarCacheStats {
		let entries = self.entries.read();
		let total_variants = entries
			.values()
			.flat_map(|kinds| kinds.values())
			.map(|variants| variants.len())
			.sum();

		EphemeralSidecarCacheStats {
			entries: entries.len(),
			total_variants,
		}
	}
}

/// Statistics about the ephemeral sidecar cache
#[derive(Debug, Clone)]
pub struct EphemeralSidecarCacheStats {
	/// Number of entries with at least one sidecar
	pub entries: usize,
	/// Total number of sidecar variants across all entries
	pub total_variants: usize,
}

#[cfg(test)]
mod tests {
	use super::*;

	#[tokio::test]
	async fn test_create_cache() {
		let library_id = Uuid::new_v4();
		let cache = EphemeralSidecarCache::new(library_id).expect("failed to create cache");

		assert_eq!(cache.library_id(), library_id);
		assert!(cache.temp_root().exists());
	}

	#[tokio::test]
	async fn test_insert_and_has() {
		let cache = EphemeralSidecarCache::new(Uuid::new_v4()).expect("failed to create cache");
		let entry_uuid = Uuid::new_v4();

		assert!(!cache.has(&entry_uuid, "thumb", "grid@1x"));

		cache.insert(entry_uuid, "thumb".to_string(), "grid@1x".to_string());

		assert!(cache.has(&entry_uuid, "thumb", "grid@1x"));
		assert!(!cache.has(&entry_uuid, "thumb", "detail@2x"));
	}

	#[tokio::test]
	async fn test_compute_path() {
		let library_id = Uuid::new_v4();
		let cache = EphemeralSidecarCache::new(library_id).expect("failed to create cache");
		let entry_uuid = Uuid::new_v4();

		let path = cache.compute_path(&entry_uuid, "thumb", "grid@1x", "webp");

		assert!(path
			.to_string_lossy()
			.contains(&format!("spacedrive-ephemeral-{}", library_id)));
		assert!(path.to_string_lossy().contains(&entry_uuid.to_string()));
		assert!(path.to_string_lossy().contains("thumbs"));
		assert!(path.to_string_lossy().ends_with("grid@1x.webp"));
	}

	#[tokio::test]
	async fn test_compute_path_transcript() {
		let cache = EphemeralSidecarCache::new(Uuid::new_v4()).expect("failed to create cache");
		let entry_uuid = Uuid::new_v4();

		let path = cache.compute_path(&entry_uuid, "transcript", "default", "txt");

		assert!(path.to_string_lossy().contains("transcript"));
		assert!(!path.to_string_lossy().contains("transcripts"));
	}

	#[tokio::test]
	async fn test_clear_all() {
		let cache = EphemeralSidecarCache::new(Uuid::new_v4()).expect("failed to create cache");
		let entry_uuid = Uuid::new_v4();

		cache.insert(entry_uuid, "thumb".to_string(), "grid@1x".to_string());
		assert_eq!(cache.stats().entries, 1);

		let cleared = cache.clear_all().await.expect("failed to clear");
		assert_eq!(cleared, 1);
		assert_eq!(cache.stats().entries, 0);
	}

	#[tokio::test]
	async fn test_scan_existing() {
		let library_id = Uuid::new_v4();
		let cache = EphemeralSidecarCache::new(library_id).expect("failed to create cache");
		let entry_uuid = Uuid::new_v4();

		// Manually create a sidecar file
		let path = cache.compute_path(&entry_uuid, "thumb", "grid@1x", "webp");
		tokio::fs::create_dir_all(path.parent().unwrap())
			.await
			.expect("failed to create dirs");
		tokio::fs::write(&path, b"fake thumbnail")
			.await
			.expect("failed to write file");

		// Create a new cache to test scanning
		let cache2 = EphemeralSidecarCache::new(library_id).expect("failed to create cache");
		let count = cache2.scan_existing().await.expect("failed to scan");

		assert_eq!(count, 1);
		assert!(cache2.has(&entry_uuid, "thumb", "grid@1x"));

		// Cleanup
		cache2.clear_all().await.ok();
	}

	#[tokio::test]
	async fn test_cleanup_orphans() {
		let library_id = Uuid::new_v4();
		let cache = EphemeralSidecarCache::new(library_id).expect("failed to create cache");

		let valid_uuid = Uuid::new_v4();
		let orphan_uuid = Uuid::new_v4();

		// Create sidecars for both
		for uuid in &[valid_uuid, orphan_uuid] {
			let path = cache.compute_path(uuid, "thumb", "grid@1x", "webp");
			tokio::fs::create_dir_all(path.parent().unwrap())
				.await
				.expect("failed to create dirs");
			tokio::fs::write(&path, b"fake thumbnail")
				.await
				.expect("failed to write file");
			cache.insert(*uuid, "thumb".to_string(), "grid@1x".to_string());
		}

		assert_eq!(cache.stats().entries, 2);

		// Cleanup orphans (only keep valid_uuid)
		let mut valid_set = HashSet::new();
		valid_set.insert(valid_uuid);
		let removed = cache
			.cleanup_orphans(&valid_set)
			.await
			.expect("failed to cleanup");

		assert_eq!(removed, 1);
		assert_eq!(cache.stats().entries, 1);
		assert!(cache.has(&valid_uuid, "thumb", "grid@1x"));
		assert!(!cache.has(&orphan_uuid, "thumb", "grid@1x"));

		// Cleanup
		cache.clear_all().await.ok();
	}

	#[tokio::test]
	async fn test_stats() {
		let cache = EphemeralSidecarCache::new(Uuid::new_v4()).expect("failed to create cache");
		let entry1 = Uuid::new_v4();
		let entry2 = Uuid::new_v4();

		cache.insert(entry1, "thumb".to_string(), "grid@1x".to_string());
		cache.insert(entry1, "thumb".to_string(), "detail@2x".to_string());
		cache.insert(entry2, "thumb".to_string(), "grid@1x".to_string());

		let stats = cache.stats();
		assert_eq!(stats.entries, 2);
		assert_eq!(stats.total_variants, 3);
	}
}
