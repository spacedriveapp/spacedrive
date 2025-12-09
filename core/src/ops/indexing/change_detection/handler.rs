//! Change handler for responding to filesystem events.
//!
//! This module provides the `ChangeHandler` trait and shared logic for
//! processing filesystem changes. Both persistent (database) and ephemeral
//! (in-memory) handlers implement this trait.

use super::types::{ChangeConfig, ChangeType, EntryRef};
use crate::ops::indexing::rules::{build_default_ruler, RuleToggles, RulerDecision};
use crate::ops::indexing::state::{DirEntry, EntryKind};
use anyhow::Result;
use std::path::Path;
use std::sync::Arc;

/// Abstracts storage operations for filesystem change handling.
///
/// Both persistent (database) and ephemeral (in-memory) handlers implement
/// this trait, allowing the same change processing logic to work with both
/// storage backends.
#[async_trait::async_trait]
pub trait ChangeHandler: Send + Sync {
	/// Find an entry by its full filesystem path.
	async fn find_by_path(&self, path: &Path) -> Result<Option<EntryRef>>;

	/// Find an entry by inode (for move detection).
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

/// Check if a path exists, distinguishing between "doesn't exist" and "can't access".
///
/// Critical for preventing false deletions when volumes go offline.
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
	impl crate::ops::indexing::rules::MetadataForIndexerRules for SimpleMetadata {
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
	use crate::ops::indexing::database_storage::DatabaseStorage;

	let meta = DatabaseStorage::extract_metadata(path, backend).await?;
	Ok(DirEntry {
		path: meta.path,
		kind: meta.kind,
		size: meta.size,
		modified: meta.modified,
		inode: meta.inode,
	})
}

/// Apply a batch of filesystem changes using the provided handler.
///
/// Processes events in the correct order: removes first, then renames,
/// creates, and finally modifies.
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

	// Deduplicate
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
pub async fn handle_create<H: ChangeHandler>(
	handler: &mut H,
	path: &Path,
	config: &ChangeConfig<'_>,
) -> Result<()> {
	tracing::debug!("Create: {}", path.display());

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

	let metadata = build_dir_entry(path, config.volume_backend).await?;

	if handler.find_by_path(path).await?.is_some() {
		tracing::debug!(
			"Entry already exists at path {}, treating as modify",
			path.display()
		);
		return handle_modify(handler, path, config).await;
	}

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

	let parent_path = path.parent().unwrap_or(Path::new("/"));
	let entry = handler.create(&metadata, parent_path).await?;

	if entry.is_directory() {
		handler.handle_new_directory(path).await?;
	} else {
		handler.run_processors(&entry, true).await?;
	}

	handler
		.emit_change_event(&entry, ChangeType::Created)
		.await?;

	Ok(())
}

/// Handle a modify event.
pub async fn handle_modify<H: ChangeHandler>(
	handler: &mut H,
	path: &Path,
	config: &ChangeConfig<'_>,
) -> Result<()> {
	tracing::debug!("Modify: {}", path.display());

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

	let metadata = build_dir_entry(path, config.volume_backend).await?;

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

	if let Some(entry) = handler.find_by_path(path).await? {
		handler.update(&entry, &metadata).await?;

		if !entry.is_directory() {
			handler.run_processors(&entry, false).await?;
		}

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
pub async fn handle_rename<H: ChangeHandler>(
	handler: &mut H,
	from: &Path,
	to: &Path,
	config: &ChangeConfig<'_>,
) -> Result<()> {
	tracing::debug!("Rename: {} -> {}", from.display(), to.display());

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
