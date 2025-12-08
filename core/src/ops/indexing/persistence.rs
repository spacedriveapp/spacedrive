//! # Persistence Abstraction for Indexing
//!
//! `core::ops::indexing::persistence` provides a unified interface for storing
//! indexing results either persistently in the database or ephemerally in memory.
//! This abstraction allows the same indexing pipeline to work for both managed
//! locations (database-backed) and ephemeral browsing (memory-only).
//!

use crate::{
	filetype::FileTypeRegistry,
	infra::{
		db::entities::{self, directory_paths, entry_closure},
		job::prelude::{JobContext, JobError, JobResult},
	},
};
use sea_orm::{
	ActiveModelTrait, ActiveValue::Set, ColumnTrait, Condition, ConnectionTrait, DbBackend,
	EntityTrait, JoinType, QueryFilter, QuerySelect, RelationTrait, Statement, TransactionTrait,
};
use std::{
	collections::HashMap,
	path::{Path, PathBuf},
	sync::Arc,
};
use tokio::sync::RwLock;
use uuid::Uuid;

use super::{
	job::EphemeralIndex,
	state::{DirEntry, EntryKind},
	PathResolver,
};

/// Unified storage interface for persistent and ephemeral indexing.
///
/// Implementations handle either database writes (DatabasePersistence) or
/// in-memory storage (EphemeralPersistence). The indexing pipeline calls
/// these methods without knowing which backend is active.
#[async_trait::async_trait]
pub trait IndexPersistence: Send + Sync {
	/// Stores an entry and returns its ID for linking content identities.
	///
	/// For database persistence, this creates an `entry` row and updates the closure table.
	/// For ephemeral persistence, this adds the entry to the in-memory index and emits
	/// a ResourceChanged event for immediate UI updates.
	async fn store_entry(
		&self,
		entry: &DirEntry,
		location_id: Option<i32>,
		location_root_path: &Path,
	) -> JobResult<i32>;

	/// Links a content identity (hash) to an entry.
	///
	/// For database persistence, this creates or finds a `content_identity` row and updates
	/// the entry's `content_id` foreign key. For ephemeral persistence, this is a no-op since
	/// in-memory indexes don't track content deduplication across sessions.
	async fn store_content_identity(
		&self,
		entry_id: i32,
		path: &Path,
		cas_id: String,
	) -> JobResult<()>;

	/// Retrieves existing entries under a path for change detection.
	///
	/// Returns a map of path -> (entry_id, inode, modified_time, size) for all entries
	/// under the indexing path. Change detection compares this snapshot against the
	/// current filesystem to identify additions, modifications, and deletions. Ephemeral
	/// persistence returns an empty map since it doesn't support incremental indexing.
	async fn get_existing_entries(
		&self,
		indexing_path: &Path,
	) -> JobResult<
		HashMap<std::path::PathBuf, (i32, Option<u64>, Option<std::time::SystemTime>, u64)>,
	>;

	async fn update_entry(&self, entry_id: i32, entry: &DirEntry) -> JobResult<()>;

	/// Returns true for database persistence, false for ephemeral.
	///
	/// Used by the indexing pipeline to determine whether to perform expensive operations
	/// like change detection (database only) or content hashing (database only).
	fn is_persistent(&self) -> bool;
}

/// Database-backed persistence with RwLock-protected entry ID cache.
///
/// This implementation writes all entries to the database and manages a cache of
/// path -> entry_id mappings for fast parent lookups during hierarchy construction.
/// The cache uses RwLock instead of clone-modify-write to prevent race conditions
/// where concurrent cache updates overwrite each other.
pub struct DatabasePersistence<'a> {
	ctx: &'a JobContext<'a>,
	device_id: i32,
	location_root_entry_id: Option<i32>,
	entry_id_cache: Arc<RwLock<HashMap<std::path::PathBuf, i32>>>,
}

impl<'a> DatabasePersistence<'a> {
	pub fn new(
		ctx: &'a JobContext<'a>,
		device_id: i32,
		location_root_entry_id: Option<i32>,
	) -> Self {
		Self {
			ctx,
			device_id,
			location_root_entry_id,
			entry_id_cache: Arc::new(RwLock::new(HashMap::new())),
		}
	}
}

#[async_trait::async_trait]
impl<'a> IndexPersistence for DatabasePersistence<'a> {
	async fn store_entry(
		&self,
		entry: &DirEntry,
		_location_id: Option<i32>,
		location_root_path: &Path,
	) -> JobResult<i32> {
		use super::entry::EntryProcessor;

		// Cache lookups use RwLock read/write operations instead of clone-modify-write.
		let parent_id = if let Some(parent_path) = entry.path.parent() {
			let cached_parent = {
				let cache = self.entry_id_cache.read().await;
				cache.get(parent_path).copied()
			};

			if let Some(id) = cached_parent {
				Some(id)
			} else {
				let parent_path_str = parent_path.to_string_lossy().to_string();
				if let Ok(Some(dir_path_record)) = entities::directory_paths::Entity::find()
					.filter(entities::directory_paths::Column::Path.eq(&parent_path_str))
					.one(self.ctx.library_db())
					.await
				{
					let mut cache = self.entry_id_cache.write().await;
					cache.insert(parent_path.to_path_buf(), dir_path_record.entry_id);
					Some(dir_path_record.entry_id)
				} else {
					tracing::warn!(
						"Parent not found for {}: {}",
						entry.path.display(),
						parent_path.display()
					);
					None
				}
			}
		} else {
			None
		};

		use entities::entry_closure;

		let extension = match entry.kind {
			EntryKind::File => entry
				.path
				.extension()
				.and_then(|ext| ext.to_str())
				.map(|ext| ext.to_lowercase()),
			EntryKind::Directory | EntryKind::Symlink => None,
		};

		let name = match entry.kind {
			EntryKind::File => entry
				.path
				.file_stem()
				.map(|stem| stem.to_string_lossy().to_string())
				.unwrap_or_else(|| {
					entry
						.path
						.file_name()
						.map(|n| n.to_string_lossy().to_string())
						.unwrap_or_else(|| "unknown".to_string())
				}),
			EntryKind::Directory | EntryKind::Symlink => entry
				.path
				.file_name()
				.map(|n| n.to_string_lossy().to_string())
				.unwrap_or_else(|| "unknown".to_string()),
		};

		let modified_at = entry
			.modified
			.and_then(|t| {
				chrono::DateTime::from_timestamp(
					t.duration_since(std::time::UNIX_EPOCH).ok()?.as_secs() as i64,
					0,
				)
			})
			.unwrap_or_else(|| chrono::Utc::now());

		let entry_uuid = Some(Uuid::new_v4());

		let new_entry = entities::entry::ActiveModel {
			uuid: Set(entry_uuid),
			name: Set(name.clone()),
			kind: Set(EntryProcessor::entry_kind_to_int(entry.kind)),
			extension: Set(extension),
			metadata_id: Set(None),
			content_id: Set(None),
			size: Set(entry.size as i64),
			aggregate_size: Set(0),
			child_count: Set(0),
			file_count: Set(0),
			created_at: Set(chrono::Utc::now()),
			modified_at: Set(modified_at),
			accessed_at: Set(None),
			permissions: Set(None),
			inode: Set(entry.inode.map(|i| i as i64)),
			parent_id: Set(parent_id),
			..Default::default()
		};

		let txn = self
			.ctx
			.library_db()
			.begin()
			.await
			.map_err(|e| JobError::execution(format!("Failed to begin transaction: {}", e)))?;

		let result = new_entry
			.insert(&txn)
			.await
			.map_err(|e| JobError::execution(format!("Failed to create entry: {}", e)))?;

		let self_closure = entry_closure::ActiveModel {
			ancestor_id: Set(result.id),
			descendant_id: Set(result.id),
			depth: Set(0),
			..Default::default()
		};
		self_closure
			.insert(&txn)
			.await
			.map_err(|e| JobError::execution(format!("Failed to insert self-closure: {}", e)))?;

		if let Some(parent_id) = parent_id {
			txn.execute(Statement::from_sql_and_values(
				DbBackend::Sqlite,
				"INSERT INTO entry_closure (ancestor_id, descendant_id, depth) \
				 SELECT ancestor_id, ?, depth + 1 \
				 FROM entry_closure \
				 WHERE descendant_id = ?",
				vec![result.id.into(), parent_id.into()],
			))
			.await
			.map_err(|e| {
				JobError::execution(format!("Failed to populate ancestor closures: {}", e))
			})?;
		}

		if entry.kind == EntryKind::Directory {
			let absolute_path = entry.path.to_string_lossy().to_string();
			let dir_path_entry = entities::directory_paths::ActiveModel {
				entry_id: Set(result.id),
				path: Set(absolute_path),
				..Default::default()
			};
			dir_path_entry.insert(&txn).await.map_err(|e| {
				JobError::execution(format!("Failed to insert directory path: {}", e))
			})?;
		}

		txn.commit()
			.await
			.map_err(|e| JobError::execution(format!("Failed to commit transaction: {}", e)))?;

		tracing::info!(
			"ENTRY_SYNC: About to sync entry name={} uuid={:?}",
			result.name,
			result.uuid
		);
		if let Err(e) = self
			.ctx
			.library()
			.sync_model_with_db(
				&result,
				crate::infra::sync::ChangeType::Insert,
				self.ctx.library_db(),
			)
			.await
		{
			tracing::warn!(
				"ENTRY_SYNC: Failed to sync entry {}: {}",
				result
					.uuid
					.map(|u| u.to_string())
					.unwrap_or_else(|| "no-uuid".to_string()),
				e
			);
		} else {
			tracing::info!(
				"ENTRY_SYNC: Successfully synced entry name={} uuid={:?}",
				result.name,
				result.uuid
			);
		}

		{
			let mut cache = self.entry_id_cache.write().await;
			cache.insert(entry.path.clone(), result.id);
		}

		Ok(result.id)
	}

	async fn store_content_identity(
		&self,
		entry_id: i32,
		path: &Path,
		cas_id: String,
	) -> JobResult<()> {
		use super::entry::EntryProcessor;

		let library_id = self.ctx.library().id();

		EntryProcessor::link_to_content_identity(self.ctx, entry_id, path, cas_id, library_id)
			.await
			.map(|_| ())
	}

	async fn get_existing_entries(
		&self,
		indexing_path: &Path,
	) -> JobResult<
		HashMap<std::path::PathBuf, (i32, Option<u64>, Option<std::time::SystemTime>, u64)>,
	> {
		use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};

		let location_root_entry_id = match self.location_root_entry_id {
			Some(id) => id,
			None => return Ok(HashMap::new()),
		};

		let indexing_path_str = indexing_path.to_string_lossy().to_string();
		let indexing_path_entry_id = if let Ok(Some(dir_record)) = directory_paths::Entity::find()
			.filter(directory_paths::Column::Path.eq(&indexing_path_str))
			.one(self.ctx.library_db())
			.await
		{
			dir_record.entry_id
		} else {
			location_root_entry_id
		};

		let descendant_ids = entry_closure::Entity::find()
			.filter(entry_closure::Column::AncestorId.eq(indexing_path_entry_id))
			.all(self.ctx.library_db())
			.await
			.map_err(|e| JobError::execution(format!("Failed to query closure table: {}", e)))?
			.into_iter()
			.map(|ec| ec.descendant_id)
			.collect::<Vec<i32>>();

		let mut all_entry_ids = vec![indexing_path_entry_id];
		all_entry_ids.extend(descendant_ids);

		// Chunk queries to stay under SQLite's 999 variable limit.
		let mut existing_entries: Vec<entities::entry::Model> = Vec::new();
		let chunk_size: usize = 900;
		for chunk in all_entry_ids.chunks(chunk_size) {
			let mut batch = entities::entry::Entity::find()
				.filter(entities::entry::Column::Id.is_in(chunk.to_vec()))
				.all(self.ctx.library_db())
				.await
				.map_err(|e| {
					JobError::execution(format!("Failed to query existing entries: {}", e))
				})?;
			existing_entries.append(&mut batch);
		}

		let mut result = HashMap::new();

		self.ctx.log(format!(
			"Loading {} existing entries",
			existing_entries.len()
		));

		for entry in existing_entries {
			let full_path = PathResolver::get_full_path(self.ctx.library_db(), entry.id)
				.await
				.unwrap_or_else(|_| PathBuf::from(&entry.name));

			let modified_time =
				entry
					.modified_at
					.timestamp()
					.try_into()
					.ok()
					.and_then(|secs: u64| {
						std::time::UNIX_EPOCH.checked_add(std::time::Duration::from_secs(secs))
					});

			result.insert(
				full_path,
				(
					entry.id,
					entry.inode.map(|i| i as u64),
					modified_time,
					entry.size as u64,
				),
			);
		}

		Ok(result)
	}

	async fn update_entry(&self, entry_id: i32, entry: &DirEntry) -> JobResult<()> {
		use super::entry::EntryProcessor;

		EntryProcessor::update_entry(self.ctx, entry_id, entry).await
	}

	fn is_persistent(&self) -> bool {
		true
	}
}

/// In-memory ephemeral persistence for browsing unmanaged paths.
///
/// Stores entries in an `EphemeralIndex` (memory-only) and emits ResourceChanged
/// events for immediate UI updates.
pub struct EphemeralPersistence {
	index: Arc<RwLock<EphemeralIndex>>,
	next_entry_id: Arc<RwLock<i32>>,
	event_bus: Option<Arc<crate::infra::event::EventBus>>,
	root_path: PathBuf,
}

impl EphemeralPersistence {
	pub fn new(
		index: Arc<RwLock<EphemeralIndex>>,
		event_bus: Option<Arc<crate::infra::event::EventBus>>,
		root_path: PathBuf,
	) -> Self {
		Self {
			index,
			next_entry_id: Arc::new(RwLock::new(1)),
			event_bus,
			root_path,
		}
	}

	async fn get_next_id(&self) -> i32 {
		let mut id = self.next_entry_id.write().await;
		let current = *id;
		*id += 1;
		current
	}
}

#[async_trait::async_trait]
impl IndexPersistence for EphemeralPersistence {
	async fn store_entry(
		&self,
		entry: &DirEntry,
		_location_id: Option<i32>,
		_location_root_path: &Path,
	) -> JobResult<i32> {
		use super::entry::EntryProcessor;

		let metadata = EntryProcessor::extract_metadata(&entry.path, None)
			.await
			.map_err(|e| JobError::execution(format!("Failed to extract metadata: {}", e)))?;

		let entry_id = self.get_next_id().await;
		let entry_uuid = Uuid::new_v4();

		// add_entry returns Ok(Some(content_kind)) if added, Ok(None) if duplicate path.
		let content_kind = {
			let mut index = self.index.write().await;
			let result = index
				.add_entry(entry.path.clone(), entry_uuid, metadata.clone())
				.map_err(|e| {
					tracing::error!("Failed to add entry to ephemeral index: {}", e);
					e
				})?;

			if result.is_some() {
				match entry.kind {
					EntryKind::File => index.stats.files += 1,
					EntryKind::Directory => index.stats.dirs += 1,
					EntryKind::Symlink => index.stats.symlinks += 1,
				}
				index.stats.bytes += entry.size;
			}
			result
		};

		let Some(content_kind) = content_kind else {
			return Ok(entry_id);
		};

		if let Some(event_bus) = &self.event_bus {
			use crate::device::get_current_device_slug;
			use crate::domain::addressing::SdPath;
			use crate::domain::file::File;
			use crate::infra::event::{Event, ResourceMetadata};

			let device_slug = get_current_device_slug();

			let sd_path = SdPath::Physical {
				device_slug: device_slug.clone(),
				path: entry.path.clone(),
			};

			let mut file = File::from_ephemeral(entry_uuid, &metadata, sd_path);
			file.content_kind = content_kind;

			let parent_path = entry.path.parent().map(|p| SdPath::Physical {
				device_slug: file.sd_path.device_slug().unwrap_or("local").to_string(),
				path: p.to_path_buf(),
			});

			let affected_paths = if let Some(parent) = parent_path {
				vec![parent]
			} else {
				vec![]
			};

			if let Ok(resource_json) = serde_json::to_value(&file) {
				event_bus.emit(Event::ResourceChanged {
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
	) -> JobResult<
		HashMap<std::path::PathBuf, (i32, Option<u64>, Option<std::time::SystemTime>, u64)>,
	> {
		Ok(HashMap::new())
	}

	async fn update_entry(&self, _entry_id: i32, _entry: &DirEntry) -> JobResult<()> {
		Ok(())
	}

	fn is_persistent(&self) -> bool {
		false
	}
}

/// Factory for creating appropriate persistence implementations
pub struct PersistenceFactory;

impl PersistenceFactory {
	/// Create a database persistence instance
	pub fn database<'a>(
		ctx: &'a JobContext<'a>,
		device_id: i32,
		location_root_entry_id: Option<i32>,
	) -> Box<dyn IndexPersistence + 'a> {
		Box::new(DatabasePersistence::new(
			ctx,
			device_id,
			location_root_entry_id,
		))
	}

	/// Create an ephemeral persistence instance
	pub fn ephemeral(
		index: Arc<RwLock<EphemeralIndex>>,
		event_bus: Option<Arc<crate::infra::event::EventBus>>,
		root_path: PathBuf,
	) -> Box<dyn IndexPersistence + Send + Sync> {
		Box::new(EphemeralPersistence::new(index, event_bus, root_path))
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::infra::event::Event;
	use crate::ops::indexing::state::{DirEntry, EntryKind};
	use std::sync::Mutex;
	use tempfile::TempDir;

	#[tokio::test]
	async fn test_ephemeral_uuid_consistency() {
		// Create temp directory for test
		let temp_dir = TempDir::new().unwrap();
		let test_file = temp_dir.path().join("test.txt");
		std::fs::write(&test_file, b"test content").unwrap();

		// Create ephemeral index
		let index = Arc::new(RwLock::new(
			EphemeralIndex::new().expect("failed to create ephemeral index"),
		));

		// Create event collector
		let collected_events = Arc::new(Mutex::new(Vec::new()));
		let events_clone = collected_events.clone();

		// Create mock event bus that collects events
		let event_bus = Arc::new(crate::infra::event::EventBus::new());
		let _subscription = event_bus.subscribe(move |event| {
			if let Event::ResourceChanged { resource, .. } = event {
				events_clone.lock().unwrap().push(resource.clone());
			}
		});

		// Create ephemeral persistence
		let persistence = EphemeralPersistence::new(
			index.clone(),
			Some(event_bus),
			temp_dir.path().to_path_buf(),
		);

		// Store entry (processing phase)
		let dir_entry = DirEntry {
			path: test_file.clone(),
			kind: EntryKind::File,
			size: 12,
			modified: Some(std::time::SystemTime::now()),
			inode: Some(12345),
		};

		let entry_id = persistence
			.store_entry(&dir_entry, None, temp_dir.path())
			.await
			.unwrap();

		// Store content identity (content phase)
		let cas_id = "test_hash_123".to_string();
		persistence
			.store_content_identity(entry_id, &test_file, cas_id)
			.await
			.unwrap();

		// Give events time to propagate
		tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

		// Collect all events
		let events = collected_events.lock().unwrap();

		// Should have 2 events: one from store_entry, one from store_content_identity
		assert_eq!(
			events.len(),
			2,
			"Expected 2 ResourceChanged events (processing + content phases)"
		);

		// Extract UUIDs from both events
		let uuid1 = events[0]["id"]
			.as_str()
			.expect("First event should have UUID");
		let uuid2 = events[1]["id"]
			.as_str()
			.expect("Second event should have UUID");

		// CRITICAL: Both events must have the same UUID for the same file
		assert_eq!(
			uuid1, uuid2,
			"UUID mismatch! Processing phase emitted UUID {} but content phase emitted UUID {}. \
			 These should be identical so the UI can match the events.",
			uuid1, uuid2
		);

		// Verify the second event has content_identity
		assert!(
			events[1]["content_identity"].is_object(),
			"Second event should include content_identity"
		);
	}
}
