//! # Content Identification and Hashing
//!
//! `core::ops::indexing::phases::content` generates BLAKE3 content hashes for files and
//! links entries to content_identity records for deduplication. Processes files in parallel
//! chunks, supports both local filesystem and cloud backends (S3, Dropbox), and carefully
//! orders sync operations (content identities before entries) to prevent foreign key violations
//! on receiving devices.

use crate::{
	domain::content_identity::ContentHashGenerator,
	infra::job::generic_progress::ToGenericProgress,
	infra::job::prelude::{JobContext, JobError, Progress},
	ops::indexing::{
		database_storage::DatabaseStorage,
		processor::{ContentHashProcessor, ProcessorEntry},
		state::{EntryKind, IndexError, IndexPhase, IndexerProgress, IndexerState},
	},
};
use std::path::Path;
use std::sync::Arc;
use tracing::warn;

/// Strips cloud URL schemes to convert full URIs into backend-relative paths.
///
/// Backends expect relative keys ("folder/file.txt"), not full URIs ("s3://bucket/folder/file.txt").
/// For S3 paths like "s3://my-bucket/docs/report.pdf", this returns "docs/report.pdf".
/// Local paths pass through unchanged.
fn to_backend_path(path: &Path) -> std::path::PathBuf {
	let path_str = path.to_string_lossy();
	if let Some(after_scheme) = path_str.strip_prefix("s3://") {
		if let Some(slash_pos) = after_scheme.find('/') {
			let key = &after_scheme[slash_pos + 1..];
			return std::path::PathBuf::from(key);
		}
	}
	path.to_path_buf()
}

/// Generates BLAKE3 content hashes for files and links them to content identities.
///
/// Processes files in parallel chunks for throughput, uses volume backends for cloud files,
/// syncs content identities before entries (to prevent foreign key violations), and emits
/// ResourceChanged events for UI updates. Empty files are skipped (no content to hash).
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

	const CHUNK_SIZE: usize = 100;

	while !state.entries_for_content.is_empty() {
		ctx.check_interrupt().await?;

		let chunk_size = CHUNK_SIZE.min(state.entries_for_content.len());
		let chunk: Vec<_> = state.entries_for_content.drain(..chunk_size).collect();
		let chunk_len = chunk.len();

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
			action_context: None,
			volume_total_capacity: state.volume_total_capacity,
		};
		ctx.progress(Progress::generic(indexer_progress.to_generic_progress()));

		let content_hash_futures: Vec<_> = chunk
			.iter()
			.map(|(entry_id, path)| {
				let backend_clone = volume_backend.cloned();
				async move {
					let hash_result = if let Some(backend) = backend_clone {
						let backend_path = to_backend_path(path);

						match backend.metadata(&backend_path).await {
							Ok(meta) => {
								ContentHashGenerator::generate_content_hash_with_backend(
									backend.as_ref(),
									&backend_path,
									meta.size,
								)
								.await
							}
							Err(e) => Err(crate::domain::ContentHashError::Io(
								std::io::Error::new(std::io::ErrorKind::Other, e),
							)),
						}
					} else {
						ContentHashGenerator::generate_content_hash(path).await
					};
					(*entry_id, path.clone(), hash_result)
				}
			})
			.collect();

		let hash_results = futures::future::join_all(content_hash_futures).await;

		let mut mime_types_to_sync = Vec::new();
		let mut content_identities_to_sync = Vec::new();
		let mut entries_to_sync = Vec::new();

		for (entry_id, path, hash_result) in hash_results {
			ctx.check_interrupt().await?;

			match hash_result {
				Ok(content_hash) => {
					match DatabaseStorage::link_to_content_identity(
						ctx.library_db(),
						entry_id,
						&path,
						content_hash.clone(),
						ctx.library().core_context().file_type_registry(),
					)
					.await
					{
						Ok(result) => {
							ctx.log(format!(
								"Created content identity for {}: {}",
								path.display(),
								content_hash
							));

							// Collect mime_type if it was newly created
							if let Some(mime_type) = result.mime_type {
								if result.is_new_mime_type {
									mime_types_to_sync.push(mime_type);
								}
							}

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
					// Empty files are expected and shouldn't be treated as errors
					if matches!(e, crate::domain::ContentHashError::EmptyFile) {
						ctx.log(format!(
							"Skipping empty file (no content identity needed): {}",
							path.display()
						));
					} else {
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
		}

		// Sync mime_types first (content_identities depend on them via FK)
		if !mime_types_to_sync.is_empty() {
			let library = ctx.library();
			match library
				.sync_models_batch(
					&mime_types_to_sync,
					crate::infra::sync::ChangeType::Insert,
					ctx.library_db(),
				)
				.await
			{
				Ok(()) => {
					ctx.log(format!(
						"Batch synced {} mime types",
						mime_types_to_sync.len()
					));
				}
				Err(e) => {
					tracing::warn!(
						"Failed to batch sync {} mime types: {}",
						mime_types_to_sync.len(),
						e
					);
				}
			}

			// Yield to let mime_type sync messages propagate before content_identity inserts
			tokio::task::yield_now().await;
		}

		if !content_identities_to_sync.is_empty() {
			let library = ctx.library();
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

		// Yield to let content_identity sync messages propagate before entry updates.
		// Without this, receiving devices might process entry.content_id foreign keys before
		// the referenced content_identity row exists, causing foreign key constraint violations.
		tokio::task::yield_now().await;

		if !entries_to_sync.is_empty() {
			let library = ctx.library();
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

		processed += chunk_len;
		state.items_since_last_update += chunk_len as u64;

		if !entries_to_sync.is_empty() {
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
	}

	ctx.log(format!(
		"Content identification complete: {} successful, {} errors out of {} total",
		success_count, error_count, total
	));

	state.phase = crate::ops::indexing::state::Phase::Complete;
	Ok(())
}
