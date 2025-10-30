//! Entry processing and metadata extraction

use super::ctx::IndexingCtx;
use super::path_resolver::PathResolver;
use super::state::{DirEntry, EntryKind, IndexerState};
use crate::infra::job::prelude::{JobContext, JobError};
use crate::{
	filetype::FileTypeRegistry,
	infra::db::entities::{self, directory_paths, entry_closure},
};
use sea_orm::{
	ActiveModelTrait, ActiveValue::Set, ColumnTrait, ConnectionTrait, DatabaseTransaction,
	DbBackend, EntityTrait, IntoActiveModel, QueryFilter, QuerySelect, Statement, TransactionTrait,
};
use std::path::{Path, PathBuf};
use uuid::Uuid;

/// Metadata about a file system entry
#[derive(Debug, Clone)]
pub struct EntryMetadata {
	pub path: PathBuf,
	pub kind: EntryKind,
	pub size: u64,
	pub modified: Option<std::time::SystemTime>,
	pub accessed: Option<std::time::SystemTime>,
	pub created: Option<std::time::SystemTime>,
	pub inode: Option<u64>,
	pub permissions: Option<u32>,
	pub is_hidden: bool,
}

impl From<DirEntry> for EntryMetadata {
	fn from(entry: DirEntry) -> Self {
		Self {
			path: entry.path.clone(),
			kind: entry.kind,
			size: entry.size,
			modified: entry.modified,
			accessed: None,
			created: None,
			inode: entry.inode,
			permissions: None,
			is_hidden: entry
				.path
				.file_name()
				.and_then(|n| n.to_str())
				.map(|n| n.starts_with('.'))
				.unwrap_or(false),
		}
	}
}

/// Handles entry creation and updates in the database
pub struct EntryProcessor;

/// Result of content identity linking (for batch sync)
pub struct ContentLinkResult {
	pub content_identity: entities::content_identity::Model,
	pub entry: entities::entry::Model,
	pub is_new_content: bool,
}

impl EntryProcessor {
	/// Get platform-specific inode
	#[cfg(unix)]
	pub fn get_inode(metadata: &std::fs::Metadata) -> Option<u64> {
		use std::os::unix::fs::MetadataExt;
		Some(metadata.ino())
	}

	#[cfg(windows)]
	pub fn get_inode(metadata: &std::fs::Metadata) -> Option<u64> {
		// Windows doesn't have inodes, but we can use file index
		use std::os::windows::fs::MetadataExt;
		Some(metadata.file_index().unwrap_or(0))
	}

	#[cfg(not(any(unix, windows)))]
	pub fn get_inode(_metadata: &std::fs::Metadata) -> Option<u64> {
		None
	}

	/// Extract detailed metadata from a path
	///
	/// Uses the provided VolumeBackend if available, otherwise falls back to direct filesystem access.
	pub async fn extract_metadata(
		path: &Path,
		backend: Option<&std::sync::Arc<dyn crate::volume::VolumeBackend>>,
	) -> Result<EntryMetadata, std::io::Error> {
		// Use backend if available, otherwise fall back to local filesystem
		if let Some(backend) = backend {
			let raw = backend
				.metadata(path)
				.await
				.map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;

			Ok(EntryMetadata {
				path: path.to_path_buf(),
				kind: raw.kind,
				size: raw.size,
				modified: raw.modified,
				accessed: raw.accessed,
				created: raw.created,
				inode: raw.inode,
				permissions: raw.permissions,
				is_hidden: path
					.file_name()
					.and_then(|n| n.to_str())
					.map(|n| n.starts_with('.'))
					.unwrap_or(false),
			})
		} else {
			// Fallback to direct filesystem access
			let metadata = tokio::fs::symlink_metadata(path).await?;

			let kind = if metadata.is_dir() {
				EntryKind::Directory
			} else if metadata.is_symlink() {
				EntryKind::Symlink
			} else {
				EntryKind::File
			};

			let inode = Self::get_inode(&metadata);

			#[cfg(unix)]
			let permissions = {
				use std::os::unix::fs::MetadataExt;
				Some(metadata.mode())
			};

			#[cfg(not(unix))]
			let permissions = None;

			Ok(EntryMetadata {
				path: path.to_path_buf(),
				kind,
				size: metadata.len(),
				modified: metadata.modified().ok(),
				accessed: metadata.accessed().ok(),
				created: metadata.created().ok(),
				inode,
				permissions,
				is_hidden: path
					.file_name()
					.and_then(|n| n.to_str())
					.map(|n| n.starts_with('.'))
					.unwrap_or(false),
			})
		}
	}

	/// Create an entry record in the database using a provided connection/transaction
	/// and collect related rows for bulk insertion by the caller.
	pub async fn create_entry_in_conn<C: ConnectionTrait>(
		state: &mut IndexerState,
		ctx: &impl IndexingCtx,
		entry: &DirEntry,
		device_id: i32,
		location_root_path: &Path,
		conn: &C,
		out_self_closures: &mut Vec<entry_closure::ActiveModel>,
		out_dir_paths: &mut Vec<directory_paths::ActiveModel>,
	) -> Result<entities::entry::Model, JobError> {
		// Extract file extension (without dot) for files, None for directories
		let extension = match entry.kind {
			EntryKind::File => entry
				.path
				.extension()
				.and_then(|ext| ext.to_str())
				.map(|ext| ext.to_lowercase()),
			EntryKind::Directory | EntryKind::Symlink => None,
		};

		// Get file/directory name
		// For files: use stem (name without extension)
		// For directories: use full name (including .app, etc.)
		let name = match entry.kind {
			EntryKind::File => {
				// For files, use stem (without extension)
				entry
					.path
					.file_stem()
					.map(|stem| stem.to_string_lossy().to_string())
					.unwrap_or_else(|| {
						entry
							.path
							.file_name()
							.map(|n| n.to_string_lossy().to_string())
							.unwrap_or_else(|| "unknown".to_string())
					})
			}
			EntryKind::Directory | EntryKind::Symlink => {
				// For directories and symlinks, use full name
				entry
					.path
					.file_name()
					.map(|n| n.to_string_lossy().to_string())
					.unwrap_or_else(|| "unknown".to_string())
			}
		};

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

		// Find parent entry ID
		let parent_id = if let Some(parent_path) = entry.path.parent() {
			// First check the cache
			if let Some(id) = state.entry_id_cache.get(parent_path).copied() {
				Some(id)
			} else {
				// If not in cache, try to find it in the database
				// This handles cases where parent was created in a previous run
				let parent_path_str = parent_path.to_string_lossy().to_string();
				if let Ok(Some(dir_path_record)) = entities::directory_paths::Entity::find()
					.filter(entities::directory_paths::Column::Path.eq(&parent_path_str))
					.one(ctx.library_db())
					.await
				{
					// Found parent in database, cache it
					state
						.entry_id_cache
						.insert(parent_path.to_path_buf(), dir_path_record.entry_id);
					Some(dir_path_record.entry_id)
				} else {
					// Parent not found - this shouldn't happen with proper sorting
					ctx.log(format!(
						"WARNING: Parent not found for {}: {}",
						entry.path.display(),
						parent_path.display()
					));
					None
				}
			}
		} else {
			None
		};

		// Create entry
		let now = chrono::Utc::now();
		let new_entry = entities::entry::ActiveModel {
			uuid: Set(entry_uuid),
			name: Set(name.clone()),
			kind: Set(Self::entry_kind_to_int(entry.kind)),
			extension: Set(extension),
			metadata_id: Set(None), // User metadata only created when user adds metadata
			content_id: Set(None),  // Will be set later during content identification phase
			size: Set(entry.size as i64),
			aggregate_size: Set(0), // Will be calculated in aggregation phase
			child_count: Set(0),    // Will be calculated in aggregation phase
			file_count: Set(0),     // Will be calculated in aggregation phase
			created_at: Set(now),
			modified_at: Set(modified_at),
			accessed_at: Set(None),
			indexed_at: Set(Some(now)), // Record when we indexed this entry
			permissions: Set(None),     // TODO: Could extract from metadata
			inode: Set(entry.inode.map(|i| i as i64)),
			parent_id: Set(parent_id),
			..Default::default()
		};

		// Insert the entry
		let result = new_entry
			.insert(conn)
			.await
			.map_err(|e| JobError::execution(format!("Failed to create entry: {}", e)))?;

		// Populate closure table
		// First, insert self-reference
		let self_closure = entry_closure::ActiveModel {
			ancestor_id: Set(result.id),
			descendant_id: Set(result.id),
			depth: Set(0),
			..Default::default()
		};
		out_self_closures.push(self_closure);

		// If there's a parent, copy all parent's ancestors
		if let Some(parent_id) = parent_id {
			// Insert closure entries for all ancestors
			conn.execute_unprepared(&format!(
				"INSERT INTO entry_closure (ancestor_id, descendant_id, depth) \
				 SELECT ancestor_id, {}, depth + 1 \
				 FROM entry_closure \
				 WHERE descendant_id = {}",
				result.id, parent_id
			))
			.await
			.map_err(|e| {
				JobError::execution(format!("Failed to populate ancestor closures: {}", e))
			})?;
		}

		// If this is a directory, populate the directory_paths table
		if entry.kind == EntryKind::Directory {
			// Use the absolute path from the DirEntry which contains the full filesystem path
			let absolute_path = entry.path.to_string_lossy().to_string();

			// Insert into directory_paths table
			let dir_path_entry = directory_paths::ActiveModel {
				entry_id: Set(result.id),
				path: Set(absolute_path),
				..Default::default()
			};
			out_dir_paths.push(dir_path_entry);
		}

		// Cache the entry ID for potential children
		state.entry_id_cache.insert(entry.path.clone(), result.id);

		Ok(result)
	}

	/// Create an entry, starting and committing its own transaction (single insert)
	pub async fn create_entry(
		state: &mut IndexerState,
		ctx: &impl IndexingCtx,
		entry: &DirEntry,
		device_id: i32,
		location_root_path: &Path,
	) -> Result<i32, JobError> {
		let txn = ctx
			.library_db()
			.begin()
			.await
			.map_err(|e| JobError::execution(format!("Failed to begin transaction: {}", e)))?;

		let mut self_closures: Vec<entry_closure::ActiveModel> = Vec::new();
		let mut dir_paths: Vec<directory_paths::ActiveModel> = Vec::new();
		let result = Self::create_entry_in_conn(
			state,
			ctx,
			entry,
			device_id,
			location_root_path,
			&txn,
			&mut self_closures,
			&mut dir_paths,
		)
		.await;

		let entry_model = match result {
			Ok(model) => model,
			Err(e) => {
				let _ = txn.rollback().await;
				return Err(e);
			}
		};

		if !self_closures.is_empty() {
			entry_closure::Entity::insert_many(self_closures)
				.exec(&txn)
				.await
				.map_err(|e| {
					JobError::execution(format!("Failed to bulk insert self-closures: {}", e))
				})?;
		}
		if !dir_paths.is_empty() {
			directory_paths::Entity::insert_many(dir_paths)
				.exec(&txn)
				.await
				.map_err(|e| {
					JobError::execution(format!("Failed to bulk insert directory paths: {}", e))
				})?;
		}
		txn.commit()
			.await
			.map_err(|e| JobError::execution(format!("Failed to commit transaction: {}", e)))?;

		// Sync entry to other devices
		if let Some(library) = ctx.library() {
			tracing::info!(
				"ENTRY_SYNC: About to sync entry name={} uuid={:?}",
				entry_model.name,
				entry_model.uuid
			);
			if let Err(e) = library
				.sync_model_with_db(&entry_model, crate::infra::sync::ChangeType::Insert, ctx.library_db())
				.await
			{
				tracing::warn!(
					"ENTRY_SYNC: Failed to sync entry {}: {}",
					entry_model.uuid.map(|u| u.to_string()).unwrap_or_else(|| "no-uuid".to_string()),
					e
				);
			} else {
				tracing::info!(
					"ENTRY_SYNC: Successfully synced entry name={} uuid={:?}",
					entry_model.name,
					entry_model.uuid
				);
			}
		}

		Ok(entry_model.id)
	}

	/// Update an existing entry
	pub async fn update_entry(
		ctx: &impl IndexingCtx,
		entry_id: i32,
		entry: &DirEntry,
	) -> Result<(), JobError> {
		let db_entry = entities::entry::Entity::find_by_id(entry_id)
			.one(ctx.library_db())
			.await
			.map_err(|e| JobError::execution(format!("Failed to find entry: {}", e)))?
			.ok_or_else(|| JobError::execution("Entry not found for update".to_string()))?;

		let mut entry_active: entities::entry::ActiveModel = db_entry.into();

		// Update modifiable fields
		entry_active.size = Set(entry.size as i64);
		if let Some(modified) = entry.modified {
			if let Some(timestamp) = chrono::DateTime::from_timestamp(
				modified
					.duration_since(std::time::UNIX_EPOCH)
					.ok()
					.map(|d| d.as_secs() as i64)
					.unwrap_or(0),
				0,
			) {
				entry_active.modified_at = Set(timestamp);
			}
		}

		if let Some(inode) = entry.inode {
			entry_active.inode = Set(Some(inode as i64));
		}

		entry_active
			.update(ctx.library_db())
			.await
			.map_err(|e| JobError::execution(format!("Failed to update entry: {}", e)))?;

		Ok(())
	}

	/// Handle entry move operation with closure table updates (creates own transaction)
	pub async fn move_entry(
		state: &mut IndexerState,
		ctx: &impl IndexingCtx,
		entry_id: i32,
		old_path: &Path,
		new_path: &Path,
		location_root_path: &Path,
	) -> Result<(), JobError> {
		// Begin transaction for atomic move operation
		let txn = ctx
			.library_db()
			.begin()
			.await
			.map_err(|e| JobError::execution(format!("Failed to begin transaction: {}", e)))?;

		let result = Self::move_entry_in_conn(
			state,
			ctx,
			entry_id,
			old_path,
			new_path,
			location_root_path,
			&txn,
		)
		.await;

		match result {
			Ok(()) => {
				txn.commit().await.map_err(|e| {
					JobError::execution(format!("Failed to commit move transaction: {}", e))
				})?;
				Ok(())
			}
			Err(e) => {
				let _ = txn.rollback().await;
				Err(e)
			}
		}
	}

	/// Handle entry move operation within existing transaction
	pub async fn move_entry_in_conn(
		state: &mut IndexerState,
		ctx: &impl IndexingCtx,
		entry_id: i32,
		old_path: &Path,
		new_path: &Path,
		location_root_path: &Path,
		txn: &DatabaseTransaction,
	) -> Result<(), JobError> {
		// Get the entry
		let db_entry = entities::entry::Entity::find_by_id(entry_id)
			.one(txn)
			.await
			.map_err(|e| JobError::execution(format!("Failed to find entry: {}", e)))?
			.ok_or_else(|| JobError::execution("Entry not found for move".to_string()))?;

		let is_directory = db_entry.kind == Self::entry_kind_to_int(EntryKind::Directory);
		let mut entry_active: entities::entry::ActiveModel = db_entry.into();

		// Find new parent entry ID
		let new_parent_id = if let Some(parent_path) = new_path.parent() {
			state.entry_id_cache.get(parent_path).copied()
		} else {
			None
		};

		// Update entry fields
		entry_active.parent_id = Set(new_parent_id);

		// Extract new name if it changed
		let mut new_name_value = None;
		if let Some(new_name) = new_path.file_stem() {
			let name_string = new_name.to_string_lossy().to_string();
			new_name_value = Some(name_string.clone());
			entry_active.name = Set(name_string);
		}

		// Save the updated entry
		entry_active
			.update(txn)
			.await
			.map_err(|e| JobError::execution(format!("Failed to update entry: {}", e)))?;

		// Update closure table for the move operation
		// Step 1: Delete all ancestor relationships for the moved subtree (except internal relationships)
		txn.execute_unprepared(&format!(
			"DELETE FROM entry_closure \
			 WHERE descendant_id IN (SELECT descendant_id FROM entry_closure WHERE ancestor_id = {}) \
			 AND ancestor_id NOT IN (SELECT descendant_id FROM entry_closure WHERE ancestor_id = {})",
			entry_id, entry_id
		))
		.await
		.map_err(|e| JobError::execution(format!("Failed to disconnect subtree: {}", e)))?;

		// Step 2: If there's a new parent, reconnect the subtree
		if let Some(new_parent_id) = new_parent_id {
			// Connect moved subtree to new parent
			txn.execute_unprepared(&format!(
				"INSERT INTO entry_closure (ancestor_id, descendant_id, depth) \
				 SELECT p.ancestor_id, c.descendant_id, p.depth + c.depth + 1 \
				 FROM entry_closure p, entry_closure c \
				 WHERE p.descendant_id = {} AND c.ancestor_id = {}",
				new_parent_id, entry_id
			))
			.await
			.map_err(|e| JobError::execution(format!("Failed to reconnect subtree: {}", e)))?;
		}

		// If this is a directory, update its path in directory_paths table
		if is_directory {
			// Get the new name from what we saved earlier
			let new_name = new_name_value.unwrap_or_else(|| {
				// If name didn't change, get it from the path
				new_path
					.file_name()
					.and_then(|n| n.to_str())
					.unwrap_or("unknown")
					.to_string()
			});

			// Build the new path
			let new_directory_path =
				PathResolver::build_directory_path(txn, new_parent_id, &new_name)
					.await
					.map_err(|e| {
						JobError::execution(format!("Failed to build new directory path: {}", e))
					})?;

			// Get the old path for descendant updates
			let old_directory_path = PathResolver::get_directory_path(txn, entry_id)
				.await
				.map_err(|e| {
					JobError::execution(format!("Failed to get old directory path: {}", e))
				})?;

			// Update the directory's own path
			let mut dir_path_active = directory_paths::Entity::find_by_id(entry_id)
				.one(txn)
				.await
				.map_err(|e| JobError::execution(format!("Failed to find directory path: {}", e)))?
				.ok_or_else(|| JobError::execution("Directory path not found".to_string()))?
				.into_active_model();
			dir_path_active.path = Set(new_directory_path.clone());
			dir_path_active.update(txn).await.map_err(|e| {
				JobError::execution(format!("Failed to update directory path: {}", e))
			})?;

			// Update descendant directory paths within the same transaction
			// Note: This is done synchronously within the batch transaction for consistency
			if let Err(e) = PathResolver::update_descendant_paths(
				txn,
				entry_id,
				&old_directory_path,
				&new_directory_path,
			)
			.await
			{
				tracing::error!("Failed to update descendant paths: {}", e);
			}
		}

		// Update cache
		state.entry_id_cache.remove(old_path);
		state
			.entry_id_cache
			.insert(new_path.to_path_buf(), entry_id);

		Ok(())
	}

	/// Convert EntryKind to integer for database storage
	pub fn entry_kind_to_int(kind: EntryKind) -> i32 {
		match kind {
			EntryKind::File => 0,
			EntryKind::Directory => 1,
			EntryKind::Symlink => 2,
		}
	}

	/// Create or find content identity and link to entry with deterministic UUID
	/// This method implements the content identification phase logic
	/// Returns models for batch syncing (caller responsible for sync)
	pub async fn link_to_content_identity(
		ctx: &impl IndexingCtx,
		entry_id: i32,
		path: &Path,
		content_hash: String,
		library_id: Uuid,
	) -> Result<ContentLinkResult, JobError> {
		// Check if content identity already exists by content_hash
		let existing = entities::content_identity::Entity::find()
			.filter(entities::content_identity::Column::ContentHash.eq(&content_hash))
			.one(ctx.library_db())
			.await
			.map_err(|e| JobError::execution(format!("Failed to query content identity: {}", e)))?;

		let (content_model, is_new_content) = if let Some(existing) = existing {
			// Increment entry count for existing content
			let mut existing_active: entities::content_identity::ActiveModel = existing.into();
			existing_active.entry_count = Set(existing_active.entry_count.unwrap() + 1);
			existing_active.last_verified_at = Set(chrono::Utc::now());

			let updated = existing_active
				.update(ctx.library_db())
				.await
				.map_err(|e| {
					JobError::execution(format!("Failed to update content identity: {}", e))
				})?;

			(updated, false)
		} else {
			// Create new content identity with deterministic UUID (ready for sync)
			let file_size = tokio::fs::symlink_metadata(path)
				.await
				.map(|m| m.len() as i64)
				.unwrap_or(0);

			// Generate deterministic UUID from content_hash + library_id
			let deterministic_uuid = {
				const LIBRARY_NAMESPACE: uuid::Uuid = uuid::Uuid::from_bytes([
					0x6b, 0xa7, 0xb8, 0x10, 0x9d, 0xad, 0x11, 0xd1, 0x80, 0xb4, 0x00, 0xc0, 0x4f,
					0xd4, 0x30, 0xc8,
				]);
				// We use v5 to ensure the UUID is deterministic and unique within the library
				let namespace = uuid::Uuid::new_v5(&LIBRARY_NAMESPACE, library_id.as_bytes());
				uuid::Uuid::new_v5(&namespace, content_hash.as_bytes())
			};

			// Detect file type using the file type registry
			let registry = FileTypeRegistry::default();
			let file_type_result = registry.identify(path).await;

			let (kind_id, mime_type_id) = match file_type_result {
				Ok(result) => {
					// Get content kind ID directly from the enum
					let kind_id = result.file_type.category as i32;

					// Handle MIME type - upsert if found
					let mime_type_id = if let Some(mime_str) = result.file_type.primary_mime_type()
					{
						// Check if MIME type already exists
						let existing = entities::mime_type::Entity::find()
							.filter(entities::mime_type::Column::MimeType.eq(mime_str))
							.one(ctx.library_db())
							.await
							.map_err(|e| {
								JobError::execution(format!("Failed to query mime type: {}", e))
							})?;

						match existing {
							Some(mime_record) => Some(mime_record.id),
							None => {
								// Create new MIME type entry
								let new_mime = entities::mime_type::ActiveModel {
									uuid: Set(Uuid::new_v4()),
									mime_type: Set(mime_str.to_string()),
									created_at: Set(chrono::Utc::now()),
									..Default::default()
								};

								let mime_result =
									new_mime.insert(ctx.library_db()).await.map_err(|e| {
										JobError::execution(format!(
											"Failed to create mime type: {}",
											e
										))
									})?;

								Some(mime_result.id)
							}
						}
					} else {
						None
					};

					(kind_id, mime_type_id)
				}
				Err(_) => {
					// If identification fails, fall back to "unknown" (0)
					(0, None)
				}
			};

			let new_content = entities::content_identity::ActiveModel {
				uuid: Set(Some(deterministic_uuid)), // Deterministic UUID for sync
				integrity_hash: Set(None),           // Generated later by validate job
				content_hash: Set(content_hash.clone()),
				mime_type_id: Set(mime_type_id),
				kind_id: Set(kind_id),
				text_content: Set(None), // TODO: Extract text content for indexing
				total_size: Set(file_size),
				entry_count: Set(1),
				first_seen_at: Set(chrono::Utc::now()),
				last_verified_at: Set(chrono::Utc::now()),
				..Default::default()
			};

			// Try to insert, but handle unique constraint violations
			let result = match new_content.insert(ctx.library_db()).await {
				Ok(model) => (model, true),
				Err(e) => {
					// Check if it's a unique constraint violation
					if e.to_string().contains("UNIQUE constraint failed") {
						// Another job created it - find and use the existing one
						let existing = entities::content_identity::Entity::find()
							.filter(entities::content_identity::Column::ContentHash.eq(&content_hash))
							.one(ctx.library_db())
							.await
							.map_err(|e| JobError::execution(format!("Failed to find existing content identity: {}", e)))?
							.ok_or_else(|| JobError::execution("Content identity should exist after unique constraint violation".to_string()))?;

						// Update entry count
						let mut existing_active: entities::content_identity::ActiveModel =
							existing.clone().into();
						existing_active.entry_count = Set(existing.entry_count + 1);
						existing_active.last_verified_at = Set(chrono::Utc::now());

						let updated = existing_active
							.update(ctx.library_db())
							.await
							.map_err(|e| {
								JobError::execution(format!(
									"Failed to update content identity: {}",
									e
								))
							})?;

						(updated, false)
					} else {
						return Err(JobError::execution(format!(
							"Failed to create content identity: {}",
							e
						)));
					}
				}
			};

			result
		};

		// Update Entry with content_id AND assign UUID (now ready for sync)
		let entry = entities::entry::Entity::find_by_id(entry_id)
			.one(ctx.library_db())
			.await
			.map_err(|e| JobError::execution(format!("Failed to find entry: {}", e)))?
			.ok_or_else(|| JobError::execution("Entry not found after creation".to_string()))?;

		let mut entry_active: entities::entry::ActiveModel = entry.into();
		entry_active.content_id = Set(Some(content_model.id));

		// Assign UUID if not already assigned (Entry now ready for sync)
		use sea_orm::ActiveValue::{NotSet, Set, Unchanged};
		match &entry_active.uuid {
			Set(None) | NotSet | Unchanged(None) => {
				let new_uuid = Uuid::new_v4();
				entry_active.uuid = Set(Some(new_uuid));
			}
			Set(Some(_)) | Unchanged(Some(_)) => {
				// Already has UUID, no action needed
			}
		}

		let updated_entry = entry_active.update(ctx.library_db()).await.map_err(|e| {
			JobError::execution(format!("Failed to link content identity to entry: {}", e))
		})?;

		Ok(ContentLinkResult {
			content_identity: content_model,
			entry: updated_entry,
			is_new_content,
		})
	}

	/// Simple move entry within existing transaction (no directory path cascade updates)
	pub async fn simple_move_entry_in_conn(
		state: &mut IndexerState,
		ctx: &impl IndexingCtx,
		entry_id: i32,
		old_path: &Path,
		new_path: &Path,
		txn: &DatabaseTransaction,
	) -> Result<(), JobError> {
		// Get the entry
		let db_entry = entities::entry::Entity::find_by_id(entry_id)
			.one(txn)
			.await
			.map_err(|e| JobError::execution(format!("Failed to find entry: {}", e)))?
			.ok_or_else(|| JobError::execution("Entry not found for move".to_string()))?;

		let mut entry_active: entities::entry::ActiveModel = db_entry.into();

		// Find new parent entry ID
		let new_parent_id = if let Some(parent_path) = new_path.parent() {
			state.entry_id_cache.get(parent_path).copied()
		} else {
			None
		};

		// Update entry fields
		entry_active.parent_id = Set(new_parent_id);

		// Extract new name and extension for files
		match new_path.extension() {
			Some(ext) => {
				// File with extension
				if let Some(stem) = new_path.file_stem() {
					entry_active.name = Set(stem.to_string_lossy().to_string());
					entry_active.extension = Set(Some(ext.to_string_lossy().to_lowercase()));
				}
			}
			None => {
				// File without extension or directory
				if let Some(name) = new_path.file_name() {
					entry_active.name = Set(name.to_string_lossy().to_string());
					entry_active.extension = Set(None);
				}
			}
		}

		// Save the updated entry
		entry_active
			.update(txn)
			.await
			.map_err(|e| JobError::execution(format!("Failed to update entry: {}", e)))?;

		// Update cache
		state.entry_id_cache.remove(old_path);
		state
			.entry_id_cache
			.insert(new_path.to_path_buf(), entry_id);

		Ok(())
	}

	/// Bulk move entries within a single transaction for better performance
	pub async fn bulk_move_entries(
		state: &mut IndexerState,
		ctx: &impl IndexingCtx,
		moves: &[(i32, PathBuf, PathBuf, super::state::DirEntry)],
		_location_root_path: &Path,
		txn: &DatabaseTransaction,
	) -> Result<usize, JobError> {
		let mut moved_count = 0;

		for (entry_id, old_path, new_path, _) in moves {
			match Self::simple_move_entry_in_conn(state, ctx, *entry_id, old_path, new_path, txn)
				.await
			{
				Ok(()) => {
					moved_count += 1;
				}
				Err(e) => {
					// Log error but continue with other moves
					ctx.log(format!(
						"Failed to move entry {} from {} to {}: {}",
						entry_id,
						old_path.display(),
						new_path.display(),
						e
					));
				}
			}
		}

		Ok(moved_count)
	}

	/// Update entry within existing transaction
	pub async fn update_entry_in_conn(
		ctx: &impl IndexingCtx,
		entry_id: i32,
		entry: &super::state::DirEntry,
		txn: &DatabaseTransaction,
	) -> Result<(), JobError> {
		// Get the existing entry
		let db_entry = entities::entry::Entity::find_by_id(entry_id)
			.one(txn)
			.await
			.map_err(|e| JobError::execution(format!("Failed to find entry: {}", e)))?
			.ok_or_else(|| JobError::execution("Entry not found for update".to_string()))?;

		let mut entry_active: entities::entry::ActiveModel = db_entry.into();

		// Update size if it changed
		if let Ok(metadata) = std::fs::symlink_metadata(&entry.path) {
			entry_active.size = Set(metadata.len() as i64);

			// Update modified time
			if let Ok(modified) = metadata.modified() {
				if let Ok(duration) = modified.duration_since(std::time::UNIX_EPOCH) {
					entry_active.modified_at = Set(chrono::DateTime::from_timestamp(
						duration.as_secs() as i64,
						0,
					)
					.unwrap_or_default());
				}
			}
		}

		// Save the updated entry
		entry_active
			.update(txn)
			.await
			.map_err(|e| JobError::execution(format!("Failed to update entry: {}", e)))?;

		Ok(())
	}
}
