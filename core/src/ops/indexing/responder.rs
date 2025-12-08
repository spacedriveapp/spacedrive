//! Change Detection Responder (function-style)
//!
//! Translates raw filesystem events into database-backed operations using the
//! indexing module. The watcher emits path-only events; this module resolves
//! real entry IDs and performs identity-preserving updates.

use crate::context::CoreContext;
use crate::domain::ResourceManager;
use crate::infra::db::entities;
use crate::infra::event::FsRawEventKind;
use crate::ops::indexing::entry::EntryProcessor;
use crate::ops::indexing::path_resolver::PathResolver;
use crate::ops::indexing::processor::{
	self, ContentHashProcessor, LocationProcessorConfig, ProcessorEntry, ProcessorResult,
};
use crate::ops::indexing::rules::{build_default_ruler, RuleToggles, RulerDecision};
use crate::ops::indexing::state::{DirEntry, IndexerState};
use crate::ops::indexing::{ctx::ResponderCtx, IndexingCtx};
use crate::ops::media::{
	ocr::OcrProcessor, proxy::ProxyProcessor, speech::SpeechToTextProcessor,
	thumbnail::ThumbnailProcessor, thumbstrip::ThumbstripProcessor,
};
use anyhow::Result;
use sea_orm::{ColumnTrait, DbErr, EntityTrait, QueryFilter, QuerySelect, TransactionTrait};
use std::path::Path;
use std::sync::Arc;
use tracing::{debug, warn};
use uuid::Uuid;

/// Check if a path exists, distinguishing between "doesn't exist" and "can't access"
///
/// This is critical for preventing false deletions when volumes go offline.
/// Returns Ok(true) if path exists, Ok(false) if confirmed absent, Err if inaccessible.
async fn path_exists_safe(
	path: &Path,
	backend: Option<&Arc<dyn crate::volume::VolumeBackend>>,
) -> Result<bool> {
	use crate::volume::error::VolumeError;

	if let Some(backend) = backend {
		// Use volume backend (works for both local and cloud)
		match backend.exists(path).await {
			Ok(exists) => Ok(exists),
			Err(VolumeError::NotMounted(_)) => {
				// Volume is not mounted - don't treat as deletion
				warn!(
					"Volume not mounted when checking path existence: {}",
					path.display()
				);
				Err(anyhow::anyhow!(
					"Volume not mounted, cannot verify path existence"
				))
			}
			Err(VolumeError::Io(ref e)) if e.kind() == std::io::ErrorKind::NotFound => {
				// Path doesn't exist - this is OK, return false
				Ok(false)
			}
			Err(VolumeError::Io(io_err)) => {
				// Other IO errors (permissions, volume offline, etc.) - don't treat as deletion
				warn!(
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
				// Other volume errors (timeout, permission denied, etc.)
				warn!(
					"Volume error when checking path existence for {}: {}",
					path.display(),
					e
				);
				Err(e.into())
			}
		}
	} else {
		// Fallback to local filesystem
		match tokio::fs::try_exists(path).await {
			Ok(exists) => Ok(exists),
			Err(e) => {
				// IO error - can't determine existence (volume may be offline)
				warn!(
					"Cannot verify path existence for {} (volume may be offline): {}",
					path.display(),
					e
				);
				Err(anyhow::anyhow!("Cannot access path: {}", e))
			}
		}
	}
}

/// Translates a single filesystem event into database mutations: create, modify, rename, or remove.
///
/// Queries the database to resolve paths to entry IDs, then delegates to specialized handlers.
/// For creates/modifies, runs the processor pipeline (content hash, thumbnails, etc.) inline.
pub async fn apply(
	context: &Arc<CoreContext>,
	library_id: Uuid,
	location_id: Uuid,
	kind: FsRawEventKind,
	rule_toggles: RuleToggles,
	location_root: &Path,
	volume_backend: Option<&Arc<dyn crate::volume::VolumeBackend>>,
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
				volume_backend,
			)
			.await?
		}
		FsRawEventKind::Modify { path } => {
			handle_modify(
				&ctx,
				context,
				library_id,
				location_id,
				&path,
				rule_toggles,
				location_root,
				volume_backend,
			)
			.await?
		}
		FsRawEventKind::Remove { path } => handle_remove(&ctx, context, location_id, &path).await?,
		FsRawEventKind::Rename { from, to } => {
			handle_rename(
				&ctx,
				context,
				location_id,
				&from,
				&to,
				rule_toggles,
				location_root,
				volume_backend,
			)
			.await?
		}
	}
	Ok(())
}

/// Processes multiple filesystem events as a batch, deduplicating and ordering for correctness.
///
/// Groups events by type, deduplicates (macOS sends duplicate creates), then processes in order:
/// removes first, then renames, creates, modifies. This prevents conflicts like creating a file
/// that should have been deleted.
pub async fn apply_batch(
	context: &Arc<CoreContext>,
	library_id: Uuid,
	location_id: Uuid,
	events: Vec<FsRawEventKind>,
	rule_toggles: RuleToggles,
	location_root: &Path,
	volume_backend: Option<&Arc<dyn crate::volume::VolumeBackend>>,
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

	// macOS FSEvents sends duplicate creates when files are written incrementally.
	creates.sort();
	creates.dedup();
	modifies.sort();
	modifies.dedup();
	removes.sort();
	removes.dedup();

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
		if let Err(e) = handle_rename(
			&ctx,
			context,
			location_id,
			&from,
			&to,
			rule_toggles,
			location_root,
			volume_backend,
		)
		.await
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
		debug!(
			"[BATCH #{}] Processing create {}/{}: {}",
			call_id,
			idx + 1,
			creates.len(),
			path.display()
		);
		if let Err(e) = handle_create(
			&ctx,
			context,
			library_id,
			location_id,
			&path,
			rule_toggles,
			location_root,
			volume_backend,
		)
		.await
		{
			tracing::error!("Failed to handle create for {}: {}", path.display(), e);
		}
		debug!(
			"[BATCH #{}] Completed create {}/{}",
			call_id,
			idx + 1,
			creates.len()
		);
	}

	// Process modifies
	for path in modifies {
		if let Err(e) = handle_modify(
			&ctx,
			context,
			library_id,
			location_id,
			&path,
			rule_toggles,
			location_root,
			volume_backend,
		)
		.await
		{
			tracing::error!("Failed to handle modify for {}: {}", path.display(), e);
		}
	}

	Ok(())
}

/// Fetches the location's root entry_id to scope path lookups within the correct location tree.
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

/// Evaluates indexing rules to determine if a path should be skipped (hidden files, system dirs, etc.).
async fn should_filter_path(
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
				crate::ops::indexing::state::EntryKind::Directory
			} else if fs_meta.is_symlink() {
				crate::ops::indexing::state::EntryKind::Symlink
			} else {
				crate::ops::indexing::state::EntryKind::File
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
		is_dir: metadata.kind == crate::ops::indexing::state::EntryKind::Directory,
	};

	match ruler.evaluate_path(path, &simple_meta).await {
		Ok(RulerDecision::Reject) => {
			debug!("Filtered path by indexing rules: {}", path.display());
			Ok(true)
		}
		Ok(RulerDecision::Accept) => Ok(false),
		Err(e) => {
			tracing::warn!("Error evaluating rules for {}: {}", path.display(), e);
			Ok(false)
		}
	}
}

/// Creates a new entry for the path, runs processors, and spawns recursive indexing for directories.
///
/// Checks for duplicate creates (race conditions), inode-based moves, and filters based on rules.
/// For directories, dispatches an IndexerJob to index contents. For files, runs the processor
/// pipeline inline (content hash, thumbnails, etc.).
async fn handle_create(
	ctx: &impl IndexingCtx,
	context: &Arc<CoreContext>,
	library_id: Uuid,
	location_id: Uuid,
	path: &Path,
	rule_toggles: RuleToggles,
	location_root: &Path,
	backend: Option<&Arc<dyn crate::volume::VolumeBackend>>,
) -> Result<()> {
	debug!("Create: {}", path.display());

	match path_exists_safe(path, backend).await {
		Ok(true) => {
			// Path exists and is accessible, proceed
		}
		Ok(false) => {
			debug!("Path no longer exists, skipping create: {}", path.display());
			return Ok(());
		}
		Err(e) => {
			warn!(
				"Skipping create event for inaccessible path {}: {}",
				path.display(),
				e
			);
			return Ok(());
		}
	}

	// Check if path should be filtered
	if should_filter_path(path, rule_toggles, location_root, backend).await? {
		debug!("✗ Skipping filtered path: {}", path.display());
		return Ok(());
	}

	debug!("→ Processing create for: {}", path.display());
	let dir_entry = build_dir_entry(path, backend).await?;

	// Check if entry already exists at this exact path (race condition from duplicate watcher events)
	let location_root_entry_id = get_location_root_entry_id(ctx, location_id).await?;
	if let Some(existing_id) =
		resolve_entry_id_by_path_scoped(ctx, path, location_root_entry_id).await?
	{
		debug!(
			"Entry already exists at path {} (entry_id={}), treating as modify instead of create",
			path.display(),
			existing_id
		);
		// Treat as a modify instead
		return handle_modify(
			ctx,
			context,
			library_id,
			location_id,
			path,
			rule_toggles,
			location_root,
			backend,
		)
		.await;
	}

	// If inode matches an existing entry at another path, treat this as a move
	if handle_move_by_inode(ctx, path, dir_entry.inode, backend).await? {
		return Ok(());
	}

	// Minimal state provides parent cache used by EntryProcessor
	let mut state = IndexerState::new(&crate::domain::addressing::SdPath::local(path));

	// Seed ancestor directories into cache to prevent ghost folder bug
	if let Ok(Some(location_record)) = entities::location::Entity::find()
		.filter(entities::location::Column::Uuid.eq(location_id))
		.one(ctx.library_db())
		.await
	{
		if let Some(location_entry_id) = location_record.entry_id {
			let _ = state
				.seed_ancestor_cache(ctx.library_db(), location_root, location_entry_id, path)
				.await;
		}
	}

	// Try to create the entry, handling unique constraint violations with upsert
	let entry_id = match EntryProcessor::create_entry(
		&mut state,
		ctx,
		&dir_entry,
		0, // device_id not needed here
		path.parent().unwrap_or_else(|| Path::new("/")),
	)
	.await
	{
		Ok(id) => {
			debug!("✓ Created entry {} for path: {}", id, path.display());
			id
		}
		Err(e) if is_unique_constraint_violation(&e) => {
			// Entry was created concurrently by another event, update it instead
			debug!(
				"Unique constraint violation for {}, updating existing entry (race condition)",
				path.display()
			);

			// Find the existing entry that caused the constraint violation
			if let Some(existing_id) =
				resolve_entry_id_by_path_scoped(ctx, path, location_root_entry_id).await?
			{
				// Update the existing entry with new metadata (including potentially new inode)
				EntryProcessor::update_entry(ctx, existing_id, &dir_entry).await?;
				debug!(
					"✓ Updated existing entry {} with new metadata (inode: {:?})",
					existing_id, dir_entry.inode
				);

				// Treat as modify for processor pipeline
				return handle_modify(
					ctx,
					context,
					library_id,
					location_id,
					path,
					rule_toggles,
					location_root,
					backend,
				)
				.await;
			} else {
				// Shouldn't happen - we got unique constraint but can't find the entry
				warn!(
					"Unique constraint violation but entry not found for path: {}",
					path.display()
				);
				return Err(e.into());
			}
		}
		Err(e) => {
			return Err(e.into());
		}
	};

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
			// Query the location to get its index_mode policy
			let location_record = entities::location::Entity::find()
				.filter(entities::location::Column::Uuid.eq(location_id))
				.one(ctx.library_db())
				.await
				.ok()
				.flatten();

			// Determine index mode from location policy (default to Content if not found)
			let index_mode = if let Some(loc) = location_record {
				match loc.index_mode.as_str() {
					"shallow" => super::job::IndexMode::Shallow,
					"content" => super::job::IndexMode::Content,
					"deep" => super::job::IndexMode::Deep,
					_ => super::job::IndexMode::Content,
				}
			} else {
				super::job::IndexMode::Content
			};

			// Create a recursive indexer job for this directory subtree
			// Use the location's index_mode to respect thumbnail/thumbstrip policies
			let indexer_job = super::job::IndexerJob::from_location(
				location_id,
				crate::domain::addressing::SdPath::local(path),
				index_mode,
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
					"✓ Spawned recursive indexer job (mode: {:?}) for directory: {}",
					index_mode,
					path.display()
				);
			}
		}
	} else {
		// For files, run processors inline (single file processing)
		if let Some(library) = context.get_library(library_id).await {
			// Load processor configuration for this location
			let proc_config =
				processor::load_location_processor_config(location_id, ctx.library_db())
					.await
					.unwrap_or_default();

			// Build processor entry (with MIME type after content linking)
			let proc_entry = build_processor_entry(ctx, entry_id, path).await?;

			// Run content hash processor first
			if proc_config
				.watcher_processors
				.iter()
				.any(|c| c.processor_type == "content_hash" && c.enabled)
			{
				let content_proc = ContentHashProcessor::new(library_id);
				if let Err(e) = content_proc.process(ctx, &proc_entry).await {
					warn!("Content hash processing failed: {}", e);
				}
			}

			// Reload processor entry to get updated content_id and MIME type
			let proc_entry = build_processor_entry(ctx, entry_id, path).await?;

			// Run thumbnail processor
			if proc_config
				.watcher_processors
				.iter()
				.any(|c| c.processor_type == "thumbnail" && c.enabled)
			{
				let thumb_proc = ThumbnailProcessor::new(library.clone());
				if thumb_proc.should_process(&proc_entry) {
					if let Err(e) = thumb_proc.process(ctx.library_db(), &proc_entry).await {
						warn!("Thumbnail processing failed: {}", e);
					}
				}
			}

			// Run thumbstrip processor
			if proc_config
				.watcher_processors
				.iter()
				.any(|c| c.processor_type == "thumbstrip" && c.enabled)
			{
				let settings = proc_config
					.watcher_processors
					.iter()
					.find(|c| c.processor_type == "thumbstrip")
					.map(|c| &c.settings);

				let thumbstrip_proc = if let Some(settings) = settings {
					ThumbstripProcessor::new(library.clone())
						.with_settings(settings)
						.unwrap_or_else(|e| {
							warn!("Failed to parse thumbstrip settings: {}", e);
							ThumbstripProcessor::new(library.clone())
						})
				} else {
					ThumbstripProcessor::new(library.clone())
				};

				if thumbstrip_proc.should_process(&proc_entry) {
					if let Err(e) = thumbstrip_proc.process(ctx.library_db(), &proc_entry).await {
						warn!("Thumbstrip processing failed: {}", e);
					}
				}
			}

			// Run proxy processor
			if proc_config
				.watcher_processors
				.iter()
				.any(|c| c.processor_type == "proxy" && c.enabled)
			{
				let settings = proc_config
					.watcher_processors
					.iter()
					.find(|c| c.processor_type == "proxy")
					.map(|c| &c.settings);

				let proxy_proc = if let Some(settings) = settings {
					ProxyProcessor::new(library.clone())
						.with_settings(settings)
						.unwrap_or_else(|e| {
							warn!("Failed to parse proxy settings: {}", e);
							ProxyProcessor::new(library.clone())
						})
				} else {
					ProxyProcessor::new(library.clone())
				};

				if proxy_proc.should_process(&proc_entry) {
					if let Err(e) = proxy_proc.process(ctx.library_db(), &proc_entry).await {
						warn!("Proxy processing failed: {}", e);
					}
				}
			}

			// Run OCR processor
			if proc_config
				.watcher_processors
				.iter()
				.any(|c| c.processor_type == "ocr" && c.enabled)
			{
				let ocr_proc = OcrProcessor::new(library.clone());
				if ocr_proc.should_process(&proc_entry) {
					if let Err(e) = ocr_proc.process(ctx.library_db(), &proc_entry).await {
						warn!("OCR processing failed: {}", e);
					}
				}
			}

			// Run speech-to-text processor
			if proc_config
				.watcher_processors
				.iter()
				.any(|c| c.processor_type == "speech_to_text" && c.enabled)
			{
				let speech_proc = SpeechToTextProcessor::new(library.clone());
				if speech_proc.should_process(&proc_entry) {
					if let Err(e) = speech_proc.process(ctx.library_db(), &proc_entry).await {
						warn!("Speech-to-text processing failed: {}", e);
					}
				}
			}
		}
	}

	// Emit resource event for the created entry
	if let Some(uuid) = entry_uuid {
		debug!("→ Emitting resource event for entry {}", uuid);
		let resource_manager =
			ResourceManager::new(Arc::new(ctx.library_db().clone()), context.events.clone());

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
	backend: Option<&Arc<dyn crate::volume::VolumeBackend>>,
) -> Result<()> {
	debug!("Modify: {}", path.display());

	// Verify path is accessible before processing
	match path_exists_safe(path, backend).await {
		Ok(true) => {
			// Path exists and is accessible, proceed
		}
		Ok(false) => {
			debug!("Path no longer exists, skipping modify: {}", path.display());
			return Ok(());
		}
		Err(e) => {
			warn!(
				"Skipping modify event for inaccessible path {}: {}",
				path.display(),
				e
			);
			return Ok(());
		}
	}

	// Check if path should be filtered
	if should_filter_path(path, rule_toggles, location_root, backend).await? {
		debug!("✗ Skipping filtered path: {}", path.display());
		return Ok(());
	}

	debug!("→ Processing modify for: {}", path.display());

	// Get location root entry ID for scoped queries
	let location_root_entry_id = get_location_root_entry_id(ctx, location_id).await?;

	// If inode indicates a move, handle as a move and skip update
	let meta = EntryProcessor::extract_metadata(path, backend).await?;
	if handle_move_by_inode(ctx, path, meta.inode, backend).await? {
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

		// For files, run processors on the modified file
		if dir_entry.kind == super::state::EntryKind::File {
			if let Some(library) = context.get_library(library_id).await {
				// Load processor configuration
				let proc_config =
					processor::load_location_processor_config(location_id, ctx.library_db())
						.await
						.unwrap_or_default();

				// Build processor entry
				let proc_entry = build_processor_entry(ctx, entry_id, path).await?;

				// Run content hash processor first
				if proc_config
					.watcher_processors
					.iter()
					.any(|c| c.processor_type == "content_hash" && c.enabled)
				{
					let content_proc = ContentHashProcessor::new(library_id);
					if let Err(e) = content_proc.process(ctx, &proc_entry).await {
						warn!("Content hash processing failed: {}", e);
					}
				}

				// Reload processor entry to get updated content_id and MIME type
				let proc_entry = build_processor_entry(ctx, entry_id, path).await?;

				// Run thumbnail processor
				if proc_config
					.watcher_processors
					.iter()
					.any(|c| c.processor_type == "thumbnail" && c.enabled)
				{
					let thumb_proc = ThumbnailProcessor::new(library.clone());
					if thumb_proc.should_process(&proc_entry) {
						if let Err(e) = thumb_proc.process(ctx.library_db(), &proc_entry).await {
							warn!("Thumbnail processing failed: {}", e);
						}
					}
				}

				// Run OCR processor
				if proc_config
					.watcher_processors
					.iter()
					.any(|c| c.processor_type == "ocr" && c.enabled)
				{
					let ocr_proc = OcrProcessor::new(library.clone());
					if ocr_proc.should_process(&proc_entry) {
						if let Err(e) = ocr_proc.process(ctx.library_db(), &proc_entry).await {
							warn!("OCR processing failed: {}", e);
						}
					}
				}

				// Run speech-to-text processor
				if proc_config
					.watcher_processors
					.iter()
					.any(|c| c.processor_type == "speech_to_text" && c.enabled)
				{
					let speech_proc = SpeechToTextProcessor::new(library.clone());
					if speech_proc.should_process(&proc_entry) {
						if let Err(e) = speech_proc.process(ctx.library_db(), &proc_entry).await {
							warn!("Speech-to-text processing failed: {}", e);
						}
					}
				}
			}
		}

		// Emit resource event for the updated entry
		if let Some(uuid) = entry_uuid {
			debug!("→ Emitting resource event for modified entry {}", uuid);
			let resource_manager =
				ResourceManager::new(Arc::new(ctx.library_db().clone()), context.events.clone());

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
		debug!(
			"✗ Entry not found for path, skipping modify: {}",
			path.display()
		);
	}
	Ok(())
}

/// Handle remove: resolve entry ID and delete subtree (closure table + cache)
async fn handle_remove(
	ctx: &impl IndexingCtx,
	context: &Arc<CoreContext>,
	location_id: Uuid,
	path: &Path,
) -> Result<()> {
	debug!("Remove: {}", path.display());

	// Get location root entry ID for scoped queries
	let location_root_entry_id = get_location_root_entry_id(ctx, location_id).await?;

	if let Some(entry_id) =
		resolve_entry_id_by_path_scoped(ctx, path, location_root_entry_id).await?
	{
		debug!("→ Deleting entry {} for path: {}", entry_id, path.display());
		delete_subtree(ctx, context, location_id, entry_id).await?;
		debug!("✓ Deleted entry {} for path: {}", entry_id, path.display());
		// Note: ResourceDeleted events are emitted by sync_models_batch in delete_subtree
	} else {
		debug!(
			"✗ Entry not found for path, skipping remove: {}",
			path.display()
		);
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
	backend: Option<&Arc<dyn crate::volume::VolumeBackend>>,
) -> Result<()> {
	debug!("Rename: {} -> {}", from.display(), to.display());

	// Verify destination path is accessible before processing
	match path_exists_safe(to, backend).await {
		Ok(true) => {
			// Destination exists and is accessible, proceed
		}
		Ok(false) => {
			debug!(
				"Destination path doesn't exist, skipping rename: {}",
				to.display()
			);
			return Ok(());
		}
		Err(e) => {
			warn!(
				"Skipping rename event for inaccessible destination {}: {}",
				to.display(),
				e
			);
			return Ok(());
		}
	}

	// Get location root entry ID for scoped queries
	let location_root_entry_id = get_location_root_entry_id(ctx, location_id).await?;

	// Check if the destination path should be filtered
	// If the file is being moved to a filtered location, we should remove it from the database
	if should_filter_path(to, rule_toggles, location_root, backend).await? {
		debug!(
			"✗ Destination path is filtered, removing entry: {}",
			to.display()
		);
		// Treat this as a removal of the source file
		return handle_remove(ctx, context, location_id, from).await;
	}

	debug!(
		"→ Processing rename for: {} -> {}",
		from.display(),
		to.display()
	);

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
async fn build_dir_entry(
	path: &Path,
	backend: Option<&Arc<dyn crate::volume::VolumeBackend>>,
) -> Result<DirEntry> {
	let meta = EntryProcessor::extract_metadata(path, backend).await?;
	Ok(DirEntry {
		path: meta.path,
		kind: meta.kind,
		size: meta.size,
		modified: meta.modified,
		inode: meta.inode,
	})
}

/// Build a ProcessorEntry from database entry
async fn build_processor_entry(
	ctx: &impl IndexingCtx,
	entry_id: i32,
	path: &Path,
) -> Result<ProcessorEntry> {
	use sea_orm::EntityTrait;

	let entry = entities::entry::Entity::find_by_id(entry_id)
		.one(ctx.library_db())
		.await?
		.ok_or_else(|| anyhow::anyhow!("Entry not found"))?;

	// Get MIME type if content exists
	let mime_type = if let Some(content_id) = entry.content_id {
		if let Ok(Some(ci)) = entities::content_identity::Entity::find_by_id(content_id)
			.one(ctx.library_db())
			.await
		{
			if let Some(mime_id) = ci.mime_type_id {
				if let Ok(Some(mime)) = entities::mime_type::Entity::find_by_id(mime_id)
					.one(ctx.library_db())
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

	// Convert DB entry kind to domain EntryKind
	let kind = match entry.kind {
		0 => super::state::EntryKind::File,
		1 => super::state::EntryKind::Directory,
		2 => super::state::EntryKind::Symlink,
		_ => super::state::EntryKind::File,
	};

	Ok(ProcessorEntry {
		id: entry.id,
		uuid: entry.uuid,
		path: path.to_path_buf(),
		kind,
		size: entry.size as u64,
		content_id: entry.content_id,
		mime_type,
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

/// Check if an error is a unique constraint violation
fn is_unique_constraint_violation(error: &crate::infra::job::error::JobError) -> bool {
	// Check if the error contains SQLite unique constraint violation messages
	let error_msg = error.to_string().to_lowercase();
	error_msg.contains("unique constraint")
		|| error_msg.contains("unique index")
		|| error_msg.contains("constraint failed")
}

/// Best-effort deletion of an entry and its subtree (with tombstone creation)
///
/// This variant is used for local deletions (watcher, indexer) and creates
/// a tombstone for the root entry UUID to sync the deletion to other devices.
async fn delete_subtree(
	ctx: &impl IndexingCtx,
	context: &Arc<CoreContext>,
	location_id: Uuid,
	entry_id: i32,
) -> Result<()> {
	use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};

	// Step 1: Collect all entry IDs in the subtree
	let mut to_delete_ids: Vec<i32> = vec![entry_id];
	if let Ok(rows) = entities::entry_closure::Entity::find()
		.filter(entities::entry_closure::Column::AncestorId.eq(entry_id))
		.all(ctx.library_db())
		.await
	{
		to_delete_ids.extend(rows.into_iter().map(|r| r.descendant_id));
	}

	// IMPORTANT: Also find descendants by parent_id recursively as a fallback
	let mut queue = vec![entry_id];
	let mut visited = std::collections::HashSet::from([entry_id]);

	while let Some(parent) = queue.pop() {
		if let Ok(children) = entities::entry::Entity::find()
			.filter(entities::entry::Column::ParentId.eq(parent))
			.all(ctx.library_db())
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

	// Step 2: Fetch all entry models that will be deleted
	let entries_to_delete = if !to_delete_ids.is_empty() {
		let mut all_entries = Vec::new();
		// Chunk to avoid SQLite variable limit
		for chunk in to_delete_ids.chunks(900) {
			let batch = entities::entry::Entity::find()
				.filter(entities::entry::Column::Id.is_in(chunk.to_vec()))
				.all(ctx.library_db())
				.await?;
			all_entries.extend(batch);
		}
		all_entries
	} else {
		Vec::new()
	};

	if !entries_to_delete.is_empty() {
		if let Some(library) = context.get_library(location_id).await {
			// Use sync_models_batch for proper sync and event handling
			// This will create tombstones for all entries and emit ResourceDeleted events
			let _ = library
				.sync_models_batch(
					&entries_to_delete,
					crate::infra::sync::ChangeType::Delete,
					ctx.library_db(),
				)
				.await;
		}
	}

	// Step 4: Now perform the actual database deletion
	let txn = ctx.library_db().begin().await?;

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
	backend: Option<&Arc<dyn crate::volume::VolumeBackend>>,
) -> Result<bool> {
	let inode_val = match inode {
		Some(i) if i != 0 => i as i64,
		_ => return Ok(false),
	};

	debug!(
		"→ Checking inode {} for potential move detection",
		inode_val
	);

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
			let dir_entry = build_dir_entry(new_path, backend).await?;
			EntryProcessor::update_entry(ctx, existing.id, &dir_entry).await?;
			debug!("✓ Updated entry {} via inode match", existing.id);
			return Ok(true);
		}
	} else {
		debug!("✗ No existing entry found with inode {}", inode_val);
	}
	Ok(false)
}
