//! Change Detection Responder (function-style)
//!
//! Translates raw filesystem events into database-backed operations using the
//! indexing module. The watcher emits path-only events; this module resolves
//! real entry IDs and performs identity-preserving updates.

use crate::context::CoreContext;
use crate::infra::db::entities;
use crate::infra::event::FsRawEventKind;
use crate::ops::indexing::entry::EntryProcessor;
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

/// Handle create: extract metadata and insert via EntryProcessor
async fn handle_create(ctx: &impl IndexingCtx, path: &Path) -> Result<()> {
	debug!("Create: {}", path.display());
	let dir_entry = build_dir_entry(path).await?;

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
	if let Some(entry_id) = resolve_entry_id_by_path(ctx, path).await? {
		let dir_entry = build_dir_entry(path).await?;
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
		let mut state = IndexerState::new(&crate::domain::addressing::SdPath::local(from));
		EntryProcessor::move_entry(
			&mut state,
			ctx,
			entry_id,
			from,
			to,
			to.parent().unwrap_or_else(|| Path::new("/")),
		)
		.await?;
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

/// Resolve a directory entry by exact cached path in directory_paths
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
