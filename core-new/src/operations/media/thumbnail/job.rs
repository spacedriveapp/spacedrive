//! Thumbnail generation job implementation

use crate::infrastructure::jobs::prelude::*;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use uuid::Uuid;

use super::{
	error::{ThumbnailError, ThumbnailResult},
	generator::ThumbnailGenerator,
	state::{ThumbnailEntry, ThumbnailPhase, ThumbnailState, ThumbnailStats},
};

/// Configuration for thumbnail generation job
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThumbnailJobConfig {
	/// Target thumbnail sizes to generate
	pub sizes: Vec<u32>,

	/// Quality setting (0-100)
	pub quality: u8,

	/// Whether to regenerate existing thumbnails
	pub regenerate: bool,

	/// Batch size for processing
	pub batch_size: usize,

	/// Maximum concurrent thumbnail generations
	pub max_concurrent: usize,
}

impl Default for ThumbnailJobConfig {
	fn default() -> Self {
		Self {
			sizes: vec![128, 256, 512],
			quality: 85,
			regenerate: false,
			batch_size: 50,
			max_concurrent: 4,
		}
	}
}

/// Progress information for thumbnail generation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThumbnailProgress {
	pub phase: ThumbnailPhase,
	pub generated_count: u64,
	pub skipped_count: u64,
	pub error_count: u64,
	pub total_count: u64,
	pub current_file: Option<String>,
	pub estimated_time_remaining: Option<Duration>,
}

impl JobProgress for ThumbnailProgress {}

/// Thumbnail generation job
#[derive(Debug, Serialize, Deserialize)]
pub struct ThumbnailJob {
	/// Entry IDs to process for thumbnails (if None, process all suitable entries)
	pub entry_ids: Option<Vec<Uuid>>,

	/// Job configuration
	pub config: ThumbnailJobConfig,

	// Resumable state
	#[serde(skip_serializing_if = "Option::is_none")]
	state: Option<ThumbnailState>,
}

impl Job for ThumbnailJob {
	const NAME: &'static str = "thumbnail_generation";
	const RESUMABLE: bool = true;
	const DESCRIPTION: Option<&'static str> = Some("Generate thumbnails for media files");
}

/// Output from thumbnail generation job
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThumbnailOutput {
	pub stats: ThumbnailStats,
	pub duration: Duration,
	pub errors: Vec<String>,
}

impl From<ThumbnailOutput> for JobOutput {
	fn from(output: ThumbnailOutput) -> Self {
		JobOutput::ThumbnailGeneration {
			generated_count: output.stats.generated_count,
			skipped_count: output.stats.skipped_count,
			error_count: output.stats.error_count,
			total_size_bytes: output.stats.thumbnails_size_bytes,
		}
	}
}

#[async_trait::async_trait]
impl JobHandler for ThumbnailJob {
	type Output = ThumbnailOutput;

	async fn run(&mut self, ctx: JobContext<'_>) -> JobResult<Self::Output> {
		// Initialize or restore state
		{
			let _state = self.get_or_create_state(&ctx).await?;
		}

		// Run each phase sequentially
		// Discovery phase
		if self.state.as_ref().unwrap().phase == ThumbnailPhase::Discovery {
			Self::run_discovery_phase_static(
				&self.config,
				&self.entry_ids,
				self.state.as_mut().unwrap(),
				&ctx,
			)
			.await?;
		}

		// Processing phase
		if self.state.as_ref().unwrap().phase == ThumbnailPhase::Processing {
			Self::run_processing_phase_static(&self.config, self.state.as_mut().unwrap(), &ctx)
				.await?;
		}

		// Cleanup phase
		if self.state.as_ref().unwrap().phase == ThumbnailPhase::Cleanup {
			Self::run_cleanup_phase_static(self.state.as_mut().unwrap(), &ctx).await?;
		}

		// Mark as complete and return results
		let state = self.state.as_mut().unwrap();
		state.phase = ThumbnailPhase::Complete;

		Ok(ThumbnailOutput {
			stats: state.stats.clone(),
			duration: state.started_at.elapsed(),
			errors: state.errors.clone(),
		})
	}

	async fn on_resume(&mut self, ctx: &JobContext<'_>) -> JobResult {
		if let Some(state) = &self.state {
			ctx.log(format!("Resuming thumbnail job in {:?} phase", state.phase));
			ctx.log(format!(
				"Progress: {} generated, {} skipped, {} errors",
				state.stats.generated_count, state.stats.skipped_count, state.stats.error_count
			));
		}
		Ok(())
	}

	async fn on_pause(&mut self, ctx: &JobContext<'_>) -> JobResult {
		ctx.log("Pausing thumbnail generation job - state will be preserved");
		Ok(())
	}

	async fn on_cancel(&mut self, ctx: &JobContext<'_>) -> JobResult {
		ctx.log("Cancelling thumbnail generation job");
		if let Some(state) = &self.state {
			ctx.log(format!(
				"Generated {} thumbnails before cancellation",
				state.stats.generated_count
			));
		}
		Ok(())
	}
}

impl ThumbnailJob {
	/// Create a new thumbnail job for all suitable entries
	pub fn new(config: ThumbnailJobConfig) -> Self {
		Self {
			entry_ids: None,
			config,
			state: None,
		}
	}

	/// Create a thumbnail job for specific entry IDs
	pub fn for_entries(entry_ids: Vec<Uuid>, config: ThumbnailJobConfig) -> Self {
		Self {
			entry_ids: Some(entry_ids),
			config,
			state: None,
		}
	}

	/// Create a thumbnail job with default configuration
	pub fn with_defaults() -> Self {
		Self::new(ThumbnailJobConfig::default())
	}

	/// Get or create the job state
	async fn get_or_create_state(
		&mut self,
		ctx: &JobContext<'_>,
	) -> JobResult<&mut ThumbnailState> {
		if self.state.is_none() {
			ctx.log("Initializing new thumbnail generation state");
			self.state = Some(ThumbnailState::new());
		}
		Ok(self.state.as_mut().unwrap())
	}

	/// Discovery phase: Find entries that need thumbnails
	async fn run_discovery_phase_static(
		config: &ThumbnailJobConfig,
		entry_ids: &Option<Vec<Uuid>>,
		state: &mut ThumbnailState,
		ctx: &JobContext<'_>,
	) -> JobResult<()> {
		ctx.progress(Progress::indeterminate(
			"Discovering files for thumbnail generation",
		));
		ctx.log("Starting thumbnail discovery phase");

		// Build MIME type conditions based on available features
		let mut mime_conditions = vec![
			"e.mime_type LIKE 'image/%'",
			"e.mime_type = 'application/pdf'",
		];

		#[cfg(feature = "ffmpeg")]
		{
			mime_conditions.push("e.mime_type LIKE 'video/%'");
		}

		let mime_condition = mime_conditions.join(" OR ");

		let query = if let Some(ref entry_ids) = entry_ids {
			format!(
				"SELECT e.id, ci.cas_id, e.mime_type, e.size, e.relative_path
                 FROM entries e
                 JOIN content_identity ci ON e.content_id = ci.id
                 WHERE e.id IN ({})
                 AND ci.cas_id IS NOT NULL
                 AND ({})
                 ORDER BY e.size DESC",
				entry_ids
					.iter()
					.map(|id| format!("'{}'", id))
					.collect::<Vec<_>>()
					.join(", "),
				mime_condition
			)
		} else {
			format!(
				"SELECT e.id, ci.cas_id, e.mime_type, e.size, e.relative_path
                 FROM entries e
                 JOIN content_identity ci ON e.content_id = ci.id
                 WHERE ci.cas_id IS NOT NULL
                 AND ({})
                 ORDER BY e.size DESC",
				mime_condition
			)
		};

		// This is a placeholder - in real implementation, we'd use the database
		// For now, we'll create some mock entries
		let entries = Self::mock_database_query_static(&query).await?;

		// Filter entries that already have thumbnails (unless regenerating)
		for entry in entries {
			if !config.regenerate
				&& Self::has_all_thumbnails_static(&entry.cas_id, config, ctx).await?
			{
				state.record_skipped();
				continue;
			}

			state.pending_entries.push(entry);
		}

		state.stats.discovered_count = state.pending_entries.len() as u64;

		// Create batches for processing
		state.batches = state
			.pending_entries
			.chunks(config.batch_size)
			.map(|chunk| chunk.to_vec())
			.collect();

		state.phase = ThumbnailPhase::Processing;

		ctx.log(format!(
			"Discovery complete: {} entries found, {} batches created",
			state.stats.discovered_count,
			state.batches.len()
		));
		ctx.progress(Progress::count(0, state.batches.len()));

		Ok(())
	}

	/// Processing phase: Generate thumbnails in batches
	async fn run_processing_phase_static(
		config: &ThumbnailJobConfig,
		state: &mut ThumbnailState,
		ctx: &JobContext<'_>,
	) -> JobResult<()> {
		ctx.log("Starting thumbnail processing phase");

		let batches = state.batches.clone(); // Clone to avoid borrowing issues
		let total_batches = batches.len();

		for (batch_idx, batch) in batches.iter().enumerate() {
			ctx.check_interrupt().await?;

			ctx.log(format!(
				"Processing batch {} of {} ({} entries)",
				batch_idx + 1,
				total_batches,
				batch.len()
			));

			// Process entries in the batch concurrently
			let tasks = batch
				.iter()
				.map(|entry| Self::generate_thumbnails_for_entry_static(entry, config, ctx));

			let results = futures::future::join_all(tasks).await;

			// Process results
			for (entry, result) in batch.iter().zip(results.iter()) {
				match result {
					Ok(thumbnail_size) => {
						state.record_generated(*thumbnail_size);
						ctx.log(format!("Generated thumbnail for {}", entry.relative_path));
					}
					Err(e) => {
						let error_msg = format!(
							"Failed to generate thumbnail for {}: {}",
							entry.relative_path, e
						);
						state.add_error(error_msg.clone());
						ctx.add_non_critical_error(
							crate::infrastructure::jobs::error::JobError::execution(error_msg),
						);
					}
				}
			}

			// Update progress
			ctx.progress(Progress::count(batch_idx + 1, total_batches));

			// Update detailed progress
			let progress = ThumbnailProgress {
				phase: state.phase.clone(),
				generated_count: state.stats.generated_count,
				skipped_count: state.stats.skipped_count,
				error_count: state.stats.error_count,
				total_count: state.stats.discovered_count,
				current_file: batch.last().map(|e| e.relative_path.clone()),
				estimated_time_remaining: None, // TODO: Calculate ETA
			};
			let progress_json = serde_json::to_value(progress).unwrap_or(serde_json::Value::Null);
			ctx.progress(Progress::Structured(progress_json));

			// Checkpoint every 10 batches
			if batch_idx % 10 == 0 {
				ctx.checkpoint().await?;
			}
		}

		state.phase = ThumbnailPhase::Cleanup;
		ctx.log("Processing phase complete");

		Ok(())
	}

	/// Cleanup phase: Remove orphaned thumbnails
	async fn run_cleanup_phase_static(
		_state: &mut ThumbnailState,
		ctx: &JobContext<'_>,
	) -> JobResult<()> {
		ctx.log("Starting cleanup phase");
		ctx.progress(Progress::indeterminate("Cleaning up orphaned thumbnails"));

		// TODO: Implement cleanup logic
		// - Find thumbnails that don't have corresponding entries
		// - Remove old thumbnails if regenerating

		ctx.log("Cleanup phase complete");
		Ok(())
	}

	/// Check if all required thumbnails exist for a CAS ID
	async fn has_all_thumbnails_static(
		cas_id: &str,
		config: &ThumbnailJobConfig,
		ctx: &JobContext<'_>,
	) -> JobResult<bool> {
		let library = ctx.library();
		for &size in &config.sizes {
			if !library.has_thumbnail(cas_id, size).await {
				return Ok(false);
			}
		}
		Ok(true)
	}

	/// Generate thumbnails for a single entry
	async fn generate_thumbnails_for_entry_static(
		entry: &ThumbnailEntry,
		config: &ThumbnailJobConfig,
		ctx: &JobContext<'_>,
	) -> ThumbnailResult<u64> {
		use super::generator::ThumbnailGenerator;
		use super::utils::ThumbnailUtils;

		// Validate parameters
		for &size in &config.sizes {
			ThumbnailUtils::validate_thumbnail_params(size, config.quality)?;
		}

		// Create appropriate generator for the file type
		let generator = ThumbnailGenerator::for_mime_type(&entry.mime_type)?;

		// Get the full path to the source file
		let library = ctx.library();
		let source_path = library.path().join(&entry.relative_path);

		if !source_path.exists() {
			return Err(ThumbnailError::FileNotFound(entry.relative_path.clone()));
		}

		let mut total_thumbnail_size = 0u64;

		// Generate thumbnails for each configured size
		for &size in &config.sizes {
			// Skip if thumbnail already exists (unless regenerating)
			if !config.regenerate && library.has_thumbnail(&entry.cas_id, size).await {
				continue;
			}

			// Build thumbnail output path
			let thumbnail_path = library.thumbnail_path(&entry.cas_id, size);

			// Ensure directory exists
			ThumbnailUtils::ensure_thumbnail_dirs(&thumbnail_path).await?;

			// Generate the thumbnail
			let thumbnail_info = generator
				.generate(&source_path, &thumbnail_path, size, config.quality)
				.await?;

			total_thumbnail_size += thumbnail_info.size_bytes as u64;

			ctx.log(format!(
				"Generated {}x{} thumbnail for {} ({}KB)",
				thumbnail_info.dimensions.0,
				thumbnail_info.dimensions.1,
				entry.relative_path,
				thumbnail_info.size_bytes / 1024
			));
		}

		Ok(total_thumbnail_size)
	}

	/// Mock database query for development
	async fn mock_database_query_static(_query: &str) -> JobResult<Vec<ThumbnailEntry>> {
		// TODO: Replace with actual database query
		let mut entries = vec![ThumbnailEntry {
			entry_id: Uuid::new_v4(),
			cas_id: "abc123def456".to_string(),
			mime_type: "image/jpeg".to_string(),
			file_size: 1024 * 1024,
			relative_path: "photos/vacation.jpg".to_string(),
		}];

		// Only add video entry if FFmpeg feature is enabled
		#[cfg(feature = "ffmpeg")]
		{
			entries.push(ThumbnailEntry {
				entry_id: Uuid::new_v4(),
				cas_id: "def456ghi789".to_string(),
				mime_type: "video/mp4".to_string(),
				file_size: 10 * 1024 * 1024,
				relative_path: "videos/movie.mp4".to_string(),
			});
		}

		#[cfg(not(feature = "ffmpeg"))]
		{
			let _ = &mut entries; // Suppress unused variable warning
		}

		Ok(entries)
	}
}
