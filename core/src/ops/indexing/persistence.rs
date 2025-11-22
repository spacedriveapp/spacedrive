//! Persistence abstraction layer for indexing operations
//!
//! This module provides a unified interface for storing indexing results
//! either persistently in the database or ephemerally in memory.

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
	job::{EphemeralContentIdentity, EphemeralIndex},
	state::{DirEntry, EntryKind},
	PathResolver,
};

/// Abstraction for storing indexing results
#[async_trait::async_trait]
pub trait IndexPersistence: Send + Sync {
	/// Store an entry and return its ID
	async fn store_entry(
		&self,
		entry: &DirEntry,
		location_id: Option<i32>,
		location_root_path: &Path,
	) -> JobResult<i32>;

	/// Store content identity and link to entry
	async fn store_content_identity(
		&self,
		entry_id: i32,
		path: &Path,
		cas_id: String,
	) -> JobResult<()>;

	/// Get existing entries for change detection, scoped to the indexing path
	async fn get_existing_entries(
		&self,
		indexing_path: &Path,
	) -> JobResult<
		HashMap<std::path::PathBuf, (i32, Option<u64>, Option<std::time::SystemTime>, u64)>,
	>;

	/// Update an existing entry
	async fn update_entry(&self, entry_id: i32, entry: &DirEntry) -> JobResult<()>;

	/// Check if this persistence layer supports operations
	fn is_persistent(&self) -> bool;
}

/// Database-backed persistence implementation
pub struct DatabasePersistence<'a> {
	ctx: &'a JobContext<'a>,
	device_id: i32,
	location_root_entry_id: Option<i32>, // The root entry ID of the location being indexed
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

		// CRITICAL FIX: Do NOT clone the cache!
		// The previous clone-modify-write pattern caused cache corruption:
		// 1. Thread A clones cache, processes entry, writes back
		// 2. Thread B clones cache (stale snapshot), processes entry, writes back
		// 3. Thread B's write overwrites Thread A's updates -> lost updates
		// 4. Worse: concurrent HashMap mutations could cause data corruption
		//
		// Instead, we manage the cache directly with proper locking.
		// We look up the parent, then create the entry, then cache it.
		// All cache operations are protected by the RwLock.

		// Find parent entry ID with proper locking
		let parent_id = if let Some(parent_path) = entry.path.parent() {
			// Try cache first (read lock)
			let cached_parent = {
				let cache = self.entry_id_cache.read().await;
				cache.get(parent_path).copied()
			};

			if let Some(id) = cached_parent {
				Some(id)
			} else {
				// Not in cache, check database (no lock held during async DB query)
				let parent_path_str = parent_path.to_string_lossy().to_string();
				if let Ok(Some(dir_path_record)) = entities::directory_paths::Entity::find()
					.filter(entities::directory_paths::Column::Path.eq(&parent_path_str))
					.one(self.ctx.library_db())
					.await
				{
					// Found in database, cache it (write lock)
					let mut cache = self.entry_id_cache.write().await;
					cache.insert(parent_path.to_path_buf(), dir_path_record.entry_id);
					Some(dir_path_record.entry_id)
				} else {
					// Parent truly not found
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

		// Now create the entry using the old implementation (not EntryProcessor)
		// We can't easily use EntryProcessor without IndexerState, and creating
		// IndexerState with clone causes the bug we're trying to fix.
		// TODO: Refactor EntryProcessor to work without full IndexerState

		// For now, inline the entry creation logic with our properly-locked cache
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

		// Cache the entry ID for potential children (write lock)
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

		// Use the library ID from the context
		let library_id = self.ctx.library().id();

		// Delegate to existing implementation with the library_id
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

		// If we don't have a location root entry ID, we can't find existing entries
		let location_root_entry_id = match self.location_root_entry_id {
			Some(id) => id,
			None => return Ok(HashMap::new()),
		};

		// Query descendants of the indexing path
		let indexing_path_str = indexing_path.to_string_lossy().to_string();
		let indexing_path_entry_id = if let Ok(Some(dir_record)) = directory_paths::Entity::find()
			.filter(directory_paths::Column::Path.eq(&indexing_path_str))
			.one(self.ctx.library_db())
			.await
		{
			// Indexing path exists in DB - use its entry ID
			dir_record.entry_id
		} else {
			// This is safe because if the path doesn't exist, there are no descendants to delete
			location_root_entry_id
		};

		// Get all descendants of the indexing path
		let descendant_ids = entry_closure::Entity::find()
			.filter(entry_closure::Column::AncestorId.eq(indexing_path_entry_id))
			.all(self.ctx.library_db())
			.await
			.map_err(|e| JobError::execution(format!("Failed to query closure table: {}", e)))?
			.into_iter()
			.map(|ec| ec.descendant_id)
			.collect::<Vec<i32>>();

		// Add the indexing path entry itself
		let mut all_entry_ids = vec![indexing_path_entry_id];
		all_entry_ids.extend(descendant_ids);

		// Fetch all entries (chunked to avoid SQLite variable limit)
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
			// Build full path for the entry using PathResolver
			let full_path = PathResolver::get_full_path(self.ctx.library_db(), entry.id)
				.await
				.unwrap_or_else(|_| PathBuf::from(&entry.name));

			// Convert timestamp to SystemTime for comparison
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

		// Delegate to existing implementation
		EntryProcessor::update_entry(self.ctx, entry_id, entry).await
	}

	fn is_persistent(&self) -> bool {
		true
	}
}

/// In-memory ephemeral persistence implementation
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

		// Extract full metadata
		// Note: Ephemeral persistence uses direct filesystem (None backend)
		let metadata = EntryProcessor::extract_metadata(&entry.path, None)
			.await
			.map_err(|e| JobError::execution(format!("Failed to extract metadata: {}", e)))?;

		// Generate a stable UUID for this ephemeral entry
		let entry_id = self.get_next_id().await;
		let entry_uuid = Uuid::new_v4();

		// Store in ephemeral index
		{
			let mut index = self.index.write().await;
			index.add_entry(entry.path.clone(), metadata.clone());

			// Update stats
			match entry.kind {
				EntryKind::File => index.stats.files += 1,
				EntryKind::Directory => index.stats.dirs += 1,
				EntryKind::Symlink => index.stats.symlinks += 1,
			}
			index.stats.bytes += entry.size;
		}

		// Emit ResourceChanged event for UI
		if let Some(event_bus) = &self.event_bus {
			use crate::device::get_current_device_slug;
			use crate::domain::addressing::SdPath;
			use crate::domain::file::File;
			use crate::infra::event::{Event, ResourceMetadata};

			// Build SdPath - for ephemeral indexing, we use Physical paths
			let device_slug = get_current_device_slug();

			let sd_path = SdPath::Physical {
				device_slug: device_slug.clone(),
				path: entry.path.clone(),
			};

			// Build File domain object from ephemeral data
			let file = File::from_ephemeral(entry_uuid, &metadata, sd_path);

			// Emit event with path metadata for filtering
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
		path: &Path,
		cas_id: String,
	) -> JobResult<()> {
		// Get file size
		let file_size = tokio::fs::symlink_metadata(path)
			.await
			.map(|m| m.len())
			.unwrap_or(0);

		// Detect file type using the file type registry
		let registry = FileTypeRegistry::default();
		let mime_type = if let Ok(result) = registry.identify(path).await {
			result.file_type.primary_mime_type().map(|s| s.to_string())
		} else {
			None
		};

		let content_identity = EphemeralContentIdentity {
			cas_id: cas_id.clone(),
			mime_type,
			file_size,
			entry_count: 1,
		};

		{
			let mut index = self.index.write().await;
			index.add_content_identity(cas_id, content_identity);
		}

		Ok(())
	}

	async fn get_existing_entries(
		&self,
		_indexing_path: &Path,
	) -> JobResult<
		HashMap<std::path::PathBuf, (i32, Option<u64>, Option<std::time::SystemTime>, u64)>,
	> {
		// Ephemeral persistence doesn't support change detection
		Ok(HashMap::new())
	}

	async fn update_entry(&self, _entry_id: i32, _entry: &DirEntry) -> JobResult<()> {
		// Updates not needed for ephemeral storage
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
