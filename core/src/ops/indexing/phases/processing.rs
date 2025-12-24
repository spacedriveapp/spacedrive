//! # Entry Processing and Change Detection
//!
//! `core::ops::indexing::phases::processing` converts discovered filesystem entries into
//! database records, applying change detection to identify new, modified, moved, and deleted
//! entries. Processes entries in depth-first order (parents before children) within database
//! transactions, preserving ephemeral UUIDs from prior browsing sessions and validating that
//! indexing paths stay within location boundaries to prevent cross-location data corruption.

use crate::{
	infra::{
		db::entities::{self, directory_paths, entry_closure},
		job::generic_progress::ToGenericProgress,
		job::prelude::{JobContext, JobError, Progress},
	},
	ops::indexing::{
		change_detection::{Change, ChangeDetector},
		database_storage::DatabaseStorage,
		state::{DirEntry, EntryKind, IndexError, IndexPhase, IndexerProgress, IndexerState},
		IndexMode,
	},
};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, TransactionTrait};
use std::{path::Path, sync::Arc};
use tracing::warn;
use uuid::Uuid;

/// Detects SQLite unique constraint violations from concurrent watcher and indexer writes.
///
/// When the file watcher creates an entry while the indexer is processing the same file,
/// both try to insert with the same (path, parent_id) combination. This is benign - the entry
/// exists, which is the desired outcome. We detect and skip these instead of failing the job.
fn is_unique_constraint_violation(error: &JobError) -> bool {
	let error_msg = error.to_string().to_lowercase();
	error_msg.contains("unique constraint")
		|| error_msg.contains("unique index")
		|| error_msg.contains("constraint failed")
}

/// Processes discovered entries into database records with change detection and UUID preservation.
///
/// Sorts all entries by depth (parents before children) to ensure hierarchy integrity, applies
/// change detection to identify new/modified/moved/deleted entries, processes changes within
/// batch transactions, preserves ephemeral UUIDs from browsing sessions, validates indexing
/// boundaries to prevent cross-location corruption, and emits sync/event batches for UI updates.
pub async fn run_processing_phase(
	location_id: Uuid,
	state: &mut IndexerState,
	ctx: &JobContext<'_>,
	mode: IndexMode,
	location_root_path: &Path,
	volume_backend: Option<&Arc<dyn crate::volume::VolumeBackend>>,
) -> Result<(), JobError> {
	let total_batches = state.entry_batches.len();
	ctx.log(format!(
		"Processing phase starting with {} batches",
		total_batches
	));

	// Populate ephemeral UUIDs so entries browsed before enabling indexing keep the same UUID,
	// preserving tags and notes attached during ephemeral mode. Without this, promoting a browsed
	// folder to a managed location would orphan all existing user metadata.
	let ephemeral_cache = ctx.library().core_context().ephemeral_cache();
	let preserved_count = state
		.populate_ephemeral_uuids(ephemeral_cache, location_root_path)
		.await;
	if preserved_count > 0 {
		ctx.log(format!(
			"Found {} ephemeral UUIDs to preserve from previous browsing",
			preserved_count
		));
	}

	if total_batches == 0 {
		ctx.log("No batches to process - transitioning to Aggregation phase");
		state.phase = crate::ops::indexing::state::Phase::Aggregation;
		return Ok(());
	}

	let location_record = entities::location::Entity::find()
		.filter(entities::location::Column::Uuid.eq(location_id))
		.one(ctx.library_db())
		.await
		.map_err(|e| JobError::execution(format!("Failed to find location: {}", e)))?
		.ok_or_else(|| JobError::execution("Location not found in database".to_string()))?;

	let device_id = location_record.device_id;
	let location_id_i32 = location_record.id;
	let location_entry_id = location_record
		.entry_id
		.ok_or_else(|| JobError::execution("Location entry_id not set (not yet synced)"))?;
	ctx.log(format!(
		"Found location record: device_id={}, location_id={}, entry_id={}",
		device_id, location_id_i32, location_entry_id
	));

	// SAFETY: Validate indexing path is within location boundaries to prevent catastrophic
	// cross-location deletion if watcher routing bugs send events for /home/user/photos to a
	// /home/user/documents location. Without this check, we'd delete all documents entries
	// not present in photos, wiping the database.
	let location_actual_path = crate::ops::indexing::path_resolver::PathResolver::get_full_path(
		ctx.library_db(),
		location_entry_id,
	)
	.await
	.map_err(|e| JobError::execution(format!("Failed to resolve location root path: {}", e)))?;

	let location_actual_str = location_actual_path.to_string_lossy();
	let is_cloud_path =
		location_actual_str.contains("://") && !location_actual_str.starts_with("local://");

	let is_within_boundaries = if is_cloud_path {
		let root_str = location_root_path.to_string_lossy();
		root_str.is_empty() || location_actual_str.starts_with(root_str.as_ref())
	} else {
		location_root_path.starts_with(&location_actual_path)
	};

	if !is_within_boundaries {
		return Err(JobError::execution(format!(
			"SAFETY VIOLATION: Indexing path '{}' is outside location boundaries '{}' (location_id={}). \
			This indicates a routing bug in the watcher. Aborting to prevent data loss.",
			location_root_path.display(),
			location_actual_path.display(),
			location_id
		)));
	}

	ctx.log(format!(
		"✓ Validated indexing path is within location boundaries: {}",
		location_actual_path.display()
	));

	// Seed entry ID cache with all ancestors between location root and indexing path.
	// Without this, re-indexing /home/user/docs/photos would fail to find /home/user/docs
	// in the cache and create a duplicate "docs" folder with wrong parent_id.
	let _ = state
		.seed_ancestor_cache(
			ctx.library_db(),
			&location_actual_path,
			location_entry_id,
			location_root_path,
		)
		.await;

	let mut change_detector = ChangeDetector::new();
	// Always load existing entries for change detection during reindexing.
	// This is required to detect moves, modifications, and deletions regardless
	// of IndexMode. The mode only affects what metadata gets extracted, not
	// whether we detect changes.
	ctx.log("Loading existing entries for change detection...");
	change_detector
		.load_existing_entries(ctx, location_id_i32, location_root_path)
		.await?;
	ctx.log(format!(
		"Loaded {} existing entries",
		change_detector.entry_count()
	));

	// Sort all discovered entries by depth (parents before children) to ensure parent entries
	// exist in the database before we try to create children with parent_id foreign keys.
	// Without this, creating /a/b/c.txt before /a would fail the parent_id constraint.
	ctx.log("Flattening and sorting all entries by depth...");
	let mut all_entries: Vec<DirEntry> = Vec::new();
	while let Some(batch) = state.entry_batches.pop() {
		all_entries.extend(batch);
	}

	all_entries.sort_by(|a, b| {
		let a_depth = a.path.components().count();
		let b_depth = b.path.components().count();

		match a_depth.cmp(&b_depth) {
			std::cmp::Ordering::Equal => {
				let a_priority = match a.kind {
					EntryKind::Directory => 0,
					EntryKind::Symlink => 1,
					EntryKind::File => 2,
				};
				let b_priority = match b.kind {
					EntryKind::Directory => 0,
					EntryKind::Symlink => 1,
					EntryKind::File => 2,
				};
				a_priority.cmp(&b_priority)
			}
			other => other,
		}
	});

	ctx.log(format!(
		"Sorted {} total entries by depth",
		all_entries.len()
	));

	let batch_size = 1000;
	let mut sorted_batches: Vec<Vec<DirEntry>> = Vec::new();
	let mut current_batch = Vec::with_capacity(batch_size);

	for entry in all_entries {
		current_batch.push(entry);
		if current_batch.len() >= batch_size {
			sorted_batches.push(std::mem::replace(
				&mut current_batch,
				Vec::with_capacity(batch_size),
			));
		}
	}
	if !current_batch.is_empty() {
		sorted_batches.push(current_batch);
	}

	state.entry_batches = sorted_batches;
	state.entry_batches.reverse();
	let total_batches = state.entry_batches.len();
	ctx.log(format!("Re-batched into {} sorted batches", total_batches));

	let mut total_processed = 0;
	let mut batch_number = 0;

	while let Some(batch) = state.entry_batches.pop() {
		ctx.check_interrupt().await?;

		batch_number += 1;
		let batch_size = batch.len();

		let indexer_progress = IndexerProgress {
			phase: IndexPhase::Processing {
				batch: batch_number,
				total_batches,
			},
			current_path: format!("Batch {}/{}", batch_number, total_batches),
			total_found: state.stats,
			processing_rate: state.calculate_rate(),
			estimated_remaining: state.estimate_remaining(),
			scope: None,
			persistence: None,
			is_ephemeral: false,
			action_context: None,
		};
		ctx.progress(Progress::generic(indexer_progress.to_generic_progress()));

		ctx.check_interrupt().await?;

		let txn = ctx.library_db().begin().await.map_err(|e| {
			JobError::execution(format!("Failed to begin processing transaction: {}", e))
		})?;

		let mut bulk_self_closures: Vec<entities::entry_closure::ActiveModel> = Vec::new();
		let mut bulk_dir_paths: Vec<entities::directory_paths::ActiveModel> = Vec::new();
		let mut created_entries: Vec<entities::entry::Model> = Vec::new();

		for entry in batch {
			if let Err(e) = ctx.check_interrupt().await {
				if let Err(rollback_err) = txn.rollback().await {
					warn!(
						"Failed to rollback transaction during interruption: {}",
						rollback_err
					);
				}
				return Err(e);
			}

			state.seen_paths.insert(entry.path.clone());

			// Cloud backends can't use std::fs::Metadata for change detection since files don't
			// exist locally. We treat cloud entries as always "new" for now. Future enhancement:
			// use backend-provided ETag or modified_at for cloud change detection.
			let change = if volume_backend.is_some() && !volume_backend.unwrap().is_local() {
				Some(Change::New(entry.path.clone()))
			} else {
				let metadata = match std::fs::symlink_metadata(&entry.path) {
					Ok(m) => m,
					Err(e) => {
						ctx.add_non_critical_error(format!(
							"Failed to get metadata for {}: {}",
							entry.path.display(),
							e
						));
						continue;
					}
				};
				change_detector.check_path(&entry.path, &metadata, entry.inode)
			};

			match change {
				Some(Change::New(_)) => {
					match DatabaseStorage::create_entry_in_conn(
						state,
						&entry,
						device_id,
						location_root_path,
						&txn,
						&mut bulk_self_closures,
						&mut bulk_dir_paths,
					)
					.await
					{
						Ok(entry_model) => {
							let entry_id = entry_model.id;
							ctx.log(format!(
								"Created entry {}: {}",
								entry_id,
								entry.path.display()
							));
							total_processed += 1;

							if mode >= IndexMode::Content && entry.kind == EntryKind::File {
								state.entries_for_content.push((entry_id, entry.path));
							}

							created_entries.push(entry_model);
						}
						Err(e) => {
							if is_unique_constraint_violation(&e) {
								ctx.log(format!(
									"Entry already exists (created by watcher): {}",
									entry.path.display()
								));
							} else {
								let error_msg = format!(
									"Failed to create entry for {}: {}",
									entry.path.display(),
									e
								);
								ctx.add_non_critical_error(error_msg);
								state.add_error(IndexError::CreateEntry {
									path: entry.path.to_string_lossy().to_string(),
									error: e.to_string(),
								});
							}
						}
					}
				}

				Some(Change::Modified { entry_id, .. }) => {
					match DatabaseStorage::update_entry_in_conn(entry_id, &entry, &txn).await {
						Ok(()) => {
							ctx.log(format!(
								"Updated entry {}: {}",
								entry_id,
								entry.path.display()
							));
							total_processed += 1;

							if mode >= IndexMode::Content && entry.kind == EntryKind::File {
								state.entries_for_content.push((entry_id, entry.path));
							}
						}
						Err(e) => {
							let error_msg = format!("Failed to update entry {}: {}", entry_id, e);
							ctx.add_non_critical_error(error_msg);
							state.add_error(IndexError::CreateEntry {
								path: entry.path.to_string_lossy().to_string(),
								error: e.to_string(),
							});
						}
					}
				}

				Some(Change::Moved {
					old_path,
					new_path,
					entry_id,
					..
				}) => {
					ctx.log(format!(
						"Detected move: {} -> {}",
						old_path.display(),
						new_path.display()
					));
					match DatabaseStorage::simple_move_entry_in_conn(
						state, entry_id, &old_path, &new_path, &txn,
					)
					.await
					{
						Ok(()) => {
							ctx.log(format!(
								"Moved entry {}: {} -> {}",
								entry_id,
								old_path.display(),
								new_path.display()
							));
							total_processed += 1;

							if mode >= IndexMode::Content && entry.kind == EntryKind::File {
								state.entries_for_content.push((entry_id, new_path));
							}
						}
						Err(e) => {
							let error_msg = format!("Failed to move entry {}: {}", entry_id, e);
							ctx.add_non_critical_error(error_msg);
							state.add_error(IndexError::CreateEntry {
								path: new_path.to_string_lossy().to_string(),
								error: e.to_string(),
							});
						}
					}
				}

				Some(Change::Deleted { .. }) => {
					// This shouldn't happen during processing of found entries
					// Deleted entries are handled after processing
				}

				None => {
					// No change - skip
					ctx.log(format!(" No change for: {}", entry.path.display()));
				}
			}
		}

		// Bulk insert self-closures if any
		if !bulk_self_closures.is_empty() {
			entities::entry_closure::Entity::insert_many(bulk_self_closures)
				.exec(&txn)
				.await
				.map_err(|e| {
					JobError::execution(format!("Failed to bulk insert self-closures: {}", e))
				})?;
		}

		// Bulk insert directory paths if any
		if !bulk_dir_paths.is_empty() {
			entities::directory_paths::Entity::insert_many(bulk_dir_paths)
				.exec(&txn)
				.await
				.map_err(|e| {
					JobError::execution(format!("Failed to bulk insert directory paths: {}", e))
				})?;
		}

		// Commit the batch creation transaction
		txn.commit().await.map_err(|e| {
			JobError::execution(format!("Failed to commit processing transaction: {}", e))
		})?;

		// All entries now have UUIDs assigned during creation
		// Sync directories and empty files immediately (sync-ready)
		// Regular files will be synced again after content identification
		if !created_entries.is_empty() {
			// Collect entry UUIDs for resource events
			let entry_uuids: Vec<Uuid> = created_entries.iter().filter_map(|e| e.uuid).collect();

			// Batch sync entries (only sync-ready ones will be included by query_for_sync filter)
			match ctx
				.library()
				.sync_models_batch(
					&created_entries,
					crate::infra::sync::ChangeType::Insert,
					ctx.library_db(),
				)
				.await
			{
				Ok(()) => {
					ctx.log(format!(
						"Batch synced {} entries (directories/empty files are sync-ready)",
						created_entries.len()
					));
				}
				Err(e) => {
					// Log but don't fail the job
					tracing::warn!(
						"Failed to batch sync {} entries: {}",
						created_entries.len(),
						e
					);
				}
			}

			// Emit ResourceChangedBatch events for UI
			if !entry_uuids.is_empty() {
				let library = ctx.library();
				let events = library.event_bus().clone();
				let db = Arc::new(ctx.library_db().clone());

				let resource_manager = crate::domain::ResourceManager::new(db, events);

				if let Err(e) = resource_manager
					.emit_resource_events("entry", entry_uuids)
					.await
				{
					tracing::warn!("Failed to emit resource events for created entries: {}", e);
				} else {
					ctx.log("Emitted resource events for created entries");
				}
			}
		}

		ctx.log(format!(
			"Processed batch {}/{}: {} entries",
			batch_number, total_batches, batch_size
		));

		// Note: State will be automatically saved during job serialization on shutdown
	}

	// Handle deleted entries
	if change_detector.entry_count() > 0 {
		ctx.log("Checking for deleted entries...");

		// Build seen_paths, ensuring the indexing root is included. The indexing path is
		// loaded as an existing entry but never "seen" during discovery (which only scans
		// children). Without this, an indexer job spawned for a subdirectory would detect
		// the subdirectory itself as "missing" and delete it along with all its children.
		let mut seen_paths: std::collections::HashSet<_> =
			state.seen_paths.iter().cloned().collect();
		seen_paths.insert(location_root_path.to_path_buf());

		let deleted = change_detector.find_deleted(&seen_paths);

		if !deleted.is_empty() {
			ctx.log(format!("Found {} deleted entries", deleted.len()));
			for change in deleted {
				if let Change::Deleted { path, entry_id } = change {
					// Skip deleting the location root entry - it shouldn't be deleted during indexing
					if entry_id == location_entry_id {
						ctx.log(format!(
							"Skipping deletion of location root entry (id: {})",
							entry_id
						));
						continue;
					}

					ctx.log(format!(
						"Deleting missing entry from database: {} (id: {})",
						path.display(),
						entry_id
					));

					// Check for interruption before deletion
					ctx.check_interrupt().await?;

					// Collect all entry IDs in the subtree
					let mut to_delete_ids: Vec<i32> = vec![entry_id];
					match entities::entry_closure::Entity::find()
						.filter(entities::entry_closure::Column::AncestorId.eq(entry_id))
						.all(ctx.library_db())
						.await
					{
						Ok(rows) => {
							to_delete_ids.extend(rows.into_iter().map(|r| r.descendant_id));
						}
						Err(e) => {
							let msg = format!(
								"Failed to query closure table for subtree of {}: {}",
								entry_id, e
							);
							ctx.add_non_critical_error(msg);
						}
					}

					to_delete_ids.sort_unstable();
					to_delete_ids.dedup();

					// Fetch all entry models that will be deleted
					let entries_to_delete = if !to_delete_ids.is_empty() {
						let mut all_entries = Vec::new();
						for chunk in to_delete_ids.chunks(900) {
							let batch = entities::entry::Entity::find()
								.filter(entities::entry::Column::Id.is_in(chunk.to_vec()))
								.all(ctx.library_db())
								.await
								.map_err(|e| {
									JobError::execution(format!("Failed to fetch entries: {}", e))
								})?;
							all_entries.extend(batch);
						}
						all_entries
					} else {
						Vec::new()
					};

					// Use sync_models_batch to handle sync tombstones AND event emission
					// Called before actual deletion so sync can create tombstones
					if !entries_to_delete.is_empty() {
						if let Err(e) = ctx
							.library()
							.sync_models_batch(
								&entries_to_delete,
								crate::infra::sync::ChangeType::Delete,
								ctx.library_db(),
							)
							.await
						{
							ctx.add_non_critical_error(format!("Failed to sync deletions: {}", e));
						}
					}

					// Step 4: Perform the actual database deletion
					let txn = ctx.library_db().begin().await.map_err(|e| {
						JobError::execution(format!("Failed to begin deletion transaction: {}", e))
					})?;

					if !to_delete_ids.is_empty() {
						use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
						let _ = entities::entry_closure::Entity::delete_many()
							.filter(
								entities::entry_closure::Column::DescendantId
									.is_in(to_delete_ids.clone()),
							)
							.exec(&txn)
							.await;
						let _ = entities::entry_closure::Entity::delete_many()
							.filter(
								entities::entry_closure::Column::AncestorId
									.is_in(to_delete_ids.clone()),
							)
							.exec(&txn)
							.await;
						let _ = entities::directory_paths::Entity::delete_many()
							.filter(
								entities::directory_paths::Column::EntryId
									.is_in(to_delete_ids.clone()),
							)
							.exec(&txn)
							.await;
						let _ = entities::entry::Entity::delete_many()
							.filter(entities::entry::Column::Id.is_in(to_delete_ids))
							.exec(&txn)
							.await;
					}

					txn.commit().await.map_err(|e| {
						JobError::execution(format!("Failed to commit deletion transaction: {}", e))
					})?;

					// Update in-memory caches
					state.entry_id_cache.remove(&path);
					ctx.log(format!("Deleted entry {} (and subtree if any)", entry_id));
				}
			}
		}
	}

	ctx.log(format!(
		"Processing phase complete: {} entries processed",
		total_processed
	));
	state.phase = crate::ops::indexing::state::Phase::Aggregation;
	Ok(())
}
