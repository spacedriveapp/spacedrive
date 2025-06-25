//! Persistence abstraction layer for indexing operations
//!
//! This module provides a unified interface for storing indexing results
//! either persistently in the database or ephemerally in memory.

use crate::{
	file_type::FileTypeRegistry,
	infrastructure::{
		database::entities,
		jobs::prelude::{JobContext, JobError, JobResult},
	},
};
use sea_orm::{ActiveModelTrait, ActiveValue::Set};
use std::{collections::HashMap, path::Path, sync::Arc};
use tokio::sync::RwLock;
use uuid::Uuid;

use super::{
	job::{EphemeralContentIdentity, EphemeralIndex},
	state::{DirEntry, EntryKind},
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
	) -> JobResult<HashMap<std::path::PathBuf, (i32, Option<u64>, Option<std::time::SystemTime>)>>;

	/// Update an existing entry
	async fn update_entry(&self, entry_id: i32, entry: &DirEntry) -> JobResult<()>;

	/// Check if this persistence layer supports operations
	fn is_persistent(&self) -> bool;
}

/// Database-backed persistence implementation
pub struct DatabasePersistence<'a> {
	ctx: &'a JobContext<'a>,
	location_id: i32,
	device_id: i32,
	entry_id_cache: Arc<RwLock<HashMap<std::path::PathBuf, i32>>>,
}

impl<'a> DatabasePersistence<'a> {
	pub fn new(ctx: &'a JobContext<'a>, location_id: i32, device_id: i32) -> Self {
		Self {
			ctx,
			location_id,
			device_id,
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

		// Calculate relative directory path from location root (without filename)
		let relative_path = if let Ok(rel_path) = entry.path.strip_prefix(location_root_path) {
			// Get parent directory relative to location root
			if let Some(parent) = rel_path.parent() {
				if parent == std::path::Path::new("") {
					String::new()
				} else {
					parent.to_string_lossy().to_string()
				}
			} else {
				String::new()
			}
		} else {
			String::new()
		};

		// Extract file extension (without dot) for files, None for directories
		let extension = match entry.kind {
			EntryKind::File => entry
				.path
				.extension()
				.and_then(|ext| ext.to_str())
				.map(|ext| ext.to_lowercase()),
			EntryKind::Directory | EntryKind::Symlink => None,
		};

		// Get file name without extension (stem)
		let name = entry
			.path
			.file_stem()
			.map(|stem| stem.to_string_lossy().to_string())
			.unwrap_or_else(|| {
				entry
					.path
					.file_name()
					.map(|n| n.to_string_lossy().to_string())
					.unwrap_or_else(|| "unknown".to_string())
			});

		// Convert timestamps
		let modified_at = entry
			.modified
			.and_then(|t| {
				chrono::DateTime::from_timestamp(
					t.duration_since(std::time::UNIX_EPOCH).ok()?.as_secs() as i64,
					0,
				)
			})
			.unwrap_or_else(|| chrono::Utc::now());

		// Determine if UUID should be assigned immediately
		// - Directories: Assign UUID immediately (no content to identify)
		// - Empty files: Assign UUID immediately (size = 0, no content to hash)
		// - Regular files: Assign UUID after content identification completes
		let should_assign_uuid = entry.kind == EntryKind::Directory || entry.size == 0;
		let entry_uuid = if should_assign_uuid {
			Some(Uuid::new_v4())
		} else {
			None // Will be assigned during content identification phase
		};

		// Create entry
		let new_entry = entities::entry::ActiveModel {
			uuid: Set(entry_uuid),
			location_id: Set(self.location_id),
			relative_path: Set(relative_path),
			name: Set(name),
			kind: Set(EntryProcessor::entry_kind_to_int(entry.kind)),
			extension: Set(extension),
			metadata_id: Set(None), // User metadata only created when user adds metadata
			content_id: Set(None),  // Will be set later if content indexing is enabled
			size: Set(entry.size as i64),
			aggregate_size: Set(0), // Will be calculated in aggregation phase
			child_count: Set(0),    // Will be calculated in aggregation phase
			file_count: Set(0),     // Will be calculated in aggregation phase
			created_at: Set(chrono::Utc::now()),
			modified_at: Set(modified_at),
			accessed_at: Set(None),
			permissions: Set(None), // TODO: Could extract from metadata
			inode: Set(entry.inode.map(|i| i as i64)),
			..Default::default()
		};

		let result = new_entry
			.insert(self.ctx.library_db())
			.await
			.map_err(|e| JobError::execution(format!("Failed to create entry: {}", e)))?;

		// Cache the entry ID for potential children
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
		EntryProcessor::link_to_content_identity(self.ctx, entry_id, path, cas_id, library_id).await
	}

	async fn get_existing_entries(
		&self,
		indexing_path: &Path,
	) -> JobResult<HashMap<std::path::PathBuf, (i32, Option<u64>, Option<std::time::SystemTime>)>>
	{
		use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};

		// Get location root to calculate relative path for the indexing scope
		let location_record = entities::location::Entity::find_by_id(self.location_id)
			.one(self.ctx.library_db())
			.await
			.map_err(|e| JobError::execution(format!("Failed to find location: {}", e)))?
			.ok_or_else(|| JobError::execution("Location not found".to_string()))?;

		// Parse the location path to determine the root
		let location_root = std::path::Path::new(&location_record.path);

		// Calculate the relative path being indexed
		let relative_indexing_path = if let Ok(rel_path) = indexing_path.strip_prefix(location_root)
		{
			if rel_path == std::path::Path::new("") {
				// Indexing entire location - use optimized query for root
				None
			} else {
				// Indexing subpath - scope the query
				Some(rel_path.to_string_lossy().to_string())
			}
		} else {
			return Err(JobError::execution(format!(
				"Indexing path {} is not within location root {}",
				indexing_path.display(),
				location_root.display()
			)));
		};

		// Build scoped query based on indexing path
		// NOTE: This query benefits from these indexes:
		// CREATE INDEX idx_entries_location_relative_path ON entries(location_id, relative_path);
		// CREATE INDEX idx_entries_location_path_prefix ON entries(location_id, relative_path varchar_pattern_ops); -- PostgreSQL
		let mut query = entities::entry::Entity::find()
			.filter(entities::entry::Column::LocationId.eq(self.location_id));

		// If indexing a subpath, filter entries to that subtree only
		if let Some(ref rel_path) = relative_indexing_path {
			// Include entries that are:
			// 1. Direct children: relative_path = rel_path
			// 2. Descendants: relative_path starts with rel_path + "/"
			query = query.filter(
				entities::entry::Column::RelativePath
					.eq(rel_path)
					.or(entities::entry::Column::RelativePath.like(format!("{}/%", rel_path)))
					.or(entities::entry::Column::RelativePath.like(format!("{}\\%", rel_path))), // Windows paths
			);
		}

		let existing_entries = query
			.all(self.ctx.library_db())
			.await
			.map_err(|e| JobError::execution(format!("Failed to query existing entries: {}", e)))?;

		let mut result = HashMap::new();

		// Log the scope for debugging
		if let Some(rel_path) = &relative_indexing_path {
			self.ctx.log(format!(
				"Loading {} existing entries for subpath: {}",
				existing_entries.len(),
				rel_path
			));
		} else {
			self.ctx.log(format!(
				"Loading {} existing entries for entire location",
				existing_entries.len()
			));
		}

		for entry in existing_entries {
			// Reconstruct full path from relative_path and name
			let full_path = if entry.relative_path.is_empty() {
				location_root.join(&entry.name)
			} else {
				location_root.join(&entry.relative_path).join(&entry.name)
			};

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
				(entry.id, entry.inode.map(|i| i as u64), modified_time),
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
}

impl EphemeralPersistence {
	pub fn new(index: Arc<RwLock<EphemeralIndex>>) -> Self {
		Self {
			index,
			next_entry_id: Arc::new(RwLock::new(1)),
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
		let metadata = EntryProcessor::extract_metadata(&entry.path)
			.await
			.map_err(|e| JobError::execution(format!("Failed to extract metadata: {}", e)))?;

		// Store in ephemeral index
		{
			let mut index = self.index.write().await;
			index.add_entry(entry.path.clone(), metadata);

			// Update stats
			match entry.kind {
				EntryKind::File => index.stats.files += 1,
				EntryKind::Directory => index.stats.dirs += 1,
				EntryKind::Symlink => index.stats.symlinks += 1,
			}
			index.stats.bytes += entry.size;
		}

		Ok(self.get_next_id().await)
	}

	async fn store_content_identity(
		&self,
		_entry_id: i32,
		path: &Path,
		cas_id: String,
	) -> JobResult<()> {
		// Get file size
		let file_size = tokio::fs::metadata(path)
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
	) -> JobResult<HashMap<std::path::PathBuf, (i32, Option<u64>, Option<std::time::SystemTime>)>>
	{
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
		location_id: i32,
		device_id: i32,
	) -> Box<dyn IndexPersistence + 'a> {
		Box::new(DatabasePersistence::new(ctx, location_id, device_id))
	}

	/// Create an ephemeral persistence instance
	pub fn ephemeral(
		index: Arc<RwLock<EphemeralIndex>>,
	) -> Box<dyn IndexPersistence + Send + Sync> {
		Box::new(EphemeralPersistence::new(index))
	}
}
