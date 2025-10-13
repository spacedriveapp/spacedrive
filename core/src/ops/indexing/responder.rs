//! Change Detection Responder (function-style)
//!
//! Translates raw filesystem events into database-backed operations using the
//! indexing module. The watcher emits path-only events; this module resolves
//! real entry IDs and performs identity-preserving updates.

use crate::context::CoreContext;
use crate::infra::db::entities;
use crate::infra::event::FsRawEventKind;
use crate::ops::indexing::entry::EntryProcessor;
use crate::ops::indexing::path_resolver::PathResolver;
use crate::ops::indexing::state::{DirEntry, IndexerState};
use crate::ops::indexing::{ctx::ResponderCtx, IndexingCtx};
use anyhow::Result;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, TransactionTrait};
use std::path::Path;
use std::sync::Arc;
use tracing::debug;
use uuid::Uuid;

/// Apply a raw FS change by resolving it to DB operations (create/modify/move/delete)
pub async fn apply(
	context: &Arc<CoreContext>,
	library_id: Uuid,
	kind: FsRawEventKind,
) -> Result<()> {
	// Lightweight indexing context for DB access
	let ctx = ResponderCtx::new(context, library_id).await?;

	match kind {
		FsRawEventKind::Create { path } => handle_create(&ctx, &path).await?,
		FsRawEventKind::Modify { path } => handle_modify(&ctx, &path).await?,
		FsRawEventKind::Remove { path } => handle_remove(&ctx, &path).await?,
		FsRawEventKind::Rename { from, to } => handle_rename(&ctx, &from, &to).await?,
	}
	Ok(())
}

/// Apply a batch of raw FS changes with optimized processing
pub async fn apply_batch(
	context: &Arc<CoreContext>,
	library_id: Uuid,
	events: Vec<FsRawEventKind>,
) -> Result<()> {
	if events.is_empty() {
		return Ok(());
	}

	// Lightweight indexing context for DB access
	let ctx = ResponderCtx::new(context, library_id).await?;

	// Group events by type for potential bulk operations
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

	// Process in order: removes first, then renames, then creates, then modifies
	// This ensures we don't try to create files that should be removed, etc.

	// Process removes
	for path in removes {
		if let Err(e) = handle_remove(&ctx, &path).await {
			tracing::error!("Failed to handle remove for {}: {}", path.display(), e);
		}
	}

	// Process renames
	for (from, to) in renames {
		if let Err(e) = handle_rename(&ctx, &from, &to).await {
			tracing::error!(
				"Failed to handle rename from {} to {}: {}",
				from.display(),
				to.display(),
				e
			);
		}
	}

	// Process creates
	for path in creates {
		if let Err(e) = handle_create(&ctx, &path).await {
			tracing::error!("Failed to handle create for {}: {}", path.display(), e);
		}
	}

	// Process modifies
	for path in modifies {
		if let Err(e) = handle_modify(&ctx, &path).await {
			tracing::error!("Failed to handle modify for {}: {}", path.display(), e);
		}
	}

	Ok(())
}

/// Handle create: extract metadata and insert via EntryProcessor
async fn handle_create(ctx: &impl IndexingCtx, path: &Path) -> Result<()> {
	debug!("Create: {}", path.display());
	let dir_entry = build_dir_entry(path).await?;

	// If inode matches an existing entry at another path, treat this as a move
	if handle_move_by_inode(ctx, path, dir_entry.inode).await? {
		return Ok(());
	}

	// Minimal state provides parent cache used by EntryProcessor
	let mut state = IndexerState::new(&crate::domain::addressing::SdPath::local(path));
	let _ = EntryProcessor::create_entry(
		&mut state,
		ctx,
		&dir_entry,
		0, // device_id not needed here
		path.parent().unwrap_or_else(|| Path::new("/")),
	)
	.await?;
	Ok(())
}

/// Handle modify: resolve entry ID by path, then update
async fn handle_modify(ctx: &impl IndexingCtx, path: &Path) -> Result<()> {
	debug!("Modify: {}", path.display());

	// If inode indicates a move, handle as a move and skip update
	let meta = EntryProcessor::extract_metadata(path).await?;
	if handle_move_by_inode(ctx, path, meta.inode).await? {
		return Ok(());
	}

	if let Some(entry_id) = resolve_entry_id_by_path(ctx, path).await? {
		let dir_entry = DirEntry {
			path: meta.path,
			kind: meta.kind,
			size: meta.size,
			modified: meta.modified,
			inode: meta.inode,
		};
		EntryProcessor::update_entry(ctx, entry_id, &dir_entry).await?;
	}
	Ok(())
}

/// Handle remove: resolve entry ID and delete subtree (closure table + cache)
async fn handle_remove(ctx: &impl IndexingCtx, path: &Path) -> Result<()> {
	debug!("Remove: {}", path.display());
	if let Some(entry_id) = resolve_entry_id_by_path(ctx, path).await? {
		delete_subtree(ctx, entry_id).await?;
	}
	Ok(())
}

/// Handle rename/move: resolve source entry and move via EntryProcessor
async fn handle_rename(ctx: &impl IndexingCtx, from: &Path, to: &Path) -> Result<()> {
	debug!("Rename: {} -> {}", from.display(), to.display());
	if let Some(entry_id) = resolve_entry_id_by_path(ctx, from).await? {
		debug!("Found entry {} for old path, moving to new path", entry_id);

		// Create state and populate entry_id_cache with parent directories
		let mut state = IndexerState::new(&crate::domain::addressing::SdPath::local(from));

		// Populate cache with new parent directory if it exists
		if let Some(new_parent_path) = to.parent() {
			if let Ok(Some(parent_id)) = resolve_directory_entry_id(ctx, new_parent_path).await {
				state
					.entry_id_cache
					.insert(new_parent_path.to_path_buf(), parent_id);
				debug!(
					"Populated parent cache: {} -> {}",
					new_parent_path.display(),
					parent_id
				);
			}
		}

		EntryProcessor::move_entry(
			&mut state,
			ctx,
			entry_id,
			from,
			to,
			to.parent().unwrap_or_else(|| Path::new("/")),
		)
		.await?;
		debug!("✓ Successfully moved entry {} to new path", entry_id);
	} else {
		debug!(
			"Entry not found for old path {}, skipping rename",
			from.display()
		);
	}
	Ok(())
}

/// Build a DirEntry from current filesystem metadata
async fn build_dir_entry(path: &Path) -> Result<DirEntry> {
	let meta = EntryProcessor::extract_metadata(path).await?;
	Ok(DirEntry {
		path: meta.path,
		kind: meta.kind,
		size: meta.size,
		modified: meta.modified,
		inode: meta.inode,
	})
}

/// Resolve an entry ID by absolute path (directory first, then file by parent/name/extension)
async fn resolve_entry_id_by_path(ctx: &impl IndexingCtx, abs_path: &Path) -> Result<Option<i32>> {
	if let Some(id) = resolve_directory_entry_id(ctx, abs_path).await? {
		return Ok(Some(id));
	}
	resolve_file_entry_id(ctx, abs_path).await
}

/// Resolve a directory entry by its full path in the directory_paths table
async fn resolve_directory_entry_id(
	ctx: &impl IndexingCtx,
	abs_path: &Path,
) -> Result<Option<i32>> {
	let path_str = abs_path.to_string_lossy().to_string();
	let model = entities::directory_paths::Entity::find()
		.filter(entities::directory_paths::Column::Path.eq(path_str))
		.one(ctx.library_db())
		.await?;
	Ok(model.map(|m| m.entry_id))
}

/// Resolve a file entry by parent directory path + file name (+ extension)
async fn resolve_file_entry_id(ctx: &impl IndexingCtx, abs_path: &Path) -> Result<Option<i32>> {
	let parent = match abs_path.parent() {
		Some(p) => p,
		None => return Ok(None),
	};
	let parent_str = parent.to_string_lossy().to_string();
	let parent_dir = match entities::directory_paths::Entity::find()
		.filter(entities::directory_paths::Column::Path.eq(parent_str))
		.one(ctx.library_db())
		.await?
	{
		Some(m) => m,
		None => return Ok(None),
	};

	let name = abs_path
		.file_stem()
		.and_then(|s| s.to_str())
		.unwrap_or("")
		.to_string();
	let ext = abs_path
		.extension()
		.and_then(|s| s.to_str())
		.map(|s| s.to_lowercase());

	let mut q = entities::entry::Entity::find()
		.filter(entities::entry::Column::ParentId.eq(parent_dir.entry_id))
		.filter(entities::entry::Column::Name.eq(name));
	if let Some(e) = ext {
		q = q.filter(entities::entry::Column::Extension.eq(e));
	} else {
		q = q.filter(entities::entry::Column::Extension.is_null());
	}
	let model = q.one(ctx.library_db()).await?;
	Ok(model.map(|m| m.id))
}

/// Best-effort deletion of an entry and its subtree
async fn delete_subtree(ctx: &impl IndexingCtx, entry_id: i32) -> Result<()> {
	let txn = ctx.library_db().begin().await?;
	let mut to_delete_ids: Vec<i32> = vec![entry_id];
	if let Ok(rows) = entities::entry_closure::Entity::find()
		.filter(entities::entry_closure::Column::AncestorId.eq(entry_id))
		.all(&txn)
		.await
	{
		to_delete_ids.extend(rows.into_iter().map(|r| r.descendant_id));
	}
	to_delete_ids.sort_unstable();
	to_delete_ids.dedup();
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
	Ok(())
}

/// Inode-aware move detection: if an existing entry has the same inode but a different path,
/// treat the change as a move and update the database accordingly.
async fn handle_move_by_inode(
	ctx: &impl IndexingCtx,
	new_path: &Path,
	inode: Option<u64>,
) -> Result<bool> {
	let inode_val = match inode {
		Some(i) if i != 0 => i as i64,
		_ => return Ok(false),
	};
	if let Some(existing) = entities::entry::Entity::find()
		.filter(entities::entry::Column::Inode.eq(inode_val))
		.one(ctx.library_db())
		.await?
	{
		// Resolve old full path
		let old_path = PathResolver::get_full_path(ctx.library_db(), existing.id)
			.await
			.unwrap_or_else(|_| std::path::PathBuf::from(&existing.name));
		if old_path != new_path {
			// File was moved to a different path
			let mut state = IndexerState::new(&crate::domain::addressing::SdPath::local(&old_path));
			EntryProcessor::move_entry(
				&mut state,
				ctx,
				existing.id,
				&old_path,
				new_path,
				new_path.parent().unwrap_or_else(|| Path::new("/")),
			)
			.await?;
			return Ok(true);
		} else {
			// Same path, same inode - this is a modification (macOS FSEvents reports as Create)
			// Update the existing entry instead of creating a duplicate
			debug!(
				"Entry already exists at path with same inode, updating instead of creating: {}",
				new_path.display()
			);
			let dir_entry = build_dir_entry(new_path).await?;
			EntryProcessor::update_entry(ctx, existing.id, &dir_entry).await?;
			return Ok(true);
		}
	}
	Ok(false)
}
