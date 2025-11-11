//! Change Detection Responder (function-style)
//!
//! Translates raw filesystem events into database-backed operations using the
//! indexing module. The watcher emits path-only events; this module resolves
//! real entry IDs and performs identity-preserving updates.

use crate::context::CoreContext;
use crate::domain::content_identity::ContentHashGenerator;
use crate::domain::ResourceManager;
use crate::infra::db::entities;
use crate::infra::event::FsRawEventKind;
use crate::ops::indexing::entry::EntryProcessor;
use crate::ops::indexing::path_resolver::PathResolver;
use crate::ops::indexing::rules::{build_default_ruler, RuleToggles, RulerDecision};
use crate::ops::indexing::state::{DirEntry, IndexerState};
use crate::ops::indexing::{ctx::ResponderCtx, IndexingCtx};
use anyhow::Result;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QuerySelect, TransactionTrait};
use std::path::Path;
use std::sync::Arc;
use tracing::{debug, warn};
use uuid::Uuid;

/// Apply a raw FS change by resolving it to DB operations (create/modify/move/delete)
pub async fn apply(
	context: &Arc<CoreContext>,
	library_id: Uuid,
	location_id: Uuid,
	kind: FsRawEventKind,
	rule_toggles: RuleToggles,
	location_root: &Path,
) -> Result<()> {
	// Lightweight indexing context for DB access
	let ctx = ResponderCtx::new(context, library_id).await?;

	match kind {
		FsRawEventKind::Create { path } => {
			handle_create(
				&ctx,
				context,
				library_id,
				location_id,
				&path,
				rule_toggles,
				location_root,
			)
			.await?
		}
		FsRawEventKind::Modify { path } => {
			handle_modify(&ctx, context, library_id, location_id, &path, rule_toggles, location_root).await?
		}
		FsRawEventKind::Remove { path } => handle_remove(&ctx, context, location_id, &path).await?,
		FsRawEventKind::Rename { from, to } => {
			handle_rename(&ctx, context, location_id, &from, &to, rule_toggles, location_root).await?
		}
	}
	Ok(())
}

/// Apply a batch of raw FS changes with optimized processing
pub async fn apply_batch(
	context: &Arc<CoreContext>,
	library_id: Uuid,
	location_id: Uuid,
	events: Vec<FsRawEventKind>,
	rule_toggles: RuleToggles,
	location_root: &Path,
) -> Result<()> {
	if events.is_empty() {
		return Ok(());
	}

	use std::sync::atomic::{AtomicU64, Ordering};
	static CALL_COUNTER: AtomicU64 = AtomicU64::new(0);
	let call_id = CALL_COUNTER.fetch_add(1, Ordering::SeqCst);

	debug!(
		"[BATCH #{}] Responder received batch of {} events for location {} (thread {:?})",
		call_id,
		events.len(),
		location_id,
		std::thread::current().id()
	);

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

	// Deduplicate events - macOS FSEvents can send duplicate Create events for the same file
	// when it's written in stages (common for screenshots, large files, etc)
	creates.sort();
	creates.dedup();
	modifies.sort();
	modifies.dedup();
	removes.sort();
	removes.dedup();

	// Process in order: removes first, then renames, then creates, then modifies
	// This ensures we don't try to create files that should be removed, etc.

	debug!(
		"Processing batch: {} creates, {} modifies, {} removes, {} renames",
		creates.len(),
		modifies.len(),
		removes.len(),
		renames.len()
	);

	// Process removes
	for path in removes {
		if let Err(e) = handle_remove(&ctx, context, location_id, &path).await {
			tracing::error!("Failed to handle remove for {}: {}", path.display(), e);
		}
	}

	// Process renames
	for (from, to) in renames {
		if let Err(e) =
			handle_rename(&ctx, context, location_id, &from, &to, rule_toggles, location_root).await
		{
			tracing::error!(
				"Failed to handle rename from {} to {}: {}",
				from.display(),
				to.display(),
				e
			);
		}
	}

	// Process creates
	for (idx, path) in creates.iter().enumerate() {
		debug!("[BATCH #{}] Processing create {}/{}: {}", call_id, idx + 1, creates.len(), path.display());
		if let Err(e) = handle_create(
			&ctx,
			context,
			library_id,
			location_id,
			&path,
			rule_toggles,
			location_root,
		)
		.await
		{
			tracing::error!("Failed to handle create for {}: {}", path.display(), e);
		}
		debug!("[BATCH #{}] Completed create {}/{}", call_id, idx + 1, creates.len());
	}

	// Process modifies
	for path in modifies {
		if let Err(e) = handle_modify(&ctx, context, library_id, location_id, &path, rule_toggles, location_root).await {
			tracing::error!("Failed to handle modify for {}: {}", path.display(), e);
		}
	}

	Ok(())
}

/// Get the location's root entry ID for scoping queries
async fn get_location_root_entry_id(ctx: &impl IndexingCtx, location_id: Uuid) -> Result<i32> {
	let location_record = entities::location::Entity::find()
		.filter(entities::location::Column::Uuid.eq(location_id))
		.one(ctx.library_db())
		.await?
		.ok_or_else(|| anyhow::anyhow!("Location not found: {}", location_id))?;

	location_record
		.entry_id
		.ok_or_else(|| anyhow::anyhow!("Location {} has no root entry", location_id))
}

/// Check if a path should be filtered based on indexing rules
async fn should_filter_path(
	path: &Path,
	rule_toggles: RuleToggles,
	location_root: &Path,
) -> Result<bool> {
	// Build ruler for this path using the same logic as the indexer
	let ruler = build_default_ruler(rule_toggles, location_root, path).await;

	// Get metadata for the path
	let metadata = tokio::fs::metadata(path).await?;

	// Simple metadata implementation for rule evaluation
	struct SimpleMetadata {
		is_dir: bool,
	}
	impl crate::ops::indexing::rules::MetadataForIndexerRules for SimpleMetadata {
		fn is_dir(&self) -> bool {
			self.is_dir
		}
	}

	let simple_meta = SimpleMetadata {
		is_dir: metadata.is_dir(),
	};

	// Evaluate the path against the ruler
	match ruler.evaluate_path(path, &simple_meta).await {
		Ok(RulerDecision::Reject) => {
			debug!("Filtered path by indexing rules: {}", path.display());
			Ok(true)
		}
		Ok(RulerDecision::Accept) => Ok(false),
		Err(e) => {
			tracing::warn!("Error evaluating rules for {}: {}", path.display(), e);
			Ok(false) // Don't filter on error, let it through
		}
	}
}

/// Handle create: extract metadata and insert via EntryProcessor
async fn handle_create(
	ctx: &impl IndexingCtx,
	context: &Arc<CoreContext>,
	library_id: Uuid,
	location_id: Uuid,
	path: &Path,
	rule_toggles: RuleToggles,
	location_root: &Path,
) -> Result<()> {
	debug!("Create: {}", path.display());

	// Check if path should be filtered
	if should_filter_path(path, rule_toggles, location_root).await? {
		debug!("✗ Skipping filtered path: {}", path.display());
		return Ok(());
	}

	debug!("→ Processing create for: {}", path.display());
	let dir_entry = build_dir_entry(path).await?;

	// Check if entry already exists at this exact path (race condition from duplicate watcher events)
	let location_root_entry_id = get_location_root_entry_id(ctx, location_id).await?;
	if let Some(existing_id) = resolve_entry_id_by_path_scoped(ctx, path, location_root_entry_id).await? {
		debug!(
			"Entry already exists at path {} (entry_id={}), treating as modify instead of create",
			path.display(),
			existing_id
		);
		// Treat as a modify instead
		return handle_modify(ctx, context, library_id, location_id, path, rule_toggles, location_root).await;
	}

	// If inode matches an existing entry at another path, treat this as a move
	if handle_move_by_inode(ctx, path, dir_entry.inode).await? {
		return Ok(());
	}

	// Minimal state provides parent cache used by EntryProcessor
	let mut state = IndexerState::new(&crate::domain::addressing::SdPath::local(path));

	// Seed the location root entry into cache to scope parent lookup
	// This ensures parents are found within THIS location's tree, not another device's location
	// with the same path. Without this, create_entry could attach to the wrong location's tree.
	if let Ok(Some(location_record)) = entities::location::Entity::find()
		.filter(entities::location::Column::Uuid.eq(location_id))
		.one(ctx.library_db())
		.await
	{
		if let Some(location_entry_id) = location_record.entry_id {
			state
				.entry_id_cache
				.insert(location_root.to_path_buf(), location_entry_id);
			debug!(
				"Seeded location root {} (entry {}) into cache for scoped parent lookup",
				location_root.display(),
				location_entry_id
			);
		}
	}

	let entry_id = EntryProcessor::create_entry(
		&mut state,
		ctx,
		&dir_entry,
		0, // device_id not needed here
		path.parent().unwrap_or_else(|| Path::new("/")),
	)
	.await?;

	debug!("✓ Created entry {} for path: {}", entry_id, path.display());

	// Get the entry UUID for event emission
	let entry_uuid = match entities::entry::Entity::find_by_id(entry_id)
		.one(ctx.library_db())
		.await?
	{
		Some(entry) => entry.uuid,
		None => None,
	};

	// If this is a directory, spawn a recursive indexer job to index its contents
	if dir_entry.kind == super::state::EntryKind::Directory {
		debug!(
			"Created directory detected, spawning recursive indexer job for: {}",
			path.display()
		);

		// Get the library to access the job manager
		if let Some(library) = context.get_library(library_id).await {
			// Create a recursive indexer job for this directory subtree
			let indexer_job = super::job::IndexerJob::from_location(
				location_id,
				crate::domain::addressing::SdPath::local(path),
				super::job::IndexMode::Content,
			);

			// Dispatch the job asynchronously (fire and forget)
			if let Err(e) = library.jobs().dispatch(indexer_job).await {
				warn!(
					"Failed to spawn indexer job for directory {}: {}",
					path.display(),
					e
				);
			} else {
				debug!(
					"✓ Spawned recursive indexer job for directory: {}",
					path.display()
				);
			}
		}
	} else {
		// For files, run content identification inline (single file is fast)
		debug!("→ Generating content hash for single file: {}", path.display());

		if let Ok(content_hash) = ContentHashGenerator::generate_content_hash(path).await {
			debug!("✓ Generated content hash: {}", content_hash);

			// Link the content identity
			if let Err(e) = EntryProcessor::link_to_content_identity(
				ctx,
				entry_id,
				path,
				content_hash,
				library_id,
			)
			.await
			{
				warn!("Failed to link content identity for {}: {}", path.display(), e);
			} else {
				debug!("✓ Linked content identity for entry {}", entry_id);
			}
		} else {
			debug!("✗ Failed to generate content hash for {}", path.display());
		}
	}

	// Emit resource event for the created entry
	if let Some(uuid) = entry_uuid {
		debug!("→ Emitting resource event for entry {}", uuid);
		let resource_manager = ResourceManager::new(
			Arc::new(ctx.library_db().clone()),
			context.events.clone(),
		);

		if let Err(e) = resource_manager
			.emit_resource_events("entry", vec![uuid])
			.await
		{
			warn!("Failed to emit resource event for created entry: {}", e);
		} else {
			debug!("✓ Emitted resource event for entry {}", uuid);
		}
	}

	Ok(())
}

/// Handle modify: resolve entry ID by path, then update
async fn handle_modify(
	ctx: &impl IndexingCtx,
	context: &Arc<CoreContext>,
	library_id: Uuid,
	location_id: Uuid,
	path: &Path,
	rule_toggles: RuleToggles,
	location_root: &Path,
) -> Result<()> {
	debug!("Modify: {}", path.display());

	// Check if path should be filtered
	if should_filter_path(path, rule_toggles, location_root).await? {
		debug!("✗ Skipping filtered path: {}", path.display());
		return Ok(());
	}

	debug!("→ Processing modify for: {}", path.display());

	// Get location root entry ID for scoped queries
	let location_root_entry_id = get_location_root_entry_id(ctx, location_id).await?;

	// If inode indicates a move, handle as a move and skip update
	// Responder uses direct filesystem access (None backend) since it reacts to local FS events
	let meta = EntryProcessor::extract_metadata(path, None).await?;
	if handle_move_by_inode(ctx, path, meta.inode).await? {
		return Ok(());
	}

	if let Some(entry_id) =
		resolve_entry_id_by_path_scoped(ctx, path, location_root_entry_id).await?
	{
		let dir_entry = DirEntry {
			path: meta.path.clone(),
			kind: meta.kind,
			size: meta.size,
			modified: meta.modified,
			inode: meta.inode,
		};
		EntryProcessor::update_entry(ctx, entry_id, &dir_entry).await?;
		debug!("✓ Updated entry {} for path: {}", entry_id, path.display());

		// Get entry UUID for event emission
		let entry_uuid = match entities::entry::Entity::find_by_id(entry_id)
			.one(ctx.library_db())
			.await?
		{
			Some(entry) => entry.uuid,
			None => None,
		};

		// For files, regenerate content hash if size changed
		if dir_entry.kind == super::state::EntryKind::File {
			debug!("→ Regenerating content hash for modified file: {}", path.display());

			if let Ok(content_hash) = ContentHashGenerator::generate_content_hash(path).await {
				debug!("✓ Generated content hash: {}", content_hash);

				if let Err(e) = EntryProcessor::link_to_content_identity(
					ctx,
					entry_id,
					path,
					content_hash,
					library_id,
				)
				.await
				{
					warn!("Failed to link content identity for {}: {}", path.display(), e);
				} else {
					debug!("✓ Linked content identity for entry {}", entry_id);
				}
			} else {
				debug!("✗ Failed to generate content hash for {}", path.display());
			}
		}

		// Emit resource event for the updated entry
		if let Some(uuid) = entry_uuid {
			debug!("→ Emitting resource event for modified entry {}", uuid);
			let resource_manager = ResourceManager::new(
				Arc::new(ctx.library_db().clone()),
				context.events.clone(),
			);

			if let Err(e) = resource_manager
				.emit_resource_events("entry", vec![uuid])
				.await
			{
				warn!("Failed to emit resource event for modified entry: {}", e);
			} else {
				debug!("✓ Emitted resource event for entry {}", uuid);
			}
		}
	} else {
		debug!("✗ Entry not found for path, skipping modify: {}", path.display());
	}
	Ok(())
}

/// Handle remove: resolve entry ID and delete subtree (closure table + cache)
async fn handle_remove(ctx: &impl IndexingCtx, context: &Arc<CoreContext>, location_id: Uuid, path: &Path) -> Result<()> {
	debug!("Remove: {}", path.display());

	// Get location root entry ID for scoped queries
	let location_root_entry_id = get_location_root_entry_id(ctx, location_id).await?;

	if let Some(entry_id) =
		resolve_entry_id_by_path_scoped(ctx, path, location_root_entry_id).await?
	{
		// Get entry UUID before deleting
		let entry_uuid = match entities::entry::Entity::find_by_id(entry_id)
			.one(ctx.library_db())
			.await?
		{
			Some(entry) => entry.uuid,
			None => None,
		};

		debug!("→ Deleting entry {} for path: {}", entry_id, path.display());
		delete_subtree(ctx, entry_id).await?;
		debug!("✓ Deleted entry {} for path: {}", entry_id, path.display());

		// Emit ResourceDeleted event if entry had a UUID
		if let Some(uuid) = entry_uuid {
			debug!("→ Emitting ResourceDeleted event for entry {}", uuid);
			context.events.emit(crate::infra::event::Event::ResourceDeleted {
				resource_type: "file".to_string(),
				resource_id: uuid,
			});
			debug!("✓ Emitted ResourceDeleted event for entry {}", uuid);
		}
	} else {
		debug!("✗ Entry not found for path, skipping remove: {}", path.display());
	}
	Ok(())
}

/// Handle rename/move: resolve source entry and move via EntryProcessor
async fn handle_rename(
	ctx: &impl IndexingCtx,
	context: &Arc<CoreContext>,
	location_id: Uuid,
	from: &Path,
	to: &Path,
	rule_toggles: RuleToggles,
	location_root: &Path,
) -> Result<()> {
	debug!("Rename: {} -> {}", from.display(), to.display());

	// Get location root entry ID for scoped queries
	let location_root_entry_id = get_location_root_entry_id(ctx, location_id).await?;

	// Check if the destination path should be filtered
	// If the file is being moved to a filtered location, we should remove it from the database
	if should_filter_path(to, rule_toggles, location_root).await? {
		debug!(
			"✗ Destination path is filtered, removing entry: {}",
			to.display()
		);
		// Treat this as a removal of the source file
		return handle_remove(ctx, context, location_id, from).await;
	}

	debug!("→ Processing rename for: {} -> {}", from.display(), to.display());

	if let Some(entry_id) =
		resolve_entry_id_by_path_scoped(ctx, from, location_root_entry_id).await?
	{
		debug!("Found entry {} for old path, moving to new path", entry_id);

		// Create state and populate entry_id_cache with parent directories
		let mut state = IndexerState::new(&crate::domain::addressing::SdPath::local(from));

		// Populate cache with new parent directory if it exists
		if let Some(new_parent_path) = to.parent() {
			if let Ok(Some(parent_id)) =
				resolve_directory_entry_id_scoped(ctx, new_parent_path, location_root_entry_id)
					.await
			{
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
	// Responder uses direct filesystem access (None backend) since it reacts to local FS events
	let meta = EntryProcessor::extract_metadata(path, None).await?;
	Ok(DirEntry {
		path: meta.path,
		kind: meta.kind,
		size: meta.size,
		modified: meta.modified,
		inode: meta.inode,
	})
}

/// Resolve an entry ID by absolute path, scoped to location's entry tree
async fn resolve_entry_id_by_path_scoped(
	ctx: &impl IndexingCtx,
	abs_path: &Path,
	location_root_entry_id: i32,
) -> Result<Option<i32>> {
	if let Some(id) =
		resolve_directory_entry_id_scoped(ctx, abs_path, location_root_entry_id).await?
	{
		return Ok(Some(id));
	}
	resolve_file_entry_id_scoped(ctx, abs_path, location_root_entry_id).await
}

/// Resolve a directory entry by path, scoped to location's entry tree using entry_closure
async fn resolve_directory_entry_id_scoped(
	ctx: &impl IndexingCtx,
	abs_path: &Path,
	location_root_entry_id: i32,
) -> Result<Option<i32>> {
	use sea_orm::FromQueryResult;

	let path_str = abs_path.to_string_lossy().to_string();

	// Query directory_paths and JOIN with entry_closure to scope by location
	// This ensures we only find entries within THIS location's tree
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
		vec![path_str.into(), location_root_entry_id.into()],
	))
	.one(ctx.library_db())
	.await?;

	Ok(result.map(|r| r.entry_id))
}

/// Resolve a file entry by parent directory path + file name, scoped to location's tree
async fn resolve_file_entry_id_scoped(
	ctx: &impl IndexingCtx,
	abs_path: &Path,
	location_root_entry_id: i32,
) -> Result<Option<i32>> {
	let parent = match abs_path.parent() {
		Some(p) => p,
		None => return Ok(None),
	};

	// First resolve parent directory using scoped lookup
	let parent_id =
		match resolve_directory_entry_id_scoped(ctx, parent, location_root_entry_id).await? {
			Some(id) => id,
			None => return Ok(None),
		};

	// Now find the file entry by parent + name + extension
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
		.filter(entities::entry::Column::ParentId.eq(parent_id))
		.filter(entities::entry::Column::Name.eq(name));
	if let Some(e) = ext {
		q = q.filter(entities::entry::Column::Extension.eq(e));
	} else {
		q = q.filter(entities::entry::Column::Extension.is_null());
	}
	let model = q.one(ctx.library_db()).await?;
	Ok(model.map(|m| m.id))
}

/// Best-effort deletion of an entry and its subtree (with tombstone creation)
///
/// This variant is used for local deletions (watcher, indexer) and creates
/// a tombstone for the root entry UUID to sync the deletion to other devices.
async fn delete_subtree(ctx: &impl IndexingCtx, entry_id: i32) -> Result<()> {
	let txn = ctx.library_db().begin().await?;

	// Get root entry UUID
	let root_entry = entities::entry::Entity::find_by_id(entry_id)
		.one(&txn)
		.await?
		.ok_or_else(|| anyhow::anyhow!("Entry not found: {}", entry_id))?;

	// Check if UUID is present (entries without UUIDs aren't sync-ready yet)
	let root_uuid = match root_entry.uuid {
		Some(uuid) => uuid,
		None => {
			// No UUID means not sync-ready, skip tombstone creation and just delete without tombstones
			tracing::debug!("Entry {} has no UUID, skipping tombstone", entry_id);
			return delete_subtree_no_txn(entry_id, &txn)
				.await
				.map_err(|e| anyhow::anyhow!(e));
		}
	};

	// Find the location this entry belongs to by finding a location with entry_id matching any ancestor
	// Walk up the tree to find the root entry (which should be the location's entry_id)
	let mut current_id = entry_id;
	let mut visited = std::collections::HashSet::new();
	let location = loop {
		visited.insert(current_id);

		// Try to find a location with this entry_id
		if let Some(loc) = entities::location::Entity::find()
			.filter(entities::location::Column::EntryId.eq(current_id))
			.one(&txn)
			.await?
		{
			break loc;
		}

		// Get parent entry
		let entry = entities::entry::Entity::find_by_id(current_id)
			.one(&txn)
			.await?
			.ok_or_else(|| anyhow::anyhow!("Entry not found during tree traversal"))?;

		match entry.parent_id {
			Some(parent) if !visited.contains(&parent) => current_id = parent,
			_ => {
				return Err(anyhow::anyhow!(
					"Could not find location for entry {}",
					entry_id
				))
			}
		}
	};

	let device_id = location.device_id;

	// Find all descendants using closure table
	let mut to_delete_ids: Vec<i32> = vec![entry_id];
	if let Ok(rows) = entities::entry_closure::Entity::find()
		.filter(entities::entry_closure::Column::AncestorId.eq(entry_id))
		.all(&txn)
		.await
	{
		to_delete_ids.extend(rows.into_iter().map(|r| r.descendant_id));
	}

	// IMPORTANT: Also find descendants by parent_id recursively as a fallback
	// This handles cases where the closure table is incomplete (e.g., race conditions during indexing)
	let mut queue = vec![entry_id];
	let mut visited = std::collections::HashSet::from([entry_id]);

	while let Some(parent) = queue.pop() {
		// Find all children of this parent
		if let Ok(children) = entities::entry::Entity::find()
			.filter(entities::entry::Column::ParentId.eq(parent))
			.all(&txn)
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

	tracing::debug!(
		"Deleting entry {} and {} descendants (total {} entries)",
		entry_id,
		to_delete_ids.len() - 1,
		to_delete_ids.len()
	);

	// Delete entries and related data
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

	// Create tombstone for root UUID only (children cascade on receiver)
	use sea_orm::ActiveValue::{NotSet, Set};
	let tombstone = entities::device_state_tombstone::ActiveModel {
		id: NotSet,
		model_type: Set("entry".to_string()),
		record_uuid: Set(root_uuid),
		device_id: Set(device_id),
		deleted_at: Set(chrono::Utc::now().into()),
	};

	use sea_orm::sea_query::OnConflict;
	entities::device_state_tombstone::Entity::insert(tombstone)
		.on_conflict(
			OnConflict::columns(vec![
				entities::device_state_tombstone::Column::ModelType,
				entities::device_state_tombstone::Column::RecordUuid,
				entities::device_state_tombstone::Column::DeviceId,
			])
			.do_nothing()
			.to_owned(),
		)
		.exec(&txn)
		.await?;

	txn.commit().await?;
	Ok(())
}

/// Best-effort deletion of an entry and its subtree (without tombstone creation)
///
/// This variant is used when applying deletion tombstones from sync to avoid
/// recursion. It performs the same deletions but does not create new tombstones.
pub async fn delete_subtree_internal(
	entry_id: i32,
	db: &sea_orm::DatabaseConnection,
) -> Result<(), sea_orm::DbErr> {
	use sea_orm::TransactionTrait;

	let txn = db.begin().await?;
	delete_subtree_no_txn(entry_id, &txn).await?;
	txn.commit().await?;
	Ok(())
}

/// Helper to delete subtree without transaction management (for use within existing transactions)
async fn delete_subtree_no_txn<C>(entry_id: i32, db: &C) -> Result<(), sea_orm::DbErr>
where
	C: sea_orm::ConnectionTrait,
{
	// Find all descendants
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

	// Delete entries and related data
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

	debug!("→ Checking inode {} for potential move detection", inode_val);

	if let Some(existing) = entities::entry::Entity::find()
		.filter(entities::entry::Column::Inode.eq(inode_val))
		.one(ctx.library_db())
		.await?
	{
		// Resolve old full path
		let old_path = PathResolver::get_full_path(ctx.library_db(), existing.id)
			.await
			.unwrap_or_else(|_| std::path::PathBuf::from(&existing.name));

		debug!(
			"Found existing entry {} (uuid={:?}) with inode {}: old_path={}, new_path={}",
			existing.id,
			existing.uuid,
			inode_val,
			old_path.display(),
			new_path.display()
		);

		if old_path != new_path {
			// File was moved to a different path
			debug!(
				"✓ Detected inode-based move: {} → {}",
				old_path.display(),
				new_path.display()
			);
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
			debug!("✓ Completed inode-based move for entry {}", existing.id);
			return Ok(true);
		} else {
			// Same path, same inode - this is a modification (macOS FSEvents reports as Create)
			// Update the existing entry instead of creating a duplicate
			debug!(
				"Entry already exists at path with same inode {}, updating instead of creating: {}",
				inode_val,
				new_path.display()
			);
			let dir_entry = build_dir_entry(new_path).await?;
			EntryProcessor::update_entry(ctx, existing.id, &dir_entry).await?;
			debug!("✓ Updated entry {} via inode match", existing.id);
			return Ok(true);
		}
	} else {
		debug!("✗ No existing entry found with inode {}", inode_val);
	}
	Ok(false)
}
