//! Processing phase - creates/updates database entries

use crate::{
	infra::{
		db::entities::{self, directory_paths, entry_closure},
		job::generic_progress::ToGenericProgress,
		job::prelude::{JobContext, JobError, Progress},
	},
	ops::indexing::{
		change_detection::{Change, ChangeDetector},
		entry::EntryProcessor,
		state::{DirEntry, EntryKind, IndexError, IndexPhase, IndexerProgress, IndexerState},
		IndexMode,
	},
};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, TransactionTrait};
use std::{path::Path, sync::Arc};
use tracing::warn;
use uuid::Uuid;

/// Run the processing phase of indexing
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

	if total_batches == 0 {
		ctx.log("No batches to process - transitioning to Aggregation phase");
		state.phase = crate::ops::indexing::state::Phase::Aggregation;
		return Ok(());
	}

	// Get the actual location record from database
	let location_record = entities::location::Entity::find()
		.filter(entities::location::Column::Uuid.eq(location_id))
		.one(ctx.library_db())
		.await
		.map_err(|e| JobError::execution(format!("Failed to find location: {}", e)))?
		.ok_or_else(|| JobError::execution("Location not found in database".to_string()))?;

	let device_id = location_record.device_id;
	let location_id_i32 = location_record.id;
	let location_entry_id = location_record.entry_id;
	ctx.log(format!(
		"Found location record: device_id={}, location_id={}, entry_id={}",
		device_id, location_id_i32, location_entry_id
	));

	// Add the location root entry to the cache so children can find their parent
	state
		.entry_id_cache
		.insert(location_root_path.to_path_buf(), location_entry_id);

	// Load existing entries for change detection scoped to the indexing path
	// Note: location_root_path is the actual path being indexed (could be a subpath of the location)
	let mut change_detector = ChangeDetector::new();
	if !state.existing_entries.is_empty() || mode != IndexMode::Shallow {
		ctx.log("Loading existing entries for change detection...");
		change_detector
			.load_existing_entries(ctx, location_id_i32, location_root_path)
			.await?;
		ctx.log(format!(
			"Loaded {} existing entries",
			change_detector.entry_count()
		));
	}

	// Flatten all batches and sort globally by depth to ensure parents are always processed before children
	ctx.log("Flattening and sorting all entries by depth...");
	let mut all_entries: Vec<DirEntry> = Vec::new();
	while let Some(batch) = state.entry_batches.pop() {
		all_entries.extend(batch);
	}

	// Sort all entries by depth first, then by type
	all_entries.sort_by(|a, b| {
		let a_depth = a.path.components().count();
		let b_depth = b.path.components().count();

		// First sort by depth (parents before children)
		match a_depth.cmp(&b_depth) {
			std::cmp::Ordering::Equal => {
				// Then sort by type (directories before files at same depth)
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

	// Re-batch the sorted entries for processing
	let batch_size = 1000; // Use a reasonable batch size
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

	// Use pop() below to consume batches. Reverse so that the first (shallowest) batch is processed first.
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
			action_context: None, // TODO: Pass action context from job state
		};
		ctx.progress(Progress::generic(indexer_progress.to_generic_progress()));

		// Check for interruption before starting transaction
		ctx.check_interrupt().await?;

		// Begin a single transaction for all new entry creations in this batch
		let txn = ctx.library_db().begin().await.map_err(|e| {
			JobError::execution(format!("Failed to begin processing transaction: {}", e))
		})?;

		// Accumulate related rows for bulk insert
		let mut bulk_self_closures: Vec<entities::entry_closure::ActiveModel> = Vec::new();
		let mut bulk_dir_paths: Vec<entities::directory_paths::ActiveModel> = Vec::new();

		// Process batch - check for changes and create/update entries
		// (Already sorted globally by depth)
		for entry in batch {
			// Check for interruption during batch processing
			if let Err(e) = ctx.check_interrupt().await {
				// Rollback transaction before propagating interruption
				if let Err(rollback_err) = txn.rollback().await {
					warn!(
						"Failed to rollback transaction during interruption: {}",
						rollback_err
					);
				}
				return Err(e);
			}

			// Add to seen_paths for delete detection (important for resumed jobs)
			state.seen_paths.insert(entry.path.clone());

			// Check for changes
			// Note: For cloud backends, we skip change detection for now since we can't
			// access std::fs::Metadata directly. Cloud entries are always treated as "new"
			// on first index. Future: implement cloud-specific change detection using
			// backend metadata.
			let change = if volume_backend.is_some() && !volume_backend.unwrap().is_local() {
				// Cloud backend - treat as new for now
				Some(Change::New(entry.path.clone()))
			} else {
				// Local backend - use standard change detection
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
					// Create new entry within batch transaction
					match EntryProcessor::create_entry_in_conn(
						state,
						ctx,
						&entry,
						device_id,
						location_root_path,
						&txn,
						&mut bulk_self_closures,
						&mut bulk_dir_paths,
					)
					.await
					{
						Ok(entry_id) => {
							ctx.log(format!(
								"Created entry {}: {}",
								entry_id,
								entry.path.display()
							));
							total_processed += 1;

							// Track for content identification if needed
							if mode >= IndexMode::Content && entry.kind == EntryKind::File {
								state.entries_for_content.push((entry_id, entry.path));
							}
							// end Some(Change::New)
						}
						Err(e) => {
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

				Some(Change::Modified { entry_id, .. }) => {
					// Update existing entry within batch transaction
					match EntryProcessor::update_entry_in_conn(ctx, entry_id, &entry, &txn).await {
						Ok(()) => {
							ctx.log(format!(
								"Updated entry {}: {}",
								entry_id,
								entry.path.display()
							));
							total_processed += 1;

							// Re-process content if needed
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
					// Handle move - update path in database
					ctx.log(format!(
						"Detected move: {} -> {}",
						old_path.display(),
						new_path.display()
					));
					match EntryProcessor::simple_move_entry_in_conn(
						state, ctx, entry_id, &old_path, &new_path, &txn,
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

							// Re-process content if needed for moved files
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

		ctx.log(format!(
			"Processed batch {}/{}: {} entries",
			batch_number, total_batches, batch_size
		));

		// Note: State will be automatically saved during job serialization on shutdown
	}

	// Handle deleted entries
	if change_detector.entry_count() > 0 {
		ctx.log("Checking for deleted entries...");
		let seen_paths: std::collections::HashSet<_> = state.seen_paths.iter().cloned().collect();
		let deleted = change_detector.find_deleted(&seen_paths);

		if !deleted.is_empty() {
			ctx.log(format!("Found {} deleted entries", deleted.len()));
			for change in deleted {
				if let Change::Deleted { path, entry_id } = change {
					ctx.log(format!(
						"Deleting missing entry from database: {} (id: {})",
						path.display(),
						entry_id
					));

					// Check for interruption before deletion transaction
					ctx.check_interrupt().await?;

					// Best-effort subtree deletion: remove closure links, directory path cache, and entries
					let txn = ctx.library_db().begin().await.map_err(|e| {
						JobError::execution(format!("Failed to begin deletion transaction: {}", e))
					})?;

					// Collect subtree descendant IDs (including the entry itself)
					let mut to_delete_ids: Vec<i32> = vec![entry_id];
					match entities::entry_closure::Entity::find()
						.filter(entities::entry_closure::Column::AncestorId.eq(entry_id))
						.all(&txn)
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
							// Attempt to delete just the single entry below
						}
					}

					// De-duplicate IDs
					to_delete_ids.sort_unstable();
					to_delete_ids.dedup();

					// Remove closure links referencing any of the subtree nodes (as ancestor or descendant)
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

						// Remove directory path cache rows
						let _ = entities::directory_paths::Entity::delete_many()
							.filter(
								entities::directory_paths::Column::EntryId
									.is_in(to_delete_ids.clone()),
							)
							.exec(&txn)
							.await;

						// Finally remove entries themselves
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
