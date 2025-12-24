//! Ephemeral thumbnail generation job
//!
//! Generates thumbnails on-demand for ephemeral entries (files in unmanaged
//! locations). Unlike the managed thumbnail job that processes entire
//! locations, this job generates thumbnails only for visible viewport items.
//!
//! ## Design Rationale
//!
//! Ephemeral thumbnails are stored by entry UUID in the system temp directory,
//! not by content hash in the library folder. This avoids database overhead
//! for temporary browsing sessions while still providing smooth UX.

use crate::{
	infra::{event::Event, job::prelude::*},
	ops::indexing::ephemeral::{EphemeralIndex, EphemeralSidecarCache},
};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, path::PathBuf, sync::Arc};
use tokio::sync::Semaphore;
use tracing::{debug, warn};
use uuid::Uuid;

use super::{
	config::ThumbnailVariantConfig,
	error::{ThumbnailError, ThumbnailResult},
	generator::ThumbnailGenerator,
};

/// Ephemeral thumbnail generation job
///
/// Generates thumbnails for ephemeral entries on-demand. The job is
/// non-resumable since it's triggered by viewport changes and completes
/// quickly (typically <1s for a viewport of ~50 items).
#[derive(Debug, Serialize, Deserialize)]
pub struct EphemeralThumbnailJob {
	/// Entry UUIDs to generate thumbnails for
	pub entry_uuids: Vec<Uuid>,

	/// Target variant config
	pub variant_config: ThumbnailVariantConfig,

	/// Library ID
	pub library_id: Uuid,

	/// Maximum concurrent generations
	#[serde(default = "default_max_concurrent")]
	pub max_concurrent: usize,
}

fn default_max_concurrent() -> usize {
	4
}

impl Job for EphemeralThumbnailJob {
	const NAME: &'static str = "ephemeral_thumbnail_generation";
	const RESUMABLE: bool = false;
	const DESCRIPTION: Option<&'static str> = Some("Generate thumbnails for ephemeral entries");
}

impl crate::infra::job::traits::DynJob for EphemeralThumbnailJob {
	fn job_name(&self) -> &'static str {
		Self::NAME
	}
}

/// Output from ephemeral thumbnail generation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EphemeralThumbnailOutput {
	/// Number of thumbnails generated
	pub generated_count: usize,
	/// Number skipped (already existed)
	pub skipped_count: usize,
	/// Number of errors
	pub error_count: usize,
	/// Total size of generated thumbnails in bytes
	pub total_size_bytes: u64,
}

impl From<EphemeralThumbnailOutput> for JobOutput {
	fn from(output: EphemeralThumbnailOutput) -> Self {
		JobOutput::ThumbnailGeneration {
			generated_count: output.generated_count as u64,
			skipped_count: output.skipped_count as u64,
			error_count: output.error_count as u64,
			total_size_bytes: output.total_size_bytes,
		}
	}
}

#[async_trait::async_trait]
impl JobHandler for EphemeralThumbnailJob {
	type Output = EphemeralThumbnailOutput;

	async fn run(&mut self, ctx: JobContext<'_>) -> JobResult<Self::Output> {
		debug!(
			"Starting ephemeral thumbnail generation for {} entries",
			self.entry_uuids.len()
		);

		// Get the ephemeral index and sidecar cache
		let ephemeral_cache = ctx.library().core_ctx().ephemeral_cache.get_global_index();
		let sidecar_cache = ctx
			.library()
			.core_ctx()
			.ephemeral_cache
			.get_sidecar_cache(self.library_id);

		// Resolve entry UUIDs to paths
		let entries_to_process = self
			.resolve_entry_paths(&ephemeral_cache, &sidecar_cache)
			.await?;

		if entries_to_process.is_empty() {
			return Ok(EphemeralThumbnailOutput {
				generated_count: 0,
				skipped_count: self.entry_uuids.len(),
				error_count: 0,
				total_size_bytes: 0,
			});
		}

		debug!(
			"Resolved {} entries to process (skipped {} already existing)",
			entries_to_process.len(),
			self.entry_uuids.len() - entries_to_process.len()
		);

		// Generate thumbnails with concurrency limit
		let results = self
			.generate_thumbnails(entries_to_process, &sidecar_cache, &ctx)
			.await;

		// Aggregate results
		let mut output = EphemeralThumbnailOutput {
			generated_count: 0,
			skipped_count: self.entry_uuids.len() - results.len(),
			error_count: 0,
			total_size_bytes: 0,
		};

		for result in results {
			match result {
				Ok(size) => {
					output.generated_count += 1;
					output.total_size_bytes += size;
				}
				Err(_) => {
					output.error_count += 1;
				}
			}
		}

		debug!(
			"Ephemeral thumbnail generation complete: {} generated, {} skipped, {} errors",
			output.generated_count, output.skipped_count, output.error_count
		);

		Ok(output)
	}
}

impl EphemeralThumbnailJob {
	/// Resolve entry UUIDs to filesystem paths via ephemeral index
	///
	/// Filters out entries that already have thumbnails or no longer exist.
	async fn resolve_entry_paths(
		&self,
		ephemeral_index: &Arc<tokio::sync::RwLock<EphemeralIndex>>,
		sidecar_cache: &Arc<EphemeralSidecarCache>,
	) -> JobResult<Vec<(Uuid, PathBuf)>> {
		let index = ephemeral_index.read().await;
		let mut entries = Vec::new();

		for &entry_uuid in &self.entry_uuids {
			// Skip if thumbnail already exists
			if sidecar_cache.has(&entry_uuid, "thumb", &self.variant_config.name) {
				continue;
			}

			// Resolve UUID to path
			if let Some(path) = index.get_path_by_uuid(entry_uuid) {
				entries.push((entry_uuid, path));
			} else {
				warn!("Entry UUID {} not found in ephemeral index", entry_uuid);
			}
		}

		Ok(entries)
	}

	/// Generate thumbnails with concurrency control
	///
	/// Processes entries in parallel batches, emitting events as each
	/// thumbnail completes. The semaphore limits concurrent I/O operations
	/// to prevent overwhelming the filesystem on slow network shares.
	async fn generate_thumbnails(
		&self,
		entries: Vec<(Uuid, PathBuf)>,
		sidecar_cache: &Arc<EphemeralSidecarCache>,
		ctx: &JobContext<'_>,
	) -> Vec<ThumbnailResult<u64>> {
		let semaphore = Arc::new(Semaphore::new(self.max_concurrent));
		let mut handles = Vec::new();

		for (entry_uuid, source_path) in entries {
			let sem = semaphore.clone();
			let cache = sidecar_cache.clone();
			let variant_config = self.variant_config.clone();
			let library_id = self.library_id;
			let event_bus = ctx.library().core_ctx().event_bus.clone();

			let handle = tokio::spawn(async move {
				let _permit = sem.acquire().await.unwrap();

				// Determine MIME type from file extension
				let mime_type = mime_guess::from_path(&source_path)
					.first()
					.map(|m| m.to_string())
					.unwrap_or_else(|| "application/octet-stream".to_string());

				// Create generator for this file type
				let generator = match ThumbnailGenerator::for_mime_type(&mime_type) {
					Ok(gen) => gen,
					Err(e) => return Err(e),
				};

				// Compute output path
				let output_path =
					cache.compute_path(&entry_uuid, "thumb", &variant_config.name, "webp");

				// Generate thumbnail
				let thumbnail_info = generator
					.generate(
						&source_path,
						&output_path,
						variant_config.size,
						variant_config.quality,
					)
					.await?;

				// Update cache
				cache.insert(entry_uuid, "thumb".to_string(), variant_config.name.clone());

				// Emit event
				let _ = event_bus
					.emit(Event::EphemeralSidecarGenerated {
						library_id,
						entry_uuid,
						kind: "thumb".to_string(),
						variant: variant_config.name.clone(),
						format: thumbnail_info.format.clone(),
						size: thumbnail_info.size_bytes as u64,
					})
					.await;

				Ok(thumbnail_info.size_bytes as u64)
			});

			handles.push(handle);
		}

		// Wait for all thumbnails to complete
		let mut results = Vec::new();
		for handle in handles {
			match handle.await {
				Ok(result) => results.push(result),
				Err(e) => {
					results.push(Err(ThumbnailError::other(format!(
						"Task join error: {}",
						e
					))));
				}
			}
		}

		results
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_default_max_concurrent() {
		assert_eq!(default_max_concurrent(), 4);
	}

	#[test]
	fn test_job_constants() {
		assert_eq!(
			EphemeralThumbnailJob::NAME,
			"ephemeral_thumbnail_generation"
		);
		assert!(!EphemeralThumbnailJob::RESUMABLE);
	}
}
