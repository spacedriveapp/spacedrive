//! Integration tests for the ephemeral sidecar system
//!
//! Tests on-demand thumbnail generation for ephemeral entries, cache behavior,
//! orphan cleanup, and HTTP serving.

use sd_core::ops::indexing::ephemeral::{EphemeralIndex, EphemeralSidecarCache};
use std::collections::HashSet;
use uuid::Uuid;

#[tokio::test]
async fn test_ephemeral_sidecar_cache_lifecycle() {
	let library_id = Uuid::new_v4();
	let cache = EphemeralSidecarCache::new(library_id).expect("failed to create cache");

	let entry_uuid = Uuid::new_v4();

	// Initially no sidecars exist
	assert!(!cache.has(&entry_uuid, "thumb", "grid@1x"));

	// Insert a sidecar
	cache.insert(entry_uuid, "thumb".to_string(), "grid@1x".to_string());

	// Now it exists
	assert!(cache.has(&entry_uuid, "thumb", "grid@1x"));

	// Different variant doesn't exist
	assert!(!cache.has(&entry_uuid, "thumb", "detail@2x"));

	// Stats reflect the cache
	let stats = cache.stats();
	assert_eq!(stats.entries, 1);
	assert_eq!(stats.total_variants, 1);

	// Cleanup
	cache.clear_all().await.expect("failed to clear cache");
	assert_eq!(cache.stats().entries, 0);
}

#[tokio::test]
async fn test_ephemeral_sidecar_path_computation() {
	let library_id = Uuid::new_v4();
	let cache = EphemeralSidecarCache::new(library_id).expect("failed to create cache");
	let entry_uuid = Uuid::new_v4();

	// Compute path for thumbnail
	let thumb_path = cache.compute_path(&entry_uuid, "thumb", "grid@1x", "webp");

	// Verify path structure
	assert!(thumb_path
		.to_string_lossy()
		.contains(&format!("spacedrive-ephemeral-{}", library_id)));
	assert!(thumb_path.to_string_lossy().contains("sidecars/entry"));
	assert!(thumb_path
		.to_string_lossy()
		.contains(&entry_uuid.to_string()));
	assert!(thumb_path.to_string_lossy().contains("thumbs"));
	assert!(thumb_path.to_string_lossy().ends_with("grid@1x.webp"));

	// Test transcript (stays singular)
	let transcript_path = cache.compute_path(&entry_uuid, "transcript", "default", "txt");
	assert!(transcript_path.to_string_lossy().contains("transcript"));
	assert!(!transcript_path.to_string_lossy().contains("transcripts"));

	// Cleanup
	cache.clear_all().await.ok();
}

#[tokio::test]
async fn test_ephemeral_sidecar_scan_existing() {
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

	// Create a new cache instance and scan
	let cache2 = EphemeralSidecarCache::new(library_id).expect("failed to create cache");
	let count = cache2.scan_existing().await.expect("failed to scan");

	assert_eq!(count, 1);
	assert!(cache2.has(&entry_uuid, "thumb", "grid@1x"));

	// Cleanup
	cache2.clear_all().await.ok();
}

#[tokio::test]
async fn test_ephemeral_sidecar_orphan_cleanup() {
	let library_id = Uuid::new_v4();
	let cache = EphemeralSidecarCache::new(library_id).expect("failed to create cache");

	let valid_uuid = Uuid::new_v4();
	let orphan_uuid = Uuid::new_v4();

	// Create sidecars for both entries
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
async fn test_ephemeral_sidecar_multiple_variants() {
	let library_id = Uuid::new_v4();
	let cache = EphemeralSidecarCache::new(library_id).expect("failed to create cache");

	let entry1 = Uuid::new_v4();
	let entry2 = Uuid::new_v4();

	// Entry 1 has 2 variants
	cache.insert(entry1, "thumb".to_string(), "grid@1x".to_string());
	cache.insert(entry1, "thumb".to_string(), "detail@2x".to_string());

	// Entry 2 has 1 variant
	cache.insert(entry2, "thumb".to_string(), "grid@1x".to_string());

	let stats = cache.stats();
	assert_eq!(stats.entries, 2);
	assert_eq!(stats.total_variants, 3);

	// Verify specific lookups
	assert!(cache.has(&entry1, "thumb", "grid@1x"));
	assert!(cache.has(&entry1, "thumb", "detail@2x"));
	assert!(cache.has(&entry2, "thumb", "grid@1x"));
	assert!(!cache.has(&entry2, "thumb", "detail@2x"));

	// Cleanup
	cache.clear_all().await.ok();
}

#[tokio::test]
async fn test_ephemeral_index_entry_uuid_mapping() {
	let mut index = EphemeralIndex::new().expect("failed to create index");

	let path = std::path::PathBuf::from("/test/file.jpg");
	let uuid = Uuid::new_v4();

	let metadata = sd_core::ops::indexing::database_storage::EntryMetadata {
		kind: sd_core::ops::indexing::state::EntryKind::File,
		size: 1024,
		modified: 0,
		created: Some(0),
	};

	// Add entry
	index
		.add_entry(path.clone(), uuid, metadata)
		.expect("failed to add entry");

	// Verify UUID mapping
	assert_eq!(index.get_entry_uuid(&path), Some(uuid));
	assert_eq!(index.get_path_by_uuid(uuid), Some(path.clone()));

	// Non-existent UUID returns None
	assert_eq!(index.get_path_by_uuid(Uuid::new_v4()), None);
}

#[tokio::test]
async fn test_ephemeral_sidecar_different_kinds() {
	let library_id = Uuid::new_v4();
	let cache = EphemeralSidecarCache::new(library_id).expect("failed to create cache");
	let entry_uuid = Uuid::new_v4();

	// Add different sidecar kinds
	cache.insert(entry_uuid, "thumb".to_string(), "grid@1x".to_string());
	cache.insert(entry_uuid, "preview".to_string(), "video".to_string());
	cache.insert(entry_uuid, "transcript".to_string(), "default".to_string());

	// Verify all exist
	assert!(cache.has(&entry_uuid, "thumb", "grid@1x"));
	assert!(cache.has(&entry_uuid, "preview", "video"));
	assert!(cache.has(&entry_uuid, "transcript", "default"));

	// Verify paths have correct structure
	let thumb_path = cache.compute_path(&entry_uuid, "thumb", "grid@1x", "webp");
	let preview_path = cache.compute_path(&entry_uuid, "preview", "video", "mp4");
	let transcript_path = cache.compute_path(&entry_uuid, "transcript", "default", "txt");

	assert!(thumb_path.to_string_lossy().contains("thumbs"));
	assert!(preview_path.to_string_lossy().contains("previews"));
	assert!(transcript_path.to_string_lossy().contains("transcript"));

	// Cleanup
	cache.clear_all().await.ok();
}

#[tokio::test]
async fn test_ephemeral_sidecar_concurrent_access() {
	let library_id = Uuid::new_v4();
	let cache = std::sync::Arc::new(
		EphemeralSidecarCache::new(library_id).expect("failed to create cache"),
	);

	let mut handles = Vec::new();

	// Spawn multiple tasks inserting sidecars concurrently
	for i in 0..10 {
		let cache_clone = cache.clone();
		let handle = tokio::spawn(async move {
			let entry_uuid = Uuid::new_v4();
			cache_clone.insert(entry_uuid, "thumb".to_string(), format!("variant{}", i));
			entry_uuid
		});
		handles.push(handle);
	}

	// Wait for all tasks to complete
	let mut uuids = Vec::new();
	for handle in handles {
		let uuid = handle.await.expect("task failed");
		uuids.push(uuid);
	}

	// Verify all entries exist
	for (i, uuid) in uuids.iter().enumerate() {
		assert!(cache.has(uuid, "thumb", &format!("variant{}", i)));
	}

	assert_eq!(cache.stats().entries, 10);

	// Cleanup
	cache.clear_all().await.ok();
}

#[tokio::test]
async fn test_ephemeral_sidecar_temp_root_location() {
	let library_id = Uuid::new_v4();
	let cache = EphemeralSidecarCache::new(library_id).expect("failed to create cache");

	// Verify temp root is in system temp directory
	let temp_root = cache.temp_root();
	let system_temp = std::env::temp_dir();

	assert!(temp_root.starts_with(&system_temp));
	assert!(temp_root
		.to_string_lossy()
		.contains(&format!("spacedrive-ephemeral-{}", library_id)));
	assert!(temp_root.to_string_lossy().contains("sidecars"));

	// Cleanup
	cache.clear_all().await.ok();
}
