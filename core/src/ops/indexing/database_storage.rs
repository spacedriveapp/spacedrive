//! # Core Database Storage for Indexing
//!
//! `core::ops::indexing::database_storage` provides the foundational database operations layer
//! for the indexing system. All database writes (creates, updates, moves, deletes) flow
//! through this module, ensuring consistency across both watcher and job pipelines.
//!
//! ## Key Design Decisions
//!
//! **Closure Table Hierarchy:** Parent-child relationships use a closure table
//! (`entry_closure`) instead of recursive Common Table Expressions (CTEs). This makes
//! "find all descendants" queries O(1) regardless of nesting depth, at the cost of
//! additional storage (~N² in worst case for deeply nested trees). Move operations
//! require rebuilding closures for the entire moved subtree.
//!
//! **Ephemeral UUID Preservation:** When converting ephemeral browsing sessions to
//! persistent indexed locations, entries retain their original UUIDs. This prevents
//! orphaning user metadata (tags, notes, colors) that were attached during browsing.
//! Without preservation, promoting `/mnt/nas` to a managed location would generate new
//! UUIDs and break all existing tag associations.
//!
//! **Globally Deterministic Content UUIDs:** Content identities use v5 UUIDs (namespace hash of
//! `content_hash` only) so any device can independently identify identical files and merge
//! metadata without coordination. This enables offline duplicate detection across all devices
//! and libraries.
//!
//! ## Example
//! ```rust,no_run
//! use spacedrive_core::ops::indexing::{DatabaseStorage, state::DirEntry};
//!
//! let entry = DirEntry { /* ... */ };
//! let entry_id = DatabaseStorage::create_entry(
//!     &mut state,
//!     &ctx,
//!     &entry,
//!     device_id,
//!     &location_root,
//! ).await?;
//! ```

use super::path_resolver::PathResolver;
use super::state::{DirEntry, EntryKind, IndexerState};
use crate::infra::job::prelude::JobError;
use crate::library::Library;
use crate::{
	filetype::FileTypeRegistry,
	infra::db::entities::{self, directory_paths, entry_closure},
};
use sea_orm::{
	ActiveModelTrait, ActiveValue::Set, ColumnTrait, ConnectionTrait, DatabaseConnection,
	DatabaseTransaction, DbBackend, EntityTrait, IntoActiveModel, QueryFilter, QuerySelect,
	Statement, TransactionTrait,
};
use std::path::{Path, PathBuf};
use uuid::Uuid;

/// Normalizes cloud storage paths to match PathBuf::parent() semantics.
///
/// Cloud backends (S3, Dropbox) store directory paths with trailing slashes
/// ("s3://bucket/folder/"), but Rust's PathBuf::parent() strips the trailing slash.
/// This mismatch breaks cache lookups when creating child entries. We normalize by
/// removing the trailing slash for cloud paths so cached parent IDs can be found.
fn normalize_cloud_dir_path(path: &Path) -> PathBuf {
	let path_str = path.to_string_lossy();
	if path_str.contains("://") && path_str.ends_with('/') {
		PathBuf::from(path_str.trim_end_matches('/'))
	} else {
		path.to_path_buf()
	}
}

/// Snapshot of filesystem metadata for a single entry.
///
/// This struct is deliberately separate from the database `entry::Model` to
/// decouple discovery (filesystem operations) from persistence (database writes).
/// During ephemeral browsing, thousands of these are created in memory without
/// touching the database, while persistent indexing converts them to ActiveModels
/// in batch transactions.
///
/// The `inode` field is populated on Unix systems but remains `None` on Windows,
/// where file indices are unstable across reboots. Change detection uses
/// (inode, mtime, size) tuples when available, falling back to path-only matching.
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

/// Core database operations for the indexing system.
///
/// DatabaseStorage provides the foundational layer for all database writes during indexing.
/// Both the watcher pipeline (`DatabaseAdapter`) and job pipeline use these methods,
/// ensuring consistent database operations. All methods come in both standalone
/// (creates own transaction) and `_in_conn` variants (uses existing transaction)
/// for flexible batch operations.
pub struct DatabaseStorage;

/// Result of linking an entry to its content identity.
///
/// Returned by `link_to_content_identity` to provide both models for sync operations.
/// The caller must sync both the content_identity and entry if running outside the
/// job system (e.g., file watcher). The `is_new_content` flag indicates whether this
/// is the first entry with this content hash, which triggers thumbnail generation.
pub struct ContentLinkResult {
	pub content_identity: entities::content_identity::Model,
	pub entry: entities::entry::Model,
	pub is_new_content: bool,
}

impl DatabaseStorage {
	/// Get platform-specific inode
	#[cfg(unix)]
	pub fn get_inode(metadata: &std::fs::Metadata) -> Option<u64> {
		use std::os::unix::fs::MetadataExt;
		Some(metadata.ino())
	}

	#[cfg(windows)]
	pub fn get_inode(_metadata: &std::fs::Metadata) -> Option<u64> {
		// Windows file indices exist but are unstable across reboots and volume operations,
		// making them unsuitable for change detection. We return None and fall back to
		// path-only matching, which is sufficient since Windows NTFS doesn't support hard
		// links for directories (the main inode use case on Unix).
		None
	}

	#[cfg(not(any(unix, windows)))]
	pub fn get_inode(_metadata: &std::fs::Metadata) -> Option<u64> {
		None
	}

	/// Resolves a parent directory path to its entry ID via pure database lookup.
	///
	/// This is the foundational database query operation. Callers (writers) should
	/// check their cache first, then call this method if the ID isn't cached.
	///
	/// For cloud paths (containing "://"), tries both with and without trailing slashes
	/// since cloud backends may store paths inconsistently.
	pub async fn resolve_parent_id(
		db: &DatabaseConnection,
		parent_path: &Path,
	) -> Result<Option<i32>, JobError> {
		let parent_path_str = parent_path.to_string_lossy().to_string();
		let is_cloud = parent_path_str.contains("://");

		let parent_variants = if is_cloud && !parent_path_str.ends_with('/') {
			vec![parent_path_str.clone(), format!("{}/", parent_path_str)]
		} else {
			vec![parent_path_str.clone()]
		};

		let query = entities::directory_paths::Entity::find()
			.filter(entities::directory_paths::Column::Path.is_in(parent_variants));

		match query.one(db).await {
			Ok(Some(dir_path_record)) => Ok(Some(dir_path_record.entry_id)),
			Ok(None) => Ok(None),
			Err(e) => Err(JobError::execution(format!(
				"Failed to resolve parent ID for {}: {}",
				parent_path.display(),
				e
			))),
		}
	}

	/// Extracts filesystem metadata through either a volume backend or direct I/O.
	///
	/// Volume backends abstract cloud storage (S3, Dropbox) and local filesystems
	/// behind a unified interface. When a backend is provided, metadata comes from
	/// the volume's cache or API; otherwise this falls back to `tokio::fs` for local
	/// paths. Cloud volumes MUST provide a backend since there's no local file to read.
	///
	/// Returns `Err` if the path doesn't exist or lacks read permissions. On permission
	/// errors, the entry should still be indexed as inaccessible rather than skipped
	/// entirely - this preserves the directory tree structure for UI navigation.
	pub async fn extract_metadata(
		path: &Path,
		backend: Option<&std::sync::Arc<dyn crate::volume::VolumeBackend>>,
	) -> Result<EntryMetadata, std::io::Error> {
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
		entry: &DirEntry,
		device_id: i32,
		location_root_path: &Path,
		conn: &C,
		out_self_closures: &mut Vec<entry_closure::ActiveModel>,
		out_dir_paths: &mut Vec<directory_paths::ActiveModel>,
	) -> Result<entities::entry::Model, JobError> {
		// Extensions are normalized to lowercase and stored without the leading dot
		// because search queries are case-insensitive ("JPG" should match "*.jpg").
		// Directories never have extensions even if named "folder.app" since macOS
		// treats .app bundles as atomic units, not files with extensions.
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

		// UUID assignment: preserve ephemeral UUIDs from prior browsing sessions
		// so user metadata (tags, notes) survives the transition to persistent indexing.
		let entry_uuid = if let Some(ephemeral_uuid) = state.get_ephemeral_uuid(&entry.path) {
			tracing::debug!(
				"Preserving ephemeral UUID {} for {}",
				ephemeral_uuid,
				entry.path.display()
			);
			Some(ephemeral_uuid)
		} else {
			Some(Uuid::new_v4())
		};

		// Parent ID should already be resolved and cached by the caller (writer layer).
		// This keeps DBWriter focused on pure database operations without cache management.
		let parent_id = entry
			.path
			.parent()
			.and_then(|parent_path| state.entry_id_cache.get(parent_path).copied());

		let now = chrono::Utc::now();
		tracing::debug!(
			"Creating entry: name={}, path={}, inode={:?}, parent_id={:?}",
			name,
			entry.path.display(),
			entry.inode,
			parent_id
		);
		let new_entry = entities::entry::ActiveModel {
			uuid: Set(entry_uuid),
			name: Set(name.clone()),
			kind: Set(Self::entry_kind_to_int(entry.kind)),
			extension: Set(extension),
			metadata_id: Set(None),
			content_id: Set(None),
			size: Set(entry.size as i64),
			aggregate_size: Set(0),
			child_count: Set(0),
			file_count: Set(0),
			created_at: Set(now),
			modified_at: Set(modified_at),
			accessed_at: Set(None),
			indexed_at: Set(Some(now)),
			permissions: Set(None),
			inode: Set(entry.inode.map(|i| i as i64)),
			parent_id: Set(parent_id),
			..Default::default()
		};

		let result = new_entry
			.insert(conn)
			.await
			.map_err(|e| JobError::execution(format!("Failed to create entry: {}", e)))?;

		tracing::debug!(
			"Entry inserted in DB: id={}, name={}, inode={:?}",
			result.id,
			result.name,
			result.inode
		);

		let self_closure = entry_closure::ActiveModel {
			ancestor_id: Set(result.id),
			descendant_id: Set(result.id),
			depth: Set(0),
			..Default::default()
		};
		out_self_closures.push(self_closure);

		// Copy all parent's ancestor relationships to build the transitive closure for this entry.
		// This allows "find all descendants" queries to run in O(1) without recursive traversal.
		if let Some(parent_id) = parent_id {
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

		if entry.kind == EntryKind::Directory {
			let absolute_path = entry.path.to_string_lossy().to_string();
			let dir_path_entry = directory_paths::ActiveModel {
				entry_id: Set(result.id),
				path: Set(absolute_path),
				..Default::default()
			};
			out_dir_paths.push(dir_path_entry);
		}

		// Normalize cloud directory paths (remove trailing slash) so child entries can find
		// their parent in the cache. PathBuf::parent() doesn't include trailing slashes, but
		// cloud backends may store "s3://bucket/folder/" with the slash.
		let cache_key = if entry.kind == EntryKind::Directory {
			normalize_cloud_dir_path(&entry.path)
		} else {
			entry.path.clone()
		};
		state.entry_id_cache.insert(cache_key, result.id);

		Ok(result)
	}

	/// Create an entry, starting and committing its own transaction (single insert)
	pub async fn create_entry(
		state: &mut IndexerState,
		db: &DatabaseConnection,
		library: Option<&Library>,
		entry: &DirEntry,
		device_id: i32,
		location_root_path: &Path,
	) -> Result<i32, JobError> {
		let txn = db
			.begin()
			.await
			.map_err(|e| JobError::execution(format!("Failed to begin transaction: {}", e)))?;

		let mut self_closures: Vec<entry_closure::ActiveModel> = Vec::new();
		let mut dir_paths: Vec<directory_paths::ActiveModel> = Vec::new();
		let result = Self::create_entry_in_conn(
			state,
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
		if let Some(library) = library {
			tracing::info!(
				"ENTRY_SYNC: About to sync entry name={} uuid={:?}",
				entry_model.name,
				entry_model.uuid
			);
			if let Err(e) = library
				.sync_model_with_db(&entry_model, crate::infra::sync::ChangeType::Insert, db)
				.await
			{
				tracing::warn!(
					"ENTRY_SYNC: Failed to sync entry {}: {}",
					entry_model
						.uuid
						.map(|u| u.to_string())
						.unwrap_or_else(|| "no-uuid".to_string()),
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
		db: &DatabaseConnection,
		entry_id: i32,
		entry: &DirEntry,
	) -> Result<(), JobError> {
		let db_entry = entities::entry::Entity::find_by_id(entry_id)
			.one(db)
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

		// Update indexed_at so incremental sync picks up this change.
		// The watermark-based query filters on indexed_at, so skipping this would
		// cause modified entries to be ignored on subsequent scans.
		entry_active.indexed_at = Set(Some(chrono::Utc::now()));

		entry_active
			.update(db)
			.await
			.map_err(|e| JobError::execution(format!("Failed to update entry: {}", e)))?;

		Ok(())
	}

	/// Handle entry move operation with closure table updates (creates own transaction)
	pub async fn move_entry(
		state: &mut IndexerState,
		db: &DatabaseConnection,
		entry_id: i32,
		old_path: &Path,
		new_path: &Path,
		location_root_path: &Path,
	) -> Result<(), JobError> {
		let txn = db
			.begin()
			.await
			.map_err(|e| JobError::execution(format!("Failed to begin transaction: {}", e)))?;

		let result = Self::move_entry_in_conn(
			state,
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
		entry_id: i32,
		old_path: &Path,
		new_path: &Path,
		location_root_path: &Path,
		txn: &DatabaseTransaction,
	) -> Result<(), JobError> {
		let db_entry = entities::entry::Entity::find_by_id(entry_id)
			.one(txn)
			.await
			.map_err(|e| JobError::execution(format!("Failed to find entry: {}", e)))?
			.ok_or_else(|| JobError::execution("Entry not found for move".to_string()))?;

		let is_directory = db_entry.kind == Self::entry_kind_to_int(EntryKind::Directory);
		let mut entry_active: entities::entry::ActiveModel = db_entry.into();

		let new_parent_id = if let Some(parent_path) = new_path.parent() {
			state.entry_id_cache.get(parent_path).copied()
		} else {
			None
		};

		entry_active.parent_id = Set(new_parent_id);

		let mut new_name_value = None;
		if let Some(new_name) = new_path.file_stem() {
			let name_string = new_name.to_string_lossy().to_string();
			new_name_value = Some(name_string.clone());
			entry_active.name = Set(name_string);
		}

		entry_active
			.update(txn)
			.await
			.map_err(|e| JobError::execution(format!("Failed to update entry: {}", e)))?;

		// Rebuild closure table for the moved subtree. Moving a directory with 10,000 descendants
		// requires updating ~50M closure rows in the worst case (full tree reconnection). We do this
		// in two steps: (1) disconnect the subtree from old ancestors, (2) reconnect to new parent.
		// Step 1: Delete all ancestor relationships for the moved subtree, but preserve internal
		// relationships (entries within the subtree can still find their descendants).
		txn.execute_unprepared(&format!(
            "DELETE FROM entry_closure \
             WHERE descendant_id IN (SELECT descendant_id FROM entry_closure WHERE ancestor_id = {}) \
             AND ancestor_id NOT IN (SELECT descendant_id FROM entry_closure WHERE ancestor_id = {})",
            entry_id, entry_id
        ))
        .await
        .map_err(|e| JobError::execution(format!("Failed to disconnect subtree: {}", e)))?;

		// Step 2: Reconnect the subtree under the new parent by creating closure rows for all
		// (ancestor, descendant) pairs where ancestor is in the new parent chain and descendant
		// is in the moved subtree. The depth is calculated as parent_depth + child_depth + 1.
		if let Some(new_parent_id) = new_parent_id {
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

		if is_directory {
			let new_name = new_name_value.unwrap_or_else(|| {
				new_path
					.file_name()
					.and_then(|n| n.to_str())
					.unwrap_or("unknown")
					.to_string()
			});

			let new_directory_path =
				PathResolver::build_directory_path(txn, new_parent_id, &new_name)
					.await
					.map_err(|e| {
						JobError::execution(format!("Failed to build new directory path: {}", e))
					})?;

			let old_directory_path = PathResolver::get_directory_path(txn, entry_id)
				.await
				.map_err(|e| {
					JobError::execution(format!("Failed to get old directory path: {}", e))
				})?;

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

			// Cascade path updates to all descendant directories. Moving "/home/user/docs" to
			// "/backup/docs" requires rewriting paths for every child, which can be thousands
			// of directories. This runs in the same transaction to maintain consistency.
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

	/// Links an entry to its content identity, deduplicating files with identical hashes.
	///
	/// Content identities are shared across all entries with the same content hash
	/// (computed via BLAKE3). When two files have identical content, they reference
	/// the same `content_identity` row, enabling "find all duplicates" queries and
	/// reducing thumbnail storage (one thumbnail per content, not per entry).
	///
	/// Each content identity gets a globally deterministic UUID (v5 hash of content_hash only)
	/// so any device can independently identify the same content and merge metadata without
	/// coordination. This enables offline duplicate detection across all devices and libraries.
	///
	/// Returns both the content identity and the updated entry for batch sync operations.
	/// The caller must sync both models if running outside the job system (e.g., watcher).
	pub async fn link_to_content_identity(
		db: &DatabaseConnection,
		entry_id: i32,
		path: &Path,
		content_hash: String,
	) -> Result<ContentLinkResult, JobError> {
		let existing = entities::content_identity::Entity::find()
			.filter(entities::content_identity::Column::ContentHash.eq(&content_hash))
			.one(db)
			.await
			.map_err(|e| JobError::execution(format!("Failed to query content identity: {}", e)))?;

		let (content_model, is_new_content) = if let Some(existing) = existing {
			let mut existing_active: entities::content_identity::ActiveModel = existing.into();
			existing_active.entry_count = Set(existing_active.entry_count.unwrap() + 1);
			existing_active.last_verified_at = Set(chrono::Utc::now());

			let updated = existing_active.update(db).await.map_err(|e| {
				JobError::execution(format!("Failed to update content identity: {}", e))
			})?;

			(updated, false)
		} else {
			let file_size = tokio::fs::symlink_metadata(path)
				.await
				.map(|m| m.len() as i64)
				.unwrap_or(0);

			// Generate globally deterministic v5 UUID so any device can independently
			// create the same content identity UUID for duplicate files, enabling
			// cross-device and cross-library deduplication without coordination.
			let deterministic_uuid =
				entities::content_identity::Model::deterministic_uuid(&content_hash);

			let registry = FileTypeRegistry::default();
			let file_type_result = registry.identify(path).await;

			let (kind_id, mime_type_id) = match file_type_result {
				Ok(result) => {
					let kind_id = result.file_type.category as i32;

					let mime_type_id = if let Some(mime_str) = result.file_type.primary_mime_type()
					{
						let existing = entities::mime_type::Entity::find()
							.filter(entities::mime_type::Column::MimeType.eq(mime_str))
							.one(db)
							.await
							.map_err(|e| {
								JobError::execution(format!("Failed to query mime type: {}", e))
							})?;

						match existing {
							Some(mime_record) => Some(mime_record.id),
							None => {
								let new_mime = entities::mime_type::ActiveModel {
									uuid: Set(Uuid::new_v4()),
									mime_type: Set(mime_str.to_string()),
									created_at: Set(chrono::Utc::now()),
									..Default::default()
								};

								let mime_result = new_mime.insert(db).await.map_err(|e| {
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
				Err(_) => (0, None),
			};

			let new_content = entities::content_identity::ActiveModel {
				uuid: Set(Some(deterministic_uuid)),
				integrity_hash: Set(None),
				content_hash: Set(content_hash.clone()),
				mime_type_id: Set(mime_type_id),
				kind_id: Set(kind_id),
				text_content: Set(None),
				total_size: Set(file_size),
				entry_count: Set(1),
				first_seen_at: Set(chrono::Utc::now()),
				last_verified_at: Set(chrono::Utc::now()),
				..Default::default()
			};

			// Handle race condition: another job (or device sync) may have created this
			// content identity between our check and insert. Catch UNIQUE constraint violations
			// and use the existing record instead of failing.
			let result = match new_content.insert(db).await {
				Ok(model) => (model, true),
				Err(e) => {
					if e.to_string().contains("UNIQUE constraint failed") {
						let existing = entities::content_identity::Entity::find()
                            .filter(entities::content_identity::Column::ContentHash.eq(&content_hash))
                            .one(db)
                            .await
                            .map_err(|e| JobError::execution(format!("Failed to find existing content identity: {}", e)))?
                            .ok_or_else(|| JobError::execution("Content identity should exist after unique constraint violation".to_string()))?;

						let mut existing_active: entities::content_identity::ActiveModel =
							existing.clone().into();
						existing_active.entry_count = Set(existing.entry_count + 1);
						existing_active.last_verified_at = Set(chrono::Utc::now());

						let updated = existing_active.update(db).await.map_err(|e| {
							JobError::execution(format!("Failed to update content identity: {}", e))
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

		let entry = entities::entry::Entity::find_by_id(entry_id)
			.one(db)
			.await
			.map_err(|e| JobError::execution(format!("Failed to find entry: {}", e)))?
			.ok_or_else(|| JobError::execution("Entry not found after creation".to_string()))?;

		let mut entry_active: entities::entry::ActiveModel = entry.into();
		entry_active.content_id = Set(Some(content_model.id));

		let updated_entry = entry_active.update(db).await.map_err(|e| {
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

		let new_parent_id = if let Some(parent_path) = new_path.parent() {
			// Check cache first, then fall back to database query
			if let Some(&parent_id) = state.entry_id_cache.get(parent_path) {
				Some(parent_id)
			} else {
				// Parent not in cache - query database
				let parent_path_str = parent_path.to_string_lossy().to_string();
				let is_cloud = parent_path_str.contains("://");

				let parent_variants = if is_cloud && !parent_path_str.ends_with('/') {
					vec![parent_path_str.clone(), format!("{}/", parent_path_str)]
				} else {
					vec![parent_path_str.clone()]
				};

				let query = entities::directory_paths::Entity::find()
					.filter(entities::directory_paths::Column::Path.is_in(parent_variants));

				match query.one(txn).await {
					Ok(Some(dir_path_record)) => {
						let parent_id = dir_path_record.entry_id;
						// Cache the parent ID for future lookups
						state
							.entry_id_cache
							.insert(parent_path.to_path_buf(), parent_id);
						Some(parent_id)
					}
					Ok(None) => None,
					Err(e) => {
						return Err(JobError::execution(format!(
							"Failed to resolve parent ID for {}: {}",
							parent_path.display(),
							e
						)));
					}
				}
			}
		} else {
			None
		};

		entry_active.parent_id = Set(new_parent_id);

		match new_path.extension() {
			Some(ext) => {
				if let Some(stem) = new_path.file_stem() {
					entry_active.name = Set(stem.to_string_lossy().to_string());
					entry_active.extension = Set(Some(ext.to_string_lossy().to_lowercase()));
				}
			}
			None => {
				if let Some(name) = new_path.file_name() {
					entry_active.name = Set(name.to_string_lossy().to_string());
					entry_active.extension = Set(None);
				}
			}
		}

		entry_active
			.update(txn)
			.await
			.map_err(|e| JobError::execution(format!("Failed to update entry: {}", e)))?;

		state.entry_id_cache.remove(old_path);
		state
			.entry_id_cache
			.insert(new_path.to_path_buf(), entry_id);

		Ok(())
	}

	/// Bulk move entries within a single transaction for better performance
	pub async fn bulk_move_entries(
		state: &mut IndexerState,
		moves: &[(i32, PathBuf, PathBuf, super::state::DirEntry)],
		_location_root_path: &Path,
		txn: &DatabaseTransaction,
	) -> Result<usize, JobError> {
		let mut moved_count = 0;

		for (entry_id, old_path, new_path, _) in moves {
			match Self::simple_move_entry_in_conn(state, *entry_id, old_path, new_path, txn).await {
				Ok(()) => {
					moved_count += 1;
				}
				Err(e) => {
					// Bulk move operations are best-effort: one failure shouldn't roll back
					// the entire batch. Parent directory renames succeed even if a child fails
					// due to file locks, though the child will have a stale path until the next
					// reindex cleans it up.
					tracing::debug!(
						"Failed to move entry {} from {} to {}: {}",
						entry_id,
						old_path.display(),
						new_path.display(),
						e
					);
				}
			}
		}

		Ok(moved_count)
	}

	/// Update entry within existing transaction
	pub async fn update_entry_in_conn(
		entry_id: i32,
		entry: &super::state::DirEntry,
		txn: &DatabaseTransaction,
	) -> Result<(), JobError> {
		let db_entry = entities::entry::Entity::find_by_id(entry_id)
			.one(txn)
			.await
			.map_err(|e| JobError::execution(format!("Failed to find entry: {}", e)))?
			.ok_or_else(|| JobError::execution("Entry not found for update".to_string()))?;

		let mut entry_active: entities::entry::ActiveModel = db_entry.into();

		if let Ok(metadata) = std::fs::symlink_metadata(&entry.path) {
			entry_active.size = Set(metadata.len() as i64);

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

		entry_active
			.update(txn)
			.await
			.map_err(|e| JobError::execution(format!("Failed to update entry: {}", e)))?;

		Ok(())
	}

	/// Deletes an entry and all its descendants from the database.
	///
	/// This is a raw database operation that does NOT:
	/// - Create tombstones for sync
	/// - Emit events for UI updates
	/// - Run any processors
	///
	/// Use cases:
	/// - Applying remote tombstones (deletion already synced)
	/// - Cascade deletes from entity relationships
	/// - Database cleanup operations
	///
	/// For watcher-triggered deletions that need sync/events, use
	/// `DatabaseAdapter::delete()` instead.
	pub async fn delete_subtree(
		entry_id: i32,
		db: &sea_orm::DatabaseConnection,
	) -> Result<(), sea_orm::DbErr> {
		use sea_orm::TransactionTrait;

		let txn = db.begin().await?;
		Self::delete_subtree_in_txn(entry_id, &txn).await?;
		txn.commit().await?;
		Ok(())
	}

	/// Deletes a subtree within an existing transaction.
	///
	/// Traverses via entry_closure to find all descendants, then deletes
	/// closure links, directory_paths, and entries in the correct order.
	pub async fn delete_subtree_in_txn<C>(entry_id: i32, db: &C) -> Result<(), sea_orm::DbErr>
	where
		C: sea_orm::ConnectionTrait,
	{
		use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};

		let mut to_delete_ids: Vec<i32> = vec![entry_id];
		if let Ok(rows) = entities::entry_closure::Entity::find()
			.filter(entities::entry_closure::Column::AncestorId.eq(entry_id))
			.all(db)
			.await
		{
			to_delete_ids.extend(rows.into_iter().map(|r| r.descendant_id));
		}
		to_delete_ids.sort_unstable();
		to_delete_ids.dedup();

		if !to_delete_ids.is_empty() {
			let _ = entities::entry_closure::Entity::delete_many()
				.filter(entities::entry_closure::Column::DescendantId.is_in(to_delete_ids.clone()))
				.exec(db)
				.await;
			let _ = entities::entry_closure::Entity::delete_many()
				.filter(entities::entry_closure::Column::AncestorId.is_in(to_delete_ids.clone()))
				.exec(db)
				.await;

			let _ = entities::directory_paths::Entity::delete_many()
				.filter(entities::directory_paths::Column::EntryId.is_in(to_delete_ids.clone()))
				.exec(db)
				.await;

			let _ = entities::entry::Entity::delete_many()
				.filter(entities::entry::Column::Id.is_in(to_delete_ids))
				.exec(db)
				.await;
		}

		Ok(())
	}
}
