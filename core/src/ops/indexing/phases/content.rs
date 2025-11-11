//! Content identification phase - generates CAS IDs and links content

use crate::{
	domain::content_identity::ContentHashGenerator,
	infra::job::generic_progress::ToGenericProgress,
	infra::job::prelude::{JobContext, JobError, Progress},
	ops::indexing::{
		ctx::IndexingCtx,
		entry::EntryProcessor,
		state::{IndexError, IndexPhase, IndexerProgress, IndexerState},
	},
};
use std::sync::Arc;

/// Run the content identification phase
pub async fn run_content_phase(
	state: &mut IndexerState,
	ctx: &JobContext<'_>,
	library_id: uuid::Uuid,
	volume_backend: Option<&Arc<dyn crate::volume::VolumeBackend>>,
) -> Result<(), JobError> {
	let total = state.entries_for_content.len();
	ctx.log(format!(
		"Content identification phase starting with {} files",
		total
	));

	if total == 0 {
		ctx.log("No files to process for content identification");
		state.phase = crate::ops::indexing::state::Phase::Complete;
		return Ok(());
	}

	let mut processed = 0;
	let mut success_count = 0;
	let mut error_count = 0;

	// Process in chunks for better performance and memory usage
	const CHUNK_SIZE: usize = 100;

	while !state.entries_for_content.is_empty() {
		ctx.check_interrupt().await?;

		let chunk_size = CHUNK_SIZE.min(state.entries_for_content.len());
		let chunk: Vec<_> = state.entries_for_content.drain(..chunk_size).collect();
		let chunk_len = chunk.len();

		// Report progress BEFORE processing (using current processed count)
		let indexer_progress = IndexerProgress {
			phase: IndexPhase::ContentIdentification {
				current: processed,
				total,
			},
			current_path: format!("Generating content identities ({}/{})", processed, total),
			total_found: state.stats,
			processing_rate: state.calculate_rate(),
			estimated_remaining: state.estimate_remaining(),
			scope: None,
			persistence: None,
			is_ephemeral: false,
			action_context: None, // TODO: Pass action context from job state
		};
		ctx.progress(Progress::generic(indexer_progress.to_generic_progress()));

		// Process chunk in parallel for better performance
		let content_hash_futures: Vec<_> = chunk
			.iter()
			.map(|(entry_id, path)| {
				let backend_clone = volume_backend.cloned();
				async move {
					let hash_result = if let Some(backend) = backend_clone {
						// Use backend for content hashing (supports both local and cloud)
						// Get file size first
						match backend.metadata(path).await {
							Ok(meta) => {
								ContentHashGenerator::generate_content_hash_with_backend(
									backend.as_ref(),
									path,
									meta.size,
								)
								.await
							}
							Err(e) => Err(crate::domain::ContentHashError::Io(
								std::io::Error::new(std::io::ErrorKind::Other, e),
							)),
						}
					} else {
						// No backend - use local filesystem path
						ContentHashGenerator::generate_content_hash(path).await
					};
					(*entry_id, path.clone(), hash_result)
				}
			})
			.collect();

		// Wait for all content hash generations to complete
		let hash_results = futures::future::join_all(content_hash_futures).await;

		// Collect results for batch syncing
		let mut content_identities_to_sync = Vec::new();
		let mut entries_to_sync = Vec::new();

		// Process results
		for (entry_id, path, hash_result) in hash_results {
			// Check for interruption during result processing
			ctx.check_interrupt().await?;

			match hash_result {
				Ok(content_hash) => {
					match EntryProcessor::link_to_content_identity(
						ctx,
						entry_id,
						&path,
						content_hash.clone(),
						library_id,
					)
					.await
					{
						Ok(result) => {
							ctx.log(format!(
								"Created content identity for {}: {}",
								path.display(),
								content_hash
							));

							// Collect for batch sync
							content_identities_to_sync.push(result.content_identity);
							entries_to_sync.push(result.entry);

							success_count += 1;
						}
						Err(e) => {
							let error_msg = format!(
								"Failed to create content identity for {}: {}",
								path.display(),
								e
							);
							ctx.add_non_critical_error(error_msg);
							state.add_error(IndexError::ContentId {
								path: path.to_string_lossy().to_string(),
								error: e.to_string(),
							});
							error_count += 1;
						}
					}
				}
				Err(e) => {
					let error_msg = format!(
						"Failed to generate content hash for {}: {}",
						path.display(),
						e
					);
					ctx.add_non_critical_error(error_msg);
					state.add_error(IndexError::ContentId {
						path: path.to_string_lossy().to_string(),
						error: e.to_string(),
					});
					error_count += 1;
				}
			}
		}

		// Batch sync content identities (shared resources)
		if !content_identities_to_sync.is_empty() {
			match IndexingCtx::library(ctx) {
				Some(library) => {
					match library
						.sync_models_batch(
							&content_identities_to_sync,
							crate::infra::sync::ChangeType::Insert,
							ctx.library_db(),
						)
						.await
					{
						Ok(()) => {
							ctx.log(format!(
								"Batch synced {} content identities",
								content_identities_to_sync.len()
							));
						}
						Err(e) => {
							tracing::warn!(
								"Failed to batch sync {} content identities: {}",
								content_identities_to_sync.len(),
								e
							);
						}
					}
				}
				None => {
					ctx.log("Sync disabled - content identities saved locally only");
				}
			}
		}

		// Batch sync entries (device-owned, now sync-ready with content_id assigned)
		if !entries_to_sync.is_empty() {
			match IndexingCtx::library(ctx) {
				Some(library) => {
					match library
						.sync_models_batch(
							&entries_to_sync,
							crate::infra::sync::ChangeType::Update,
							ctx.library_db(),
						)
						.await
					{
						Ok(()) => {
							ctx.log(format!(
								"Batch synced {} entries with content IDs",
								entries_to_sync.len()
							));
						}
						Err(e) => {
							tracing::warn!(
								"Failed to batch sync {} entries: {}",
								entries_to_sync.len(),
								e
							);
						}
					}
				}
				None => {
					ctx.log("Sync disabled - entries saved locally only");
				}
			}
		}

		// Update processed count AFTER processing chunk
		processed += chunk_len;

		// Update rate tracking
		state.items_since_last_update += chunk_len as u64;

		// Emit ResourceChanged events for affected Files
		if !entries_to_sync.is_empty() {
			// Collect entry UUIDs from successfully processed entries
			let entry_ids_for_events: Vec<uuid::Uuid> = entries_to_sync
				.iter()
				.filter_map(|entry_model| entry_model.uuid)
				.collect();

			if !entry_ids_for_events.is_empty() {
				let library = ctx.library();
				let events = library.event_bus().clone();
				let db = Arc::new(ctx.library_db().clone());

				let resource_manager = crate::domain::ResourceManager::new(db, events);

				if let Err(e) = resource_manager
					.emit_resource_events("entry", entry_ids_for_events)
					.await
				{
					tracing::warn!("Failed to emit resource events after content batch: {}", e);
				}
			}
		}

		// State is automatically saved during job serialization on shutdown
	}

	ctx.log(format!(
		"Content identification complete: {} successful, {} errors out of {} total",
		success_count, error_count, total
	));

	state.phase = crate::ops::indexing::state::Phase::Complete;
	Ok(())
}
