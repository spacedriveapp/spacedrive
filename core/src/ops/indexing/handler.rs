//! Unified change handling for persistent and ephemeral indexing.
//!
//! This module provides a trait-based abstraction for filesystem change handling,
//! allowing the same logic to work with both database-backed (persistent) and
//! memory-backed (ephemeral) storage. The watcher and responder use these handlers
//! to process Create/Modify/Remove/Rename events consistently.
//!
//! ## Architecture
//!
//! ```text
//! FsRawEventKind
//!       │
//!       ▼
//! ┌─────────────────────────────────────────────┐
//! │          apply_change (shared logic)        │
//! │  - path validation                          │
//! │  - rule filtering                           │
//! │  - metadata extraction                      │
//! │  - inode-based move detection               │
//! └──────────────────┬──────────────────────────┘
//!                    │
//!         ┌─────────┴─────────┐
//!         ▼                   ▼
//! ┌───────────────┐   ┌───────────────┐
//! │  Persistent   │   │   Ephemeral   │
//! │ ChangeHandler │   │ ChangeHandler │
//! │  (database)   │   │  (in-memory)  │
//! └───────────────┘   └───────────────┘
//! ```

use super::rules::{build_default_ruler, RuleToggles, RulerDecision};
use super::state::{DirEntry, EntryKind};
use anyhow::Result;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use uuid::Uuid;

/// Reference to an entry in either persistent or ephemeral storage.
///
/// Provides a uniform way to refer to entries regardless of storage backend.
/// Persistent entries have database IDs; ephemeral entries have arena indices.
#[derive(Debug, Clone)]
pub struct EntryRef {
	/// For persistent: database entry ID. For ephemeral: synthetic ID.
	pub id: i32,
	/// UUID for sync and event emission.
	pub uuid: Option<Uuid>,
	/// Full filesystem path.
	pub path: PathBuf,
	/// Entry kind (file/directory/symlink).
	pub kind: EntryKind,
}

impl EntryRef {
	pub fn is_directory(&self) -> bool {
		self.kind == EntryKind::Directory
	}
}

/// Type of change for event emission.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChangeType {
	Created,
	Modified,
	Moved,
	Deleted,
}

/// Configuration for change handling operations.
pub struct ChangeConfig<'a> {
	pub rule_toggles: RuleToggles,
	pub location_root: &'a Path,
	pub volume_backend: Option<&'a Arc<dyn crate::volume::VolumeBackend>>,
}

/// Abstracts storage operations for filesystem change handling.
///
/// Both persistent (database) and ephemeral (in-memory) handlers implement
/// this trait, allowing the same change processing logic to work with both
/// storage backends. The trait methods map to CRUD operations plus event
/// emission and processor execution.
#[async_trait::async_trait]
pub trait ChangeHandler: Send + Sync {
	/// Find an entry by its full filesystem path.
	async fn find_by_path(&self, path: &Path) -> Result<Option<EntryRef>>;

	/// Find an entry by inode (for move detection).
	/// Returns None if inode tracking is not supported or no match found.
	async fn find_by_inode(&self, inode: u64) -> Result<Option<EntryRef>>;

	/// Create a new entry from filesystem metadata.
	async fn create(&mut self, metadata: &DirEntry, parent_path: &Path) -> Result<EntryRef>;

	/// Update an existing entry's metadata.
	async fn update(&mut self, entry: &EntryRef, metadata: &DirEntry) -> Result<()>;

	/// Move an entry from old path to new path.
	async fn move_entry(
		&mut self,
		entry: &EntryRef,
		old_path: &Path,
		new_path: &Path,
		new_parent_path: &Path,
	) -> Result<()>;

	/// Delete an entry and all its descendants.
	async fn delete(&mut self, entry: &EntryRef) -> Result<()>;

	/// Run post-create/modify processors (thumbnails, content hash).
	/// No-op for ephemeral handlers.
	async fn run_processors(&self, entry: &EntryRef, is_new: bool) -> Result<()>;

	/// Emit appropriate events for UI updates.
	async fn emit_change_event(&self, entry: &EntryRef, change_type: ChangeType) -> Result<()>;

	/// Handle directory recursion after creation.
	/// Persistent: spawns indexer job. Ephemeral: inline shallow index.
	async fn handle_new_directory(&self, path: &Path) -> Result<()>;
}

// ============================================================================
// Shared Logic - Used by both handlers
// ============================================================================

/// Check if a path exists, distinguishing between "doesn't exist" and "can't access".
///
/// This is critical for preventing false deletions when volumes go offline.
/// Returns Ok(true) if path exists, Ok(false) if confirmed absent, Err if inaccessible.
pub async fn path_exists_safe(
	path: &Path,
	backend: Option<&Arc<dyn crate::volume::VolumeBackend>>,
) -> Result<bool> {
	use crate::volume::error::VolumeError;

	if let Some(backend) = backend {
		match backend.exists(path).await {
			Ok(exists) => Ok(exists),
			Err(VolumeError::NotMounted(_)) => {
				tracing::warn!(
					"Volume not mounted when checking path existence: {}",
					path.display()
				);
				Err(anyhow::anyhow!(
					"Volume not mounted, cannot verify path existence"
				))
			}
			Err(VolumeError::Io(ref e)) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
			Err(VolumeError::Io(io_err)) => {
				tracing::warn!(
					"IO error when checking path existence for {}: {}",
					path.display(),
					io_err
				);
				Err(anyhow::anyhow!(
					"IO error, volume may be offline: {}",
					io_err
				))
			}
			Err(e) => {
				tracing::warn!(
					"Volume error when checking path existence for {}: {}",
					path.display(),
					e
				);
				Err(e.into())
			}
		}
	} else {
		match tokio::fs::try_exists(path).await {
			Ok(exists) => Ok(exists),
			Err(e) => {
				tracing::warn!(
					"Cannot verify path existence for {} (volume may be offline): {}",
					path.display(),
					e
				);
				Err(anyhow::anyhow!("Cannot access path: {}", e))
			}
		}
	}
}

/// Evaluates indexing rules to determine if a path should be skipped.
pub async fn should_filter_path(
	path: &Path,
	rule_toggles: RuleToggles,
	location_root: &Path,
	backend: Option<&Arc<dyn crate::volume::VolumeBackend>>,
) -> Result<bool> {
	let ruler = build_default_ruler(rule_toggles, location_root, path).await;

	let metadata = if let Some(backend) = backend {
		backend
			.metadata(path)
			.await
			.map_err(|e| anyhow::anyhow!("Failed to get metadata via backend: {}", e))?
	} else {
		let fs_meta = tokio::fs::metadata(path).await?;
		crate::volume::backend::RawMetadata {
			kind: if fs_meta.is_dir() {
				EntryKind::Directory
			} else if fs_meta.is_symlink() {
				EntryKind::Symlink
			} else {
				EntryKind::File
			},
			size: fs_meta.len(),
			modified: fs_meta.modified().ok(),
			created: fs_meta.created().ok(),
			accessed: fs_meta.accessed().ok(),
			inode: None,
			permissions: None,
		}
	};

	struct SimpleMetadata {
		is_dir: bool,
	}
	impl super::rules::MetadataForIndexerRules for SimpleMetadata {
		fn is_dir(&self) -> bool {
			self.is_dir
		}
	}

	let simple_meta = SimpleMetadata {
		is_dir: metadata.kind == EntryKind::Directory,
	};

	match ruler.evaluate_path(path, &simple_meta).await {
		Ok(RulerDecision::Reject) => {
			tracing::debug!("Filtered path by indexing rules: {}", path.display());
			Ok(true)
		}
		Ok(RulerDecision::Accept) => Ok(false),
		Err(e) => {
			tracing::warn!("Error evaluating rules for {}: {}", path.display(), e);
			Ok(false)
		}
	}
}

/// Extracts filesystem metadata into a DirEntry.
pub async fn build_dir_entry(
	path: &Path,
	backend: Option<&Arc<dyn crate::volume::VolumeBackend>>,
) -> Result<DirEntry> {
	use super::entry::EntryProcessor;

	let meta = EntryProcessor::extract_metadata(path, backend).await?;
	Ok(DirEntry {
		path: meta.path,
		kind: meta.kind,
		size: meta.size,
		modified: meta.modified,
		inode: meta.inode,
	})
}

// ============================================================================
// Generic Change Application
// ============================================================================

/// Apply a batch of filesystem changes using the provided handler.
///
/// Processes events in the correct order: removes first, then renames,
/// creates, and finally modifies. This prevents conflicts like creating
/// a file that should have been deleted.
pub async fn apply_batch<H: ChangeHandler>(
	handler: &mut H,
	events: Vec<crate::infra::event::FsRawEventKind>,
	config: &ChangeConfig<'_>,
) -> Result<()> {
	use crate::infra::event::FsRawEventKind;

	if events.is_empty() {
		return Ok(());
	}

	let mut creates = Vec::new();
	let mut modifies = Vec::new();
	let mut removes = Vec::new();
	let mut renames = Vec::new();

	for event in events {
		match event {
			FsRawEventKind::Create { path } => creates.push(path),
			FsRawEventKind::Modify { path } => modifies.push(path),
			FsRawEventKind::Remove { path } => removes.push(path),
			FsRawEventKind::Rename { from, to } => renames.push((from, to)),
		}
	}

	// Deduplicate (macOS sends duplicate creates)
	creates.sort();
	creates.dedup();
	modifies.sort();
	modifies.dedup();
	removes.sort();
	removes.dedup();

	tracing::debug!(
		"Processing batch: {} creates, {} modifies, {} removes, {} renames",
		creates.len(),
		modifies.len(),
		removes.len(),
		renames.len()
	);

	// Process in order: removes, renames, creates, modifies
	for path in removes {
		if let Err(e) = handle_remove(handler, &path).await {
			tracing::error!("Failed to handle remove for {}: {}", path.display(), e);
		}
	}

	for (from, to) in renames {
		if let Err(e) = handle_rename(handler, &from, &to, config).await {
			tracing::error!(
				"Failed to handle rename from {} to {}: {}",
				from.display(),
				to.display(),
				e
			);
		}
	}

	for path in creates {
		if let Err(e) = handle_create(handler, &path, config).await {
			tracing::error!("Failed to handle create for {}: {}", path.display(), e);
		}
	}

	for path in modifies {
		if let Err(e) = handle_modify(handler, &path, config).await {
			tracing::error!("Failed to handle modify for {}: {}", path.display(), e);
		}
	}

	Ok(())
}

/// Handle a create event.
///
/// Validates path, checks rules, extracts metadata, detects inode-based moves,
/// and creates the entry. For directories, triggers recursive indexing.
pub async fn handle_create<H: ChangeHandler>(
	handler: &mut H,
	path: &Path,
	config: &ChangeConfig<'_>,
) -> Result<()> {
	tracing::debug!("Create: {}", path.display());

	// 1. Validate path exists
	match path_exists_safe(path, config.volume_backend).await {
		Ok(true) => {}
		Ok(false) => {
			tracing::debug!("Path no longer exists, skipping create: {}", path.display());
			return Ok(());
		}
		Err(e) => {
			tracing::warn!(
				"Skipping create event for inaccessible path {}: {}",
				path.display(),
				e
			);
			return Ok(());
		}
	}

	// 2. Apply rule filtering
	if should_filter_path(
		path,
		config.rule_toggles,
		config.location_root,
		config.volume_backend,
	)
	.await?
	{
		tracing::debug!("Skipping filtered path: {}", path.display());
		return Ok(());
	}

	// 3. Extract metadata
	let metadata = build_dir_entry(path, config.volume_backend).await?;

	// 4. Check for existing entry (treat as modify)
	if handler.find_by_path(path).await?.is_some() {
		tracing::debug!(
			"Entry already exists at path {}, treating as modify",
			path.display()
		);
		return handle_modify(handler, path, config).await;
	}

	// 5. Check for inode-based move
	if let Some(inode) = metadata.inode {
		if let Some(existing) = handler.find_by_inode(inode).await? {
			if existing.path != path {
				tracing::debug!(
					"Detected inode-based move: {} -> {}",
					existing.path.display(),
					path.display()
				);
				let old_path = existing.path.clone();
				handler
					.move_entry(
						&existing,
						&old_path,
						path,
						path.parent().unwrap_or(Path::new("/")),
					)
					.await?;
				handler
					.emit_change_event(&existing, ChangeType::Moved)
					.await?;
				return Ok(());
			}
		}
	}

	// 6. Create entry
	let parent_path = path.parent().unwrap_or(Path::new("/"));
	let entry = handler.create(&metadata, parent_path).await?;

	// 7. Handle directory recursion or run processors
	if entry.is_directory() {
		handler.handle_new_directory(path).await?;
	} else {
		handler.run_processors(&entry, true).await?;
	}

	// 8. Emit event
	handler
		.emit_change_event(&entry, ChangeType::Created)
		.await?;

	Ok(())
}

/// Handle a modify event.
///
/// Updates existing entry metadata and re-runs processors for files.
pub async fn handle_modify<H: ChangeHandler>(
	handler: &mut H,
	path: &Path,
	config: &ChangeConfig<'_>,
) -> Result<()> {
	tracing::debug!("Modify: {}", path.display());

	// 1. Validate path exists
	match path_exists_safe(path, config.volume_backend).await {
		Ok(true) => {}
		Ok(false) => {
			tracing::debug!("Path no longer exists, skipping modify: {}", path.display());
			return Ok(());
		}
		Err(e) => {
			tracing::warn!(
				"Skipping modify event for inaccessible path {}: {}",
				path.display(),
				e
			);
			return Ok(());
		}
	}

	// 2. Apply rule filtering
	if should_filter_path(
		path,
		config.rule_toggles,
		config.location_root,
		config.volume_backend,
	)
	.await?
	{
		tracing::debug!("Skipping filtered path: {}", path.display());
		return Ok(());
	}

	// 3. Extract metadata
	let metadata = build_dir_entry(path, config.volume_backend).await?;

	// 4. Check for inode-based move
	if let Some(inode) = metadata.inode {
		if let Some(existing) = handler.find_by_inode(inode).await? {
			if existing.path != path {
				tracing::debug!(
					"Detected inode-based move during modify: {} -> {}",
					existing.path.display(),
					path.display()
				);
				let old_path = existing.path.clone();
				handler
					.move_entry(
						&existing,
						&old_path,
						path,
						path.parent().unwrap_or(Path::new("/")),
					)
					.await?;
				handler
					.emit_change_event(&existing, ChangeType::Moved)
					.await?;
				return Ok(());
			}
		}
	}

	// 5. Find and update entry
	if let Some(entry) = handler.find_by_path(path).await? {
		handler.update(&entry, &metadata).await?;

		// 6. Re-run processors for files
		if !entry.is_directory() {
			handler.run_processors(&entry, false).await?;
		}

		// 7. Emit event
		handler
			.emit_change_event(&entry, ChangeType::Modified)
			.await?;
	} else {
		tracing::debug!(
			"Entry not found for path, skipping modify: {}",
			path.display()
		);
	}

	Ok(())
}

/// Handle a remove event.
///
/// Deletes the entry and its entire subtree.
pub async fn handle_remove<H: ChangeHandler>(handler: &mut H, path: &Path) -> Result<()> {
	tracing::debug!("Remove: {}", path.display());

	if let Some(entry) = handler.find_by_path(path).await? {
		handler.delete(&entry).await?;
		handler
			.emit_change_event(&entry, ChangeType::Deleted)
			.await?;
		tracing::debug!("Deleted entry for path: {}", path.display());
	} else {
		tracing::debug!(
			"Entry not found for path, skipping remove: {}",
			path.display()
		);
	}

	Ok(())
}

/// Handle a rename event.
///
/// Moves an entry from one path to another, updating parent relationships.
pub async fn handle_rename<H: ChangeHandler>(
	handler: &mut H,
	from: &Path,
	to: &Path,
	config: &ChangeConfig<'_>,
) -> Result<()> {
	tracing::debug!("Rename: {} -> {}", from.display(), to.display());

	// 1. Validate destination exists
	match path_exists_safe(to, config.volume_backend).await {
		Ok(true) => {}
		Ok(false) => {
			tracing::debug!(
				"Destination path doesn't exist, skipping rename: {}",
				to.display()
			);
			return Ok(());
		}
		Err(e) => {
			tracing::warn!(
				"Skipping rename event for inaccessible destination {}: {}",
				to.display(),
				e
			);
			return Ok(());
		}
	}

	// 2. Check if destination is filtered (treat as deletion)
	if should_filter_path(
		to,
		config.rule_toggles,
		config.location_root,
		config.volume_backend,
	)
	.await?
	{
		tracing::debug!(
			"Destination path is filtered, removing entry: {}",
			to.display()
		);
		return handle_remove(handler, from).await;
	}

	// 3. Find source entry and move
	if let Some(entry) = handler.find_by_path(from).await? {
		handler
			.move_entry(&entry, from, to, to.parent().unwrap_or(Path::new("/")))
			.await?;
		handler.emit_change_event(&entry, ChangeType::Moved).await?;
		tracing::debug!("Moved entry {} -> {}", from.display(), to.display());
	} else {
		tracing::debug!(
			"Entry not found for old path {}, skipping rename",
			from.display()
		);
	}

	Ok(())
}

// ============================================================================
// Persistent Change Handler (Database-backed)
// ============================================================================

use crate::context::CoreContext;
use crate::infra::db::entities;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QuerySelect};

/// Database-backed change handler for managed locations.
///
/// Uses EntryProcessor for CRUD operations and maintains closure table
/// relationships. Runs processor pipeline (thumbnails, content hash) for
/// new and modified files.
pub struct PersistentChangeHandler {
	context: Arc<CoreContext>,
	library_id: Uuid,
	location_id: Uuid,
	location_root_entry_id: i32,
	db: sea_orm::DatabaseConnection,
	/// Volume backend for this location
	volume_backend: Option<Arc<dyn crate::volume::VolumeBackend>>,
	/// Entry ID cache for parent lookups
	entry_id_cache: std::collections::HashMap<PathBuf, i32>,
}

impl PersistentChangeHandler {
	pub async fn new(
		context: Arc<CoreContext>,
		library_id: Uuid,
		location_id: Uuid,
		location_root: &Path,
		volume_backend: Option<Arc<dyn crate::volume::VolumeBackend>>,
	) -> Result<Self> {
		let library = context
			.get_library(library_id)
			.await
			.ok_or_else(|| anyhow::anyhow!("Library not found: {}", library_id))?;

		let db = library.db().conn().clone();

		// Get location's root entry_id
		let location_record = entities::location::Entity::find()
			.filter(entities::location::Column::Uuid.eq(location_id))
			.one(&db)
			.await?
			.ok_or_else(|| anyhow::anyhow!("Location not found: {}", location_id))?;

		let location_root_entry_id = location_record
			.entry_id
			.ok_or_else(|| anyhow::anyhow!("Location {} has no root entry", location_id))?;

		Ok(Self {
			context,
			library_id,
			location_id,
			location_root_entry_id,
			db,
			volume_backend,
			entry_id_cache: std::collections::HashMap::new(),
		})
	}

	/// Resolve entry ID by path, checking directories then files.
	async fn resolve_entry_id(&self, path: &Path) -> Result<Option<i32>> {
		// Try directory lookup first
		if let Some(id) = self.resolve_directory_entry_id(path).await? {
			return Ok(Some(id));
		}
		// Try file lookup
		self.resolve_file_entry_id(path).await
	}

	async fn resolve_directory_entry_id(&self, path: &Path) -> Result<Option<i32>> {
		use sea_orm::FromQueryResult;

		let path_str = path.to_string_lossy().to_string();

		#[derive(Debug, FromQueryResult)]
		struct DirectoryEntryId {
			entry_id: i32,
		}

		let result = DirectoryEntryId::find_by_statement(sea_orm::Statement::from_sql_and_values(
			sea_orm::DbBackend::Sqlite,
			r#"
			SELECT dp.entry_id
			FROM directory_paths dp
			INNER JOIN entry_closure ec ON ec.descendant_id = dp.entry_id
			WHERE dp.path = ?
			  AND ec.ancestor_id = ?
			"#,
			vec![path_str.into(), self.location_root_entry_id.into()],
		))
		.one(&self.db)
		.await?;

		Ok(result.map(|r| r.entry_id))
	}

	async fn resolve_file_entry_id(&self, path: &Path) -> Result<Option<i32>> {
		let parent = match path.parent() {
			Some(p) => p,
			None => return Ok(None),
		};

		let parent_id = match self.resolve_directory_entry_id(parent).await? {
			Some(id) => id,
			None => return Ok(None),
		};

		let name = path
			.file_stem()
			.and_then(|s| s.to_str())
			.unwrap_or("")
			.to_string();
		let ext = path
			.extension()
			.and_then(|s| s.to_str())
			.map(|s| s.to_lowercase());

		let mut q = entities::entry::Entity::find()
			.filter(entities::entry::Column::ParentId.eq(parent_id))
			.filter(entities::entry::Column::Name.eq(name));

		if let Some(e) = ext {
			q = q.filter(entities::entry::Column::Extension.eq(e));
		} else {
			q = q.filter(entities::entry::Column::Extension.is_null());
		}

		let model = q.one(&self.db).await?;
		Ok(model.map(|m| m.id))
	}
}

#[async_trait::async_trait]
impl ChangeHandler for PersistentChangeHandler {
	async fn find_by_path(&self, path: &Path) -> Result<Option<EntryRef>> {
		let entry_id = match self.resolve_entry_id(path).await? {
			Some(id) => id,
			None => return Ok(None),
		};

		let entry = entities::entry::Entity::find_by_id(entry_id)
			.one(&self.db)
			.await?
			.ok_or_else(|| anyhow::anyhow!("Entry {} not found after ID lookup", entry_id))?;

		let kind = match entry.kind {
			0 => EntryKind::File,
			1 => EntryKind::Directory,
			2 => EntryKind::Symlink,
			_ => EntryKind::File,
		};

		Ok(Some(EntryRef {
			id: entry.id,
			uuid: entry.uuid,
			path: path.to_path_buf(),
			kind,
		}))
	}

	async fn find_by_inode(&self, inode: u64) -> Result<Option<EntryRef>> {
		let inode_val = inode as i64;

		let entry = entities::entry::Entity::find()
			.filter(entities::entry::Column::Inode.eq(inode_val))
			.one(&self.db)
			.await?;

		match entry {
			Some(e) => {
				let full_path = super::PathResolver::get_full_path(&self.db, e.id)
					.await
					.unwrap_or_else(|_| std::path::PathBuf::from(&e.name));

				let kind = match e.kind {
					0 => EntryKind::File,
					1 => EntryKind::Directory,
					2 => EntryKind::Symlink,
					_ => EntryKind::File,
				};

				Ok(Some(EntryRef {
					id: e.id,
					uuid: e.uuid,
					path: full_path,
					kind,
				}))
			}
			None => Ok(None),
		}
	}

	async fn create(&mut self, metadata: &DirEntry, parent_path: &Path) -> Result<EntryRef> {
		use super::entry::EntryProcessor;
		use super::state::IndexerState;
		use crate::domain::addressing::SdPath;

		// Create minimal state for entry creation
		let mut state = IndexerState::new(&SdPath::local(&metadata.path));

		// Seed parent cache if we have it
		if let Some(&parent_id) = self.entry_id_cache.get(parent_path) {
			state
				.entry_id_cache
				.insert(parent_path.to_path_buf(), parent_id);
		} else if let Some(parent_id) = self.resolve_directory_entry_id(parent_path).await? {
			state
				.entry_id_cache
				.insert(parent_path.to_path_buf(), parent_id);
			self.entry_id_cache
				.insert(parent_path.to_path_buf(), parent_id);
		}

		// Use ResponderCtx for the IndexingCtx trait
		let ctx = super::ctx::ResponderCtx::new(&self.context, self.library_id).await?;

		let entry_id = EntryProcessor::create_entry(&mut state, &ctx, metadata, 0, parent_path)
			.await
			.map_err(|e| anyhow::anyhow!("Failed to create entry: {}", e))?;

		// Cache the new entry
		self.entry_id_cache.insert(metadata.path.clone(), entry_id);

		// Get the created entry for the response
		let entry = entities::entry::Entity::find_by_id(entry_id)
			.one(&self.db)
			.await?
			.ok_or_else(|| anyhow::anyhow!("Entry not found after creation"))?;

		Ok(EntryRef {
			id: entry.id,
			uuid: entry.uuid,
			path: metadata.path.clone(),
			kind: metadata.kind,
		})
	}

	async fn update(&mut self, entry: &EntryRef, metadata: &DirEntry) -> Result<()> {
		use super::entry::EntryProcessor;

		let ctx = super::ctx::ResponderCtx::new(&self.context, self.library_id).await?;
		EntryProcessor::update_entry(&ctx, entry.id, metadata)
			.await
			.map_err(|e| anyhow::anyhow!("Failed to update entry: {}", e))?;

		Ok(())
	}

	async fn move_entry(
		&mut self,
		entry: &EntryRef,
		old_path: &Path,
		new_path: &Path,
		new_parent_path: &Path,
	) -> Result<()> {
		use super::entry::EntryProcessor;
		use super::state::IndexerState;
		use crate::domain::addressing::SdPath;

		let mut state = IndexerState::new(&SdPath::local(old_path));

		// Seed parent cache
		if let Some(&parent_id) = self.entry_id_cache.get(new_parent_path) {
			state
				.entry_id_cache
				.insert(new_parent_path.to_path_buf(), parent_id);
		} else if let Some(parent_id) = self.resolve_directory_entry_id(new_parent_path).await? {
			state
				.entry_id_cache
				.insert(new_parent_path.to_path_buf(), parent_id);
			self.entry_id_cache
				.insert(new_parent_path.to_path_buf(), parent_id);
		}

		let ctx = super::ctx::ResponderCtx::new(&self.context, self.library_id).await?;
		EntryProcessor::move_entry(
			&mut state,
			&ctx,
			entry.id,
			old_path,
			new_path,
			new_parent_path,
		)
		.await
		.map_err(|e| anyhow::anyhow!("Failed to move entry: {}", e))?;

		// Update cache
		self.entry_id_cache.remove(old_path);
		self.entry_id_cache.insert(new_path.to_path_buf(), entry.id);

		Ok(())
	}

	async fn delete(&mut self, entry: &EntryRef) -> Result<()> {
		use sea_orm::TransactionTrait;

		// Collect all descendants
		let mut to_delete_ids: Vec<i32> = vec![entry.id];

		if let Ok(rows) = entities::entry_closure::Entity::find()
			.filter(entities::entry_closure::Column::AncestorId.eq(entry.id))
			.all(&self.db)
			.await
		{
			to_delete_ids.extend(rows.into_iter().map(|r| r.descendant_id));
		}

		// Also traverse via parent_id as fallback
		let mut queue = vec![entry.id];
		let mut visited = std::collections::HashSet::from([entry.id]);

		while let Some(parent) = queue.pop() {
			if let Ok(children) = entities::entry::Entity::find()
				.filter(entities::entry::Column::ParentId.eq(parent))
				.all(&self.db)
				.await
			{
				for child in children {
					if visited.insert(child.id) {
						to_delete_ids.push(child.id);
						queue.push(child.id);
					}
				}
			}
		}

		to_delete_ids.sort_unstable();
		to_delete_ids.dedup();

		// Create tombstones for sync
		let entries_to_delete = if !to_delete_ids.is_empty() {
			let mut all_entries = Vec::new();
			for chunk in to_delete_ids.chunks(900) {
				let batch = entities::entry::Entity::find()
					.filter(entities::entry::Column::Id.is_in(chunk.to_vec()))
					.all(&self.db)
					.await?;
				all_entries.extend(batch);
			}
			all_entries
		} else {
			Vec::new()
		};

		if !entries_to_delete.is_empty() {
			if let Some(library) = self.context.get_library(self.library_id).await {
				let _ = library
					.sync_models_batch(
						&entries_to_delete,
						crate::infra::sync::ChangeType::Delete,
						&self.db,
					)
					.await;
			}
		}

		// Delete in transaction
		let txn = self.db.begin().await?;

		if !to_delete_ids.is_empty() {
			let _ = entities::entry_closure::Entity::delete_many()
				.filter(entities::entry_closure::Column::DescendantId.is_in(to_delete_ids.clone()))
				.exec(&txn)
				.await;
			let _ = entities::entry_closure::Entity::delete_many()
				.filter(entities::entry_closure::Column::AncestorId.is_in(to_delete_ids.clone()))
				.exec(&txn)
				.await;
			let _ = entities::directory_paths::Entity::delete_many()
				.filter(entities::directory_paths::Column::EntryId.is_in(to_delete_ids.clone()))
				.exec(&txn)
				.await;
			let _ = entities::entry::Entity::delete_many()
				.filter(entities::entry::Column::Id.is_in(to_delete_ids))
				.exec(&txn)
				.await;
		}

		txn.commit().await?;

		// Clear from cache
		self.entry_id_cache.remove(&entry.path);

		Ok(())
	}

	async fn run_processors(&self, entry: &EntryRef, _is_new: bool) -> Result<()> {
		use super::processor::{
			load_location_processor_config, ContentHashProcessor, ProcessorEntry,
		};
		use crate::ops::media::thumbnail::ThumbnailProcessor;

		if entry.is_directory() {
			return Ok(());
		}

		let Some(library) = self.context.get_library(self.library_id).await else {
			return Ok(());
		};

		let proc_config = load_location_processor_config(self.location_id, &self.db)
			.await
			.unwrap_or_default();

		// Build processor entry
		let db_entry = entities::entry::Entity::find_by_id(entry.id)
			.one(&self.db)
			.await?
			.ok_or_else(|| anyhow::anyhow!("Entry not found"))?;

		let mime_type = if let Some(content_id) = db_entry.content_id {
			if let Ok(Some(ci)) = entities::content_identity::Entity::find_by_id(content_id)
				.one(&self.db)
				.await
			{
				if let Some(mime_id) = ci.mime_type_id {
					if let Ok(Some(mime)) = entities::mime_type::Entity::find_by_id(mime_id)
						.one(&self.db)
						.await
					{
						Some(mime.mime_type)
					} else {
						None
					}
				} else {
					None
				}
			} else {
				None
			}
		} else {
			None
		};

		let proc_entry = ProcessorEntry {
			id: entry.id,
			uuid: entry.uuid,
			path: entry.path.clone(),
			kind: entry.kind,
			size: db_entry.size as u64,
			content_id: db_entry.content_id,
			mime_type,
		};

		let ctx = super::ctx::ResponderCtx::new(&self.context, self.library_id).await?;

		// Content hash
		if proc_config
			.watcher_processors
			.iter()
			.any(|c| c.processor_type == "content_hash" && c.enabled)
		{
			let content_proc = ContentHashProcessor::new(self.library_id);
			if let Err(e) = content_proc.process(&ctx, &proc_entry).await {
				tracing::warn!("Content hash processing failed: {}", e);
			}
		}

		// Thumbnail
		if proc_config
			.watcher_processors
			.iter()
			.any(|c| c.processor_type == "thumbnail" && c.enabled)
		{
			let thumb_proc = ThumbnailProcessor::new(library.clone());
			if thumb_proc.should_process(&proc_entry) {
				if let Err(e) = thumb_proc.process(&self.db, &proc_entry).await {
					tracing::warn!("Thumbnail processing failed: {}", e);
				}
			}
		}

		Ok(())
	}

	async fn emit_change_event(&self, entry: &EntryRef, change_type: ChangeType) -> Result<()> {
		use crate::domain::ResourceManager;

		if let Some(uuid) = entry.uuid {
			let resource_manager =
				ResourceManager::new(Arc::new(self.db.clone()), self.context.events.clone());

			if let Err(e) = resource_manager
				.emit_resource_events("entry", vec![uuid])
				.await
			{
				tracing::warn!(
					"Failed to emit resource event for {:?} entry: {}",
					change_type,
					e
				);
			}
		}

		Ok(())
	}

	async fn handle_new_directory(&self, path: &Path) -> Result<()> {
		use super::job::{IndexMode, IndexerJob};
		use crate::domain::addressing::SdPath;

		let Some(library) = self.context.get_library(self.library_id).await else {
			return Ok(());
		};

		// Get index mode from location
		let index_mode = if let Ok(Some(loc)) = entities::location::Entity::find()
			.filter(entities::location::Column::Uuid.eq(self.location_id))
			.one(&self.db)
			.await
		{
			match loc.index_mode.as_str() {
				"shallow" => IndexMode::Shallow,
				"content" => IndexMode::Content,
				"deep" => IndexMode::Deep,
				_ => IndexMode::Content,
			}
		} else {
			IndexMode::Content
		};

		let indexer_job =
			IndexerJob::from_location(self.location_id, SdPath::local(path), index_mode);

		if let Err(e) = library.jobs().dispatch(indexer_job).await {
			tracing::warn!(
				"Failed to spawn indexer job for directory {}: {}",
				path.display(),
				e
			);
		} else {
			tracing::debug!(
				"Spawned recursive indexer job for directory: {}",
				path.display()
			);
		}

		Ok(())
	}
}

// ============================================================================
// Ephemeral Change Handler (Memory-backed)
// ============================================================================

use super::job::EphemeralIndex;
use tokio::sync::RwLock;

/// Memory-backed change handler for ephemeral browsing.
///
/// Updates the EphemeralIndex directly without database writes.
/// Skips processor pipeline (no thumbnails/content hash for ephemeral).
pub struct EphemeralChangeHandler {
	index: Arc<RwLock<EphemeralIndex>>,
	event_bus: Arc<crate::infra::event::EventBus>,
	root_path: PathBuf,
	/// Synthetic ID counter (EphemeralIndex uses arena indices internally)
	next_id: std::sync::atomic::AtomicI32,
}

impl EphemeralChangeHandler {
	pub fn new(
		index: Arc<RwLock<EphemeralIndex>>,
		event_bus: Arc<crate::infra::event::EventBus>,
		root_path: PathBuf,
	) -> Self {
		Self {
			index,
			event_bus,
			root_path,
			next_id: std::sync::atomic::AtomicI32::new(1),
		}
	}

	fn next_id(&self) -> i32 {
		self.next_id
			.fetch_add(1, std::sync::atomic::Ordering::SeqCst)
	}
}

#[async_trait::async_trait]
impl ChangeHandler for EphemeralChangeHandler {
	async fn find_by_path(&self, path: &Path) -> Result<Option<EntryRef>> {
		let index = self.index.read().await;

		if let Some(metadata) = index.get_entry_ref(&path.to_path_buf()) {
			let uuid = index.get_entry_uuid(&path.to_path_buf());

			Ok(Some(EntryRef {
				id: 0, // Ephemeral entries don't have stable IDs
				uuid,
				path: path.to_path_buf(),
				kind: metadata.kind,
			}))
		} else {
			Ok(None)
		}
	}

	async fn find_by_inode(&self, _inode: u64) -> Result<Option<EntryRef>> {
		// Ephemeral index doesn't track inodes
		Ok(None)
	}

	async fn create(&mut self, metadata: &DirEntry, _parent_path: &Path) -> Result<EntryRef> {
		use super::entry::EntryMetadata;

		let entry_uuid = Uuid::new_v4();
		let entry_metadata = EntryMetadata::from(metadata.clone());

		{
			let mut index = self.index.write().await;
			index
				.add_entry(metadata.path.clone(), entry_uuid, entry_metadata)
				.map_err(|e| anyhow::anyhow!("Failed to add entry to ephemeral index: {}", e))?;
		}

		Ok(EntryRef {
			id: self.next_id(),
			uuid: Some(entry_uuid),
			path: metadata.path.clone(),
			kind: metadata.kind,
		})
	}

	async fn update(&mut self, entry: &EntryRef, metadata: &DirEntry) -> Result<()> {
		use super::entry::EntryMetadata;

		// Ephemeral index doesn't have a direct update method,
		// so we remove and re-add (preserving UUID)
		let uuid = entry.uuid.unwrap_or_else(Uuid::new_v4);
		let entry_metadata = EntryMetadata::from(metadata.clone());

		{
			let mut index = self.index.write().await;
			// The add_entry method handles duplicates by returning Ok(None)
			// For updates, we need to clear first then re-add
			// Since EphemeralIndex doesn't have remove_entry, we just re-add
			// which effectively updates the metadata
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
		// Ephemeral index doesn't support moves directly
		// We delete from old path and create at new path
		// Note: This loses the UUID association, but for ephemeral that's acceptable

		let metadata = build_dir_entry(new_path, None).await?;

		{
			let mut index = self.index.write().await;
			// Remove old entry
			index.remove_entry(old_path);

			// Add at new path with preserved UUID
			let uuid = entry.uuid.unwrap_or_else(Uuid::new_v4);
			let entry_metadata = super::entry::EntryMetadata::from(metadata.clone());
			let _ = index.add_entry(new_path.to_path_buf(), uuid, entry_metadata);
		}

		Ok(())
	}

	async fn delete(&mut self, entry: &EntryRef) -> Result<()> {
		{
			let mut index = self.index.write().await;

			if entry.is_directory() {
				// Remove directory and all descendants
				index.remove_directory_tree(&entry.path);
			} else {
				// Remove single entry
				index.remove_entry(&entry.path);
			}
		}

		Ok(())
	}

	async fn run_processors(&self, _entry: &EntryRef, _is_new: bool) -> Result<()> {
		// Ephemeral handler skips processors - no thumbnails or content hash
		Ok(())
	}

	async fn emit_change_event(&self, entry: &EntryRef, change_type: ChangeType) -> Result<()> {
		use crate::device::get_current_device_slug;
		use crate::domain::addressing::SdPath;
		use crate::domain::file::File;
		use crate::domain::ContentKind;
		use crate::infra::event::{Event, ResourceMetadata};

		let Some(uuid) = entry.uuid else {
			return Ok(());
		};

		let device_slug = get_current_device_slug();

		let sd_path = SdPath::Physical {
			device_slug: device_slug.clone(),
			path: entry.path.clone(),
		};

		// Get content kind from index
		let content_kind = {
			let index = self.index.read().await;
			index.get_content_kind(&entry.path)
		};

		// Build a minimal File for the event
		let metadata = build_dir_entry(&entry.path, None).await.ok();

		if let Some(meta) = metadata {
			let entry_metadata = super::entry::EntryMetadata::from(meta);
			let mut file = File::from_ephemeral(uuid, &entry_metadata, sd_path);
			file.content_kind = content_kind;

			let parent_path = entry.path.parent().map(|p| SdPath::Physical {
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

		Ok(())
	}

	async fn handle_new_directory(&self, path: &Path) -> Result<()> {
		// For ephemeral, we do inline shallow indexing instead of spawning a job
		use super::entry::EntryMetadata;
		use super::entry::EntryProcessor;

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
					inode: EntryProcessor::get_inode(&metadata),
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

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_entry_ref_is_directory() {
		let file_ref = EntryRef {
			id: 1,
			uuid: Some(Uuid::new_v4()),
			path: PathBuf::from("/test/file.txt"),
			kind: EntryKind::File,
		};
		assert!(!file_ref.is_directory());

		let dir_ref = EntryRef {
			id: 2,
			uuid: Some(Uuid::new_v4()),
			path: PathBuf::from("/test/dir"),
			kind: EntryKind::Directory,
		};
		assert!(dir_ref.is_directory());
	}
}
