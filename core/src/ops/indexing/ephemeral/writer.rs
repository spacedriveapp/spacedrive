//! Unified ephemeral writer for both watcher and indexer pipelines.
//!
//! This module consolidates `EphemeralChangeHandler` and `EphemeralPersistence`
//! into a single implementation that serves both the file watcher and indexer job.
//! Both pipelines share the same entry storage logic, UUID generation, and event
//! emission, eliminating code duplication.
//!
use crate::infra::event::EventBus;
use crate::infra::job::prelude::{JobError, JobResult};
use crate::ops::indexing::change_detection::handler::{build_dir_entry, ChangeHandler};
use crate::ops::indexing::change_detection::types::{ChangeType, EntryRef};
use crate::ops::indexing::database_storage::EntryMetadata;
use crate::ops::indexing::persistence::IndexPersistence;
use crate::ops::indexing::state::{DirEntry, EntryKind};

use super::EphemeralIndex;

use anyhow::Result;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicI32, Ordering};
use std::sync::Arc;
use std::time::SystemTime;
use tokio::sync::RwLock;
use uuid::Uuid;

/// Unified writer for ephemeral (in-memory) index storage.
///
/// Implements both `ChangeHandler` (for the watcher pipeline) and `IndexPersistence`
/// (for the indexer job pipeline). Both pipelines share:
/// - The same `EphemeralIndex` storage
/// - UUID generation and tracking
/// - Event emission for UI updates
/// - Entry ID generation
pub struct MemoryAdapter {
	index: Arc<RwLock<EphemeralIndex>>,
	event_bus: Arc<EventBus>,
	root_path: PathBuf,
	next_id: AtomicI32,
}

impl MemoryAdapter {
	pub fn new(
		index: Arc<RwLock<EphemeralIndex>>,
		event_bus: Arc<EventBus>,
		root_path: PathBuf,
	) -> Self {
		Self {
			index,
			event_bus,
			root_path,
			next_id: AtomicI32::new(1),
		}
	}

	fn next_id(&self) -> i32 {
		self.next_id.fetch_add(1, Ordering::SeqCst)
	}

	/// Core write operation shared by both watcher and indexer pipelines.
	async fn add_entry_internal(
		&self,
		path: &Path,
		uuid: Uuid,
		metadata: EntryMetadata,
	) -> Result<(i32, Option<crate::domain::ContentKind>)> {
		let content_kind = {
			let mut index = self.index.write().await;
			index
				.add_entry(path.to_path_buf(), uuid, metadata.clone())
				.map_err(|e| anyhow::anyhow!("Failed to add entry to ephemeral index: {}", e))?
		};

		let entry_id = self.next_id();
		Ok((entry_id, content_kind))
	}

	async fn emit_resource_changed(
		&self,
		uuid: Uuid,
		path: &Path,
		metadata: &EntryMetadata,
		content_kind: crate::domain::ContentKind,
	) {
		use crate::device::get_current_device_slug;
		use crate::domain::addressing::SdPath;
		use crate::domain::file::File;
		use crate::infra::event::{Event, ResourceMetadata};

		let device_slug = get_current_device_slug();

		let sd_path = SdPath::Physical {
			device_slug: device_slug.clone(),
			path: path.to_path_buf(),
		};

		let mut file = File::from_ephemeral(uuid, metadata, sd_path);
		file.content_kind = content_kind;

		let parent_path = path.parent().map(|p| SdPath::Physical {
			device_slug: file.sd_path.device_slug().unwrap_or("local").to_string(),
			path: p.to_path_buf(),
		});

		let affected_paths = parent_path.into_iter().collect();

		if let Ok(resource_json) = serde_json::to_value(&file) {
			self.event_bus.emit(Event::ResourceChanged {
				resource_type: "file".to_string(),
				resource: resource_json,
				metadata: Some(ResourceMetadata {
					no_merge_fields: vec!["sd_path".to_string()],
					alternate_ids: vec![],
					affected_paths,
				}),
			});
		}
	}
}

#[async_trait::async_trait]
impl ChangeHandler for MemoryAdapter {
	async fn find_by_path(&self, path: &Path) -> Result<Option<EntryRef>> {
		let index = self.index.read().await;

		if let Some(metadata) = index.get_entry_ref(&path.to_path_buf()) {
			let uuid = index.get_entry_uuid(&path.to_path_buf());

			Ok(Some(EntryRef {
				id: 0,
				uuid,
				path: path.to_path_buf(),
				kind: metadata.kind,
			}))
		} else {
			Ok(None)
		}
	}

	async fn find_by_inode(&self, _inode: u64) -> Result<Option<EntryRef>> {
		// Inode tracking is skipped to minimize memory overhead; fall back to path-only detection.
		Ok(None)
	}

	async fn create(&mut self, metadata: &DirEntry, _parent_path: &Path) -> Result<EntryRef> {
		let entry_uuid = Uuid::new_v4();
		let entry_metadata = EntryMetadata::from(metadata.clone());

		let (entry_id, content_kind) = self
			.add_entry_internal(&metadata.path, entry_uuid, entry_metadata.clone())
			.await?;

		if let Some(content_kind) = content_kind {
			self.emit_resource_changed(entry_uuid, &metadata.path, &entry_metadata, content_kind)
				.await;
		}

		Ok(EntryRef {
			id: entry_id,
			uuid: Some(entry_uuid),
			path: metadata.path.clone(),
			kind: metadata.kind,
		})
	}

	async fn update(&mut self, entry: &EntryRef, metadata: &DirEntry) -> Result<()> {
		let uuid = entry.uuid.unwrap_or_else(Uuid::new_v4);
		let entry_metadata = EntryMetadata::from(metadata.clone());

		{
			let mut index = self.index.write().await;
			let _ = index.add_entry(metadata.path.clone(), uuid, entry_metadata);
		}

		Ok(())
	}

	async fn move_entry(
		&mut self,
		entry: &EntryRef,
		old_path: &Path,
		new_path: &Path,
		_new_parent_path: &Path,
	) -> Result<()> {
		let metadata = build_dir_entry(new_path, None).await?;

		{
			let mut index = self.index.write().await;
			index.remove_entry(old_path);

			let uuid = entry.uuid.unwrap_or_else(Uuid::new_v4);
			let entry_metadata = EntryMetadata::from(metadata.clone());
			let _ = index.add_entry(new_path.to_path_buf(), uuid, entry_metadata);
		}

		Ok(())
	}

	async fn delete(&mut self, entry: &EntryRef) -> Result<()> {
		{
			let mut index = self.index.write().await;

			if entry.is_directory() {
				index.remove_directory_tree(&entry.path);
			} else {
				index.remove_entry(&entry.path);
			}
		}

		Ok(())
	}

	async fn run_processors(&self, _entry: &EntryRef, _is_new: bool) -> Result<()> {
		// File processors (thumbnails, content hash) are disabled to ensure responsive, low-overhead browsing.
		Ok(())
	}

	async fn emit_change_event(&self, entry: &EntryRef, _change_type: ChangeType) -> Result<()> {
		let Some(uuid) = entry.uuid else {
			return Ok(());
		};

		let content_kind = {
			let index = self.index.read().await;
			index.get_content_kind(&entry.path)
		};

		let metadata = build_dir_entry(&entry.path, None).await.ok();

		if let Some(meta) = metadata {
			let entry_metadata = EntryMetadata::from(meta);
			self.emit_resource_changed(uuid, &entry.path, &entry_metadata, content_kind)
				.await;
		}

		Ok(())
	}

	async fn handle_new_directory(&self, path: &Path) -> Result<()> {
		use crate::ops::indexing::database_storage::DatabaseStorage;

		let mut entries = match tokio::fs::read_dir(path).await {
			Ok(e) => e,
			Err(e) => {
				tracing::warn!(
					"Failed to read directory {} for ephemeral indexing: {}",
					path.display(),
					e
				);
				return Ok(());
			}
		};

		let mut index = self.index.write().await;

		while let Ok(Some(entry)) = entries.next_entry().await {
			let entry_path = entry.path();

			if let Ok(metadata) = entry.metadata().await {
				let kind = if metadata.is_dir() {
					EntryKind::Directory
				} else if metadata.is_symlink() {
					EntryKind::Symlink
				} else {
					EntryKind::File
				};

				let entry_metadata = EntryMetadata {
					path: entry_path.clone(),
					kind,
					size: metadata.len(),
					modified: metadata.modified().ok(),
					accessed: metadata.accessed().ok(),
					created: metadata.created().ok(),
					inode: DatabaseStorage::get_inode(&metadata),
					permissions: None,
					is_hidden: entry_path
						.file_name()
						.and_then(|n| n.to_str())
						.map(|n| n.starts_with('.'))
						.unwrap_or(false),
				};

				let uuid = Uuid::new_v4();
				let _ = index.add_entry(entry_path, uuid, entry_metadata);
			}
		}

		Ok(())
	}
}

#[async_trait::async_trait]
impl IndexPersistence for MemoryAdapter {
	async fn store_entry(
		&self,
		entry: &DirEntry,
		_location_id: Option<i32>,
		_location_root_path: &Path,
	) -> JobResult<i32> {
		use crate::ops::indexing::database_storage::DatabaseStorage;

		let metadata = DatabaseStorage::extract_metadata(&entry.path, None)
			.await
			.map_err(|e| JobError::execution(format!("Failed to extract metadata: {}", e)))?;

		let entry_uuid = Uuid::new_v4();

		let (entry_id, content_kind) = {
			let mut index = self.index.write().await;
			let content_kind = index
				.add_entry(entry.path.clone(), entry_uuid, metadata.clone())
				.map_err(|e| {
					tracing::error!("Failed to add entry to ephemeral index: {}", e);
					JobError::execution(format!("Failed to add entry: {}", e))
				})?;

			if content_kind.is_some() {
				match entry.kind {
					EntryKind::File => index.stats.files += 1,
					EntryKind::Directory => index.stats.dirs += 1,
					EntryKind::Symlink => index.stats.symlinks += 1,
				}
				index.stats.bytes += entry.size;
			}

			(self.next_id(), content_kind)
		};

		if let Some(content_kind) = content_kind {
			self.emit_resource_changed(entry_uuid, &entry.path, &metadata, content_kind)
				.await;
		}

		Ok(entry_id)
	}

	async fn store_content_identity(
		&self,
		_entry_id: i32,
		_path: &Path,
		_cas_id: String,
	) -> JobResult<()> {
		Ok(())
	}

	async fn get_existing_entries(
		&self,
		_indexing_path: &Path,
	) -> JobResult<HashMap<PathBuf, (i32, Option<u64>, Option<SystemTime>, u64)>> {
		Ok(HashMap::new())
	}

	async fn update_entry(&self, _entry_id: i32, _entry: &DirEntry) -> JobResult<()> {
		Ok(())
	}

	fn is_persistent(&self) -> bool {
		false
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::infra::event::Event;
	use tempfile::TempDir;

	#[tokio::test]
	async fn test_ephemeral_writer_as_change_handler() {
		let temp_dir = TempDir::new().unwrap();
		let test_file = temp_dir.path().join("test.txt");
		std::fs::write(&test_file, b"test content").unwrap();

		let index = Arc::new(RwLock::new(
			EphemeralIndex::new().expect("failed to create ephemeral index"),
		));
		let event_bus = Arc::new(EventBus::new(1024));

		let mut writer =
			MemoryAdapter::new(index.clone(), event_bus, temp_dir.path().to_path_buf());

		let dir_entry = DirEntry {
			path: test_file.clone(),
			kind: EntryKind::File,
			size: 12,
			modified: Some(std::time::SystemTime::now()),
			inode: Some(12345),
		};

		let entry_ref = writer
			.create(&dir_entry, temp_dir.path())
			.await
			.expect("create should succeed");

		assert!(entry_ref.uuid.is_some());
		assert_eq!(entry_ref.path, test_file);
		assert_eq!(entry_ref.kind, EntryKind::File);

		let found = writer
			.find_by_path(&test_file)
			.await
			.expect("find should succeed");
		assert!(found.is_some());
	}

	#[tokio::test]
	async fn test_ephemeral_writer_as_index_persistence() {
		let temp_dir = TempDir::new().unwrap();
		let test_file = temp_dir.path().join("test.txt");
		std::fs::write(&test_file, b"test content").unwrap();

		let index = Arc::new(RwLock::new(
			EphemeralIndex::new().expect("failed to create ephemeral index"),
		));
		let event_bus = Arc::new(EventBus::new(1024));

		let writer = MemoryAdapter::new(index.clone(), event_bus, temp_dir.path().to_path_buf());

		let dir_entry = DirEntry {
			path: test_file.clone(),
			kind: EntryKind::File,
			size: 12,
			modified: Some(std::time::SystemTime::now()),
			inode: Some(12345),
		};

		let entry_id = writer
			.store_entry(&dir_entry, None, temp_dir.path())
			.await
			.expect("store_entry should succeed");

		assert!(entry_id > 0);
		assert!(!writer.is_persistent());

		let idx = index.read().await;
		assert!(idx.has_entry(&test_file));
	}

	#[tokio::test]
	async fn test_event_emission_consistency() {
		let temp_dir = TempDir::new().unwrap();
		let test_file = temp_dir.path().join("test.txt");
		std::fs::write(&test_file, b"test content").unwrap();

		let index = Arc::new(RwLock::new(
			EphemeralIndex::new().expect("failed to create ephemeral index"),
		));

		let event_bus = Arc::new(EventBus::new(1024));
		let mut subscriber = event_bus.subscribe();

		let writer = MemoryAdapter::new(index.clone(), event_bus, temp_dir.path().to_path_buf());

		let dir_entry = DirEntry {
			path: test_file.clone(),
			kind: EntryKind::File,
			size: 12,
			modified: Some(std::time::SystemTime::now()),
			inode: Some(12345),
		};

		writer
			.store_entry(&dir_entry, None, temp_dir.path())
			.await
			.expect("store_entry should succeed");

		let event =
			tokio::time::timeout(tokio::time::Duration::from_millis(100), subscriber.recv()).await;

		assert!(event.is_ok(), "Should receive an event");
		if let Ok(Ok(Event::ResourceChanged { resource, .. })) = event {
			let uuid = resource["id"].as_str();
			assert!(uuid.is_some(), "Event should have UUID");
		}
	}
}
