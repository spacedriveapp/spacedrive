//! Unified ephemeral writer for both watcher and indexer pipelines.
//!
//! This module consolidates `EphemeralChangeHandler` and `EphemeralPersistence`
//! into a single implementation that serves both the file watcher and indexer job.
//! Both pipelines share the same entry storage logic, UUID generation, and event
//! emission, eliminating code duplication.
//!
use crate::infra::event::{Event, EventBus};
use crate::infra::job::prelude::{JobError, JobResult};
use crate::ops::indexing::change_detection::handler::{build_dir_entry, ChangeHandler};
use crate::ops::indexing::change_detection::types::{ChangeType, EntryRef};
use crate::ops::indexing::database_storage::{is_hidden_path, EntryMetadata};
use crate::ops::indexing::persistence::IndexPersistence;
use crate::ops::indexing::state::{DirEntry, EntryKind};

use super::store::SourceStore;
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
/// (for the indexer job pipeline), which makes it the one place a walk and a
/// watcher converge — and therefore the place a filesystem source fans out to
/// its two destinations. Both pipelines share:
/// - The same `EphemeralIndex` storage
/// - UUID generation and tracking
/// - Event emission for UI updates
/// - Entry ID generation
///
/// The arena is written inline because a browse is waiting on it. The store is
/// handed the same observation through a queue, so nothing on the read path
/// ever waits on SQLite. A partition with no store (scratch, or a cache with
/// no directory to write to) simply skips the second write.
pub struct MemoryAdapter {
	index: Arc<RwLock<EphemeralIndex>>,
	event_bus: Arc<EventBus>,
	root_path: PathBuf,
	store: Option<Arc<SourceStore>>,
	next_id: AtomicI32,
}

impl MemoryAdapter {
	pub fn new(
		index: Arc<RwLock<EphemeralIndex>>,
		event_bus: Arc<EventBus>,
		root_path: PathBuf,
		store: Option<Arc<SourceStore>>,
	) -> Self {
		Self {
			index,
			event_bus,
			root_path,
			store,
			next_id: AtomicI32::new(1),
		}
	}

	/// The identity to write into the arena for `metadata`.
	///
	/// The store's ledger assigns it, so the arena and the record table name a
	/// file the same way. A partition with no store has no ledger to ask, and
	/// mints one that lives as long as the session does.
	async fn identify(&self, metadata: &EntryMetadata) -> Uuid {
		match &self.store {
			Some(store) => store
				.identify_one(metadata)
				.await
				.unwrap_or_else(Uuid::now_v7),
			None => Uuid::now_v7(),
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
		let (content_kind, _entry_uuid) = {
			let mut index = self.index.write().await;
			index
				.add_entry(path.to_path_buf(), uuid, metadata.clone())
				.map_err(|e| anyhow::anyhow!("Failed to add entry to ephemeral index: {}", e))?
		};

		let entry_id = self.next_id();
		Ok((entry_id, content_kind))
	}

	/// Tell clients about an entry at `path`.
	///
	/// `vacated` is the directory the entry just left, when it left one. A
	/// client renders a directory listing, so a move invalidates two of them
	/// and an event naming only the destination leaves the file showing in its
	/// old home until something else refreshes it.
	async fn emit_resource_changed(
		&self,
		uuid: Uuid,
		path: &Path,
		metadata: &EntryMetadata,
		content_kind: crate::domain::ContentKind,
		vacated: Option<&Path>,
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

		let device = file.sd_path.device_slug().unwrap_or("local").to_string();
		let affected_paths = path
			.parent()
			.into_iter()
			.chain(vacated)
			.map(|p| SdPath::Physical {
				device_slug: device.clone(),
				path: p.to_path_buf(),
			})
			.collect();

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
		let entry_metadata = EntryMetadata::from(metadata.clone());
		let entry_uuid = self.identify(&entry_metadata).await;

		tracing::debug!(
			"MemoryAdapter::create() called for path: {}",
			metadata.path.display()
		);

		let (entry_id, content_kind) = self
			.add_entry_internal(&metadata.path, entry_uuid, entry_metadata.clone())
			.await?;

		if let Some(content_kind) = content_kind {
			tracing::debug!(
				"Emitting ResourceChanged for ephemeral create: {} (content_kind: {:?})",
				metadata.path.display(),
				content_kind
			);
			self.emit_resource_changed(
				entry_uuid,
				&metadata.path,
				&entry_metadata,
				content_kind,
				None,
			)
			.await;
		} else {
			tracing::warn!(
				"No content_kind for ephemeral entry, skipping ResourceChanged: {}",
				metadata.path.display()
			);
		}

		Ok(EntryRef {
			id: entry_id,
			uuid: Some(entry_uuid),
			path: metadata.path.clone(),
			kind: metadata.kind,
		})
	}

	async fn update(&mut self, entry: &EntryRef, metadata: &DirEntry) -> Result<()> {
		let entry_metadata = EntryMetadata::from(metadata.clone());
		let uuid = match entry.uuid {
			Some(uuid) => {
				// Already identified; the store still has to hear that the
				// bytes changed.
				if let Some(store) = &self.store {
					store.identify_one(&entry_metadata).await;
				}
				uuid
			}
			None => self.identify(&entry_metadata).await,
		};

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
		let entry_metadata = EntryMetadata::from(metadata.clone());

		// The record keeps its identity across the move, which is what every
		// assertion attached to it depends on.
		let uuid = entry.uuid.unwrap_or_else(Uuid::new_v4);
		{
			let mut index = self.index.write().await;
			index.remove_entry(old_path);
			let _ = index.add_entry(new_path.to_path_buf(), uuid, entry_metadata.clone());
		}

		// A rename carries both ends, so the store rebinds rather than
		// re-deriving identity from an inode it would have to trust.
		if let Some(store) = &self.store {
			store.renamed(old_path, &entry_metadata).await;
		}

		// The event is emitted here rather than from `emit_change_event`
		// because this is the only place that holds both paths. The generic
		// hook is handed the entry as it was *before* the move, and a lookup
		// of a path the file has already left cannot describe where it went.
		let content_kind = {
			let index = self.index.read().await;
			index.get_content_kind(&new_path.to_path_buf())
		};
		self.emit_resource_changed(
			uuid,
			new_path,
			&entry_metadata,
			content_kind,
			old_path.parent(),
		)
		.await;

		Ok(())
	}

	async fn delete(&mut self, entry: &EntryRef) -> Result<()> {
		// Get the UUID before deleting (needed for event)
		let uuid = entry.uuid;

		{
			let mut index = self.index.write().await;

			if entry.is_directory() {
				index.remove_directory_tree(&entry.path);
			} else {
				index.remove_entry(&entry.path);
			}
		}

		if let Some(store) = &self.store {
			store.lost(&entry.path, entry.is_directory()).await;
		}

		// Emit ResourceDeleted event so frontend can remove from cache
		if let Some(resource_id) = uuid {
			tracing::debug!(
				"Emitting ResourceDeleted for ephemeral delete: {} (id: {})",
				entry.path.display(),
				resource_id
			);
			self.event_bus.emit(Event::ResourceDeleted {
				resource_type: "file".to_string(),
				resource_id,
			});
		}

		Ok(())
	}

	async fn run_processors(&self, _entry: &EntryRef, _is_new: bool) -> Result<()> {
		// File processors (thumbnails, content hash) are disabled to ensure responsive, low-overhead browsing.
		Ok(())
	}

	async fn emit_change_event(&self, entry: &EntryRef, change_type: ChangeType) -> Result<()> {
		// `move_entry` already emitted, with the destination and both affected
		// listings. `entry` here still points at the vacated path, so there is
		// nothing left to learn from it.
		if matches!(change_type, ChangeType::Moved) {
			return Ok(());
		}

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
			self.emit_resource_changed(uuid, &entry.path, &entry_metadata, content_kind, None)
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

		let mut observed = Vec::new();

		{
			let mut index = self.index.write().await;

			while let Ok(Some(entry)) = entries.next_entry().await {
				let entry_path = entry.path();

				let Ok(metadata) = entry.metadata().await else {
					continue;
				};
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
					inode: DatabaseStorage::get_inode(&entry_path, &metadata),
					permissions: None,
					is_hidden: is_hidden_path(&entry_path),
				};

				observed.push((entry_path, entry_metadata));
			}
		}

		// Identity comes from the store, so the listing is resolved in one round
		// trip outside the arena lock: holding a write lock while waiting on the
		// queue would stall every reader.
		let metadata: Vec<EntryMetadata> = observed.iter().map(|(_, meta)| meta.clone()).collect();
		let identities = match &self.store {
			Some(store) => store.identify(&metadata).await,
			None => vec![None; metadata.len()],
		};

		let mut index = self.index.write().await;
		for ((path, meta), identity) in observed.into_iter().zip(identities) {
			let _ = index.add_entry(path, identity.unwrap_or_else(Uuid::now_v7), meta);
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

		let identity = self.identify(&metadata).await;

		let (entry_id, content_kind, entry_uuid) = {
			let mut index = self.index.write().await;
			// The index keeps an existing entry's uuid on duplicate paths, so
			// events always carry the identity queries will resolve.
			let (content_kind, entry_uuid) = index
				.add_entry(entry.path.clone(), identity, metadata.clone())
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

			(self.next_id(), content_kind, entry_uuid)
		};

		if let Some(content_kind) = content_kind {
			// Skip event emission for hidden files (dotfiles) to match query filtering behavior.
			// Hidden files are still indexed but won't trigger UI updates since they're
			// filtered out by default in directory_listing queries.
			// TODO: make this configurable
			let is_hidden = is_hidden_path(&entry.path);

			if !is_hidden {
				self.emit_resource_changed(entry_uuid, &entry.path, &metadata, content_kind, None)
					.await;
			}
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

		let mut writer = MemoryAdapter::new(
			index.clone(),
			event_bus,
			temp_dir.path().to_path_buf(),
			None,
		);

		let dir_entry = DirEntry {
			path: test_file.clone(),
			kind: EntryKind::File,
			size: 12,
			modified: Some(std::time::SystemTime::now()),
			created: None,
			accessed: None,
			inode: Some(12345),
			permissions: None,
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

		let writer = MemoryAdapter::new(
			index.clone(),
			event_bus,
			temp_dir.path().to_path_buf(),
			None,
		);

		let dir_entry = DirEntry {
			path: test_file.clone(),
			kind: EntryKind::File,
			size: 12,
			modified: Some(std::time::SystemTime::now()),
			created: None,
			accessed: None,
			inode: Some(12345),
			permissions: None,
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

		let writer = MemoryAdapter::new(
			index.clone(),
			event_bus,
			temp_dir.path().to_path_buf(),
			None,
		);

		let dir_entry = DirEntry {
			path: test_file.clone(),
			kind: EntryKind::File,
			size: 12,
			modified: Some(std::time::SystemTime::now()),
			created: None,
			accessed: None,
			inode: Some(12345),
			permissions: None,
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

#[cfg(test)]
mod move_tests {
	use super::*;
	use crate::ops::indexing::change_detection::types::ChangeType;
	use tempfile::TempDir;

	/// The watcher's move path, wired exactly as `change_detection::handler`
	/// wires it: `move_entry`, then `emit_change_event` with the entry as it
	/// was *before* the move.
	async fn rearrange(from: &str, to: &str) -> Option<serde_json::Value> {
		let root = TempDir::new().expect("root");
		let old = root.path().join(from);
		let new = root.path().join(to);
		for path in [&old, &new] {
			if let Some(parent) = path.parent() {
				std::fs::create_dir_all(parent).expect("parent");
			}
		}
		std::fs::write(&old, b"hello").expect("write");

		let index = Arc::new(RwLock::new(EphemeralIndex::new().expect("arena")));
		let event_bus = Arc::new(EventBus::new(1024));
		let mut subscriber = event_bus.subscribe();
		let mut adapter = MemoryAdapter::new(index, event_bus, root.path().to_path_buf(), None);

		let entry = DirEntry {
			path: old.clone(),
			kind: EntryKind::File,
			size: 5,
			modified: Some(std::time::SystemTime::now()),
			created: None,
			accessed: None,
			inode: Some(7),
			permissions: None,
		};
		let created = adapter.create(&entry, root.path()).await.expect("create");
		// Drain the create event.
		let _ =
			tokio::time::timeout(tokio::time::Duration::from_millis(50), subscriber.recv()).await;

		std::fs::rename(&old, &new).expect("rename");
		adapter
			.move_entry(&created, &old, &new, new.parent().unwrap())
			.await
			.expect("move");
		adapter
			.emit_change_event(&created, ChangeType::Moved)
			.await
			.expect("emit");

		match tokio::time::timeout(tokio::time::Duration::from_millis(100), subscriber.recv()).await
		{
			Ok(Ok(Event::ResourceChanged { resource, .. })) => Some(resource),
			_ => None,
		}
	}

	#[tokio::test]
	async fn a_rename_reaches_the_ui() {
		let resource = rearrange("draft.txt", "final.txt")
			.await
			.expect("a rename the watcher saw must reach the client");

		assert!(
			resource["sd_path"].to_string().contains("final.txt"),
			"the event has to carry where the file is now, not where it was"
		);
	}

	#[tokio::test]
	async fn a_move_between_directories_reaches_the_ui() {
		let resource = rearrange("inbox/draft.txt", "archive/draft.txt")
			.await
			.expect("a move the watcher saw must reach the client");

		assert!(resource["sd_path"].to_string().contains("archive"));
	}

	/// A client renders directory listings, so a move invalidates two of them.
	/// An event naming only the destination leaves the file showing in the
	/// folder it left.
	#[tokio::test]
	async fn a_move_invalidates_both_listings() {
		let root = TempDir::new().expect("root");
		let old = root.path().join("inbox/draft.txt");
		let new = root.path().join("archive/draft.txt");
		for path in [&old, &new] {
			std::fs::create_dir_all(path.parent().unwrap()).expect("parent");
		}
		std::fs::write(&old, b"hello").expect("write");

		let index = Arc::new(RwLock::new(EphemeralIndex::new().expect("arena")));
		let event_bus = Arc::new(EventBus::new(1024));
		let mut subscriber = event_bus.subscribe();
		let mut adapter = MemoryAdapter::new(index, event_bus, root.path().to_path_buf(), None);

		let entry = DirEntry {
			path: old.clone(),
			kind: EntryKind::File,
			size: 5,
			modified: Some(std::time::SystemTime::now()),
			created: None,
			accessed: None,
			inode: Some(7),
			permissions: None,
		};
		let created = adapter.create(&entry, root.path()).await.expect("create");
		let _ =
			tokio::time::timeout(tokio::time::Duration::from_millis(50), subscriber.recv()).await;

		std::fs::rename(&old, &new).expect("rename");
		adapter
			.move_entry(&created, &old, &new, new.parent().unwrap())
			.await
			.expect("move");

		let event =
			tokio::time::timeout(tokio::time::Duration::from_millis(100), subscriber.recv()).await;

		let Ok(Ok(Event::ResourceChanged {
			metadata: Some(metadata),
			..
		})) = event
		else {
			panic!("expected a ResourceChanged carrying affected listings");
		};

		let affected: Vec<String> = metadata
			.affected_paths
			.iter()
			.map(|p| p.to_string())
			.collect();
		assert!(
			affected.iter().any(|p| p.contains("archive")),
			"the destination listing gains the file: {affected:?}"
		);
		assert!(
			affected.iter().any(|p| p.contains("inbox")),
			"the vacated listing loses it: {affected:?}"
		);
	}
}
