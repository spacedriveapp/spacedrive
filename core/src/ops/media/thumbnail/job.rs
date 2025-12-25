//! Thumbnail generation job implementation

use crate::{
	infra::job::prelude::*,
	ops::sidecar::types::{SidecarKind, SidecarStatus, SidecarVariant},
};
use serde::{Deserialize, Serialize};
use std::time::Duration;
use uuid::Uuid;

use super::{
	config::{ThumbnailVariantConfig, ThumbnailVariants},
	error::{ThumbnailError, ThumbnailResult},
	generator::ThumbnailGenerator,
	state::{ThumbnailEntry, ThumbnailPhase, ThumbnailState, ThumbnailStats},
};

/// Configuration for thumbnail generation job
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThumbnailJobConfig {
	/// Target thumbnail variants to generate
	#[serde(skip)]
	pub variants: Vec<ThumbnailVariantConfig>,

	/// Whether to regenerate existing thumbnails
	pub regenerate: bool,

	/// Batch size for processing
	pub batch_size: usize,

	/// Maximum concurrent thumbnail generations
	pub max_concurrent: usize,

	/// Whether to run this job in the background (not persisted to database, no UI updates)
	#[serde(default)]
	pub run_in_background: bool,
}

impl Default for ThumbnailJobConfig {
	fn default() -> Self {
		Self {
			variants: ThumbnailVariants::defaults(),
			regenerate: false,
			batch_size: 50,
			max_concurrent: 4,
			run_in_background: false,
		}
	}
}

impl ThumbnailJobConfig {
	/// Create a config with all standard variants
	pub fn all_variants() -> Self {
		Self {
			variants: ThumbnailVariants::all(),
			..Default::default()
		}
	}

	/// Create a config with specific variants
	pub fn with_variants(variants: Vec<ThumbnailVariantConfig>) -> Self {
		Self {
			variants,
			..Default::default()
		}
	}

	/// Create a config from legacy size list (for backward compatibility)
	pub fn from_sizes(sizes: Vec<u32>) -> Self {
		let variants = sizes
			.into_iter()
			.filter_map(|size| ThumbnailVariants::from_size(size))
			.collect();
		Self {
			variants,
			..Default::default()
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

impl crate::infra::job::traits::DynJob for ThumbnailJob {
	fn job_name(&self) -> &'static str {
		Self::NAME
	}

	fn should_persist(&self) -> bool {
		!self.config.run_in_background
	}
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

		use crate::infra::db::entities::{content_identity, entry};
		use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder, QuerySelect};

		let library = ctx.library();
		let db = library.db().conn();

		// Build query for entries with content
		let mut query = entry::Entity::find()
			.find_also_related(content_identity::Entity)
			.filter(content_identity::Column::Uuid.is_not_null());

		// Filter by specific entry UUIDs if provided
		if let Some(ref ids) = entry_ids {
			query = query.filter(entry::Column::Uuid.is_in(ids.clone()));
		}

		// Filter by file kind (0 = File) and supported extensions
		// TODO: Add proper MIME type support to Entry model
		query = query
			.filter(entry::Column::Kind.eq(0)) // Only files
			.order_by_desc(entry::Column::Size);

		// Execute query
		let results = query
			.all(db)
			.await
			.map_err(|e| JobError::execution(format!("Database query failed: {}", e)))?;

		ctx.log(format!(
			"Query returned {} entries with potential content",
			results.len()
		));

		// Process results and check for existing sidecars
		let sidecar_manager = library
			.core_context()
			.get_sidecar_manager()
			.await
			.ok_or_else(|| JobError::execution("SidecarManager not available"))?;

		let mut skipped_no_content = 0;
		let mut skipped_no_uuid = 0;

		for (entry_model, content_opt) in results {
			let content = match content_opt {
				Some(c) => c,
				None => {
					skipped_no_content += 1;
					continue;
				}
			};

			// Skip if content doesn't have a UUID yet
			let content_uuid = match content.uuid {
				Some(uuid) => uuid,
				None => {
					skipped_no_uuid += 1;
					continue;
				}
			};

			// Get full path for this entry
			use crate::ops::indexing::PathResolver;
			let full_path = match PathResolver::get_full_path(db, entry_model.id).await {
				Ok(p) => p,
				Err(e) => {
					ctx.log(format!(
						"Failed to resolve path for entry {}: {}",
						entry_model.id, e
					));
					continue;
				}
			};

			// Check if all required sidecar variants exist
			if !config.regenerate {
				let mut all_exist = true;
				for variant_config in &config.variants {
					if !sidecar_manager
						.exists(
							&library.id(),
							&content_uuid,
							&SidecarKind::Thumb,
							&variant_config.variant,
							&variant_config.format(),
						)
						.await
						.unwrap_or(false)
					{
						all_exist = false;
						break;
					}
				}

				if all_exist {
					state.record_skipped();
					continue;
				}
			}

			state.pending_entries.push(ThumbnailEntry {
				entry_id: entry_model.uuid.unwrap_or_else(Uuid::new_v4),
				content_uuid,
				content_kind_id: content.kind_id,
				extension: entry_model.extension,
				file_size: entry_model.size as u64,
				relative_path: full_path.to_string_lossy().to_string(),
			});
		}

		ctx.log(format!(
			"Discovery filtering: skipped {} (no content), {} (no UUID)",
			skipped_no_content, skipped_no_uuid
		));

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

		// Send progress update for start of processing phase
		let progress = ThumbnailProgress {
			phase: state.phase.clone(),
			generated_count: state.stats.generated_count,
			skipped_count: state.stats.skipped_count,
			error_count: state.stats.error_count,
			total_count: state.stats.discovered_count,
			current_file: None,
			estimated_time_remaining: None,
		};

		use crate::infra::job::generic_progress::ToGenericProgress;
		ctx.progress(Progress::generic(progress.to_generic_progress()));

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
						ctx.add_non_critical_error(crate::infra::job::error::JobError::execution(
							error_msg,
						));
					}
				}
			}

			// Update progress with generic progress
			let progress = ThumbnailProgress {
				phase: state.phase.clone(),
				generated_count: state.stats.generated_count,
				skipped_count: state.stats.skipped_count,
				error_count: state.stats.error_count,
				total_count: state.stats.discovered_count,
				current_file: batch.last().map(|e| e.relative_path.clone()),
				estimated_time_remaining: None, // TODO: Calculate ETA
			};

			use crate::infra::job::generic_progress::ToGenericProgress;
			ctx.progress(Progress::generic(progress.to_generic_progress()));

			// Emit ResourceChanged events for affected Files after each batch
			if !batch.is_empty() {
				let entry_uuids: Vec<uuid::Uuid> =
					batch.iter().map(|entry| entry.entry_id).collect();

				if !entry_uuids.is_empty() {
					let library = ctx.library();
					let events = library.event_bus().clone();
					let db = std::sync::Arc::new(ctx.library_db().clone());

					let resource_manager = crate::domain::ResourceManager::new(db, events);

					// Emit events for sidecar changes (which map to files)
					// The sidecar UUIDs are in the database, and the ResourceManager
					// will map them to the affected file entries
					if let Err(e) = resource_manager
						.emit_resource_events("entry", entry_uuids)
						.await
					{
						tracing::warn!(
							"Failed to emit resource events after thumbnail batch: {}",
							e
						);
					}
				}
			}

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

	/// Strip cloud URL prefix from path to get backend-relative path
	fn to_backend_path(path: &str) -> std::path::PathBuf {
		if let Some(after_scheme) = path.strip_prefix("s3://") {
			// Strip s3://bucket/ prefix to get just the key
			if let Some(slash_pos) = after_scheme.find('/') {
				let key = &after_scheme[slash_pos + 1..];
				return std::path::PathBuf::from(key);
			}
		}
		// Add support for other cloud services if needed
		// For now, return as-is for local paths
		std::path::PathBuf::from(path)
	}

	/// Check if a path is a cloud path (starts with a cloud scheme)
	fn is_cloud_path(path: &str) -> bool {
		path.starts_with("s3://")
			|| path.starts_with("gdrive://")
			|| path.starts_with("dropbox://")
			|| path.starts_with("onedrive://")
			|| path.starts_with("gcs://")
			|| path.starts_with("azblob://")
			|| path.starts_with("b2://")
			|| path.starts_with("wasabi://")
			|| path.starts_with("spaces://")
	}

	/// Generate thumbnails for a single entry
	async fn generate_thumbnails_for_entry_static(
		entry: &ThumbnailEntry,
		config: &ThumbnailJobConfig,
		ctx: &JobContext<'_>,
	) -> ThumbnailResult<u64> {
		use super::generator::ThumbnailGenerator;
		use super::utils::ThumbnailUtils;

		// Get library and sidecar manager
		let library = ctx.library();
		let sidecar_manager = library
			.core_context()
			.get_sidecar_manager()
			.await
			.ok_or_else(|| ThumbnailError::other("SidecarManager not available"))?;

		// Determine MIME type from extension
		let mime_type =
			entry
				.extension
				.as_ref()
				.and_then(|ext| match ext.to_lowercase().as_str() {
					"jpg" | "jpeg" => Some("image/jpeg"),
					"png" => Some("image/png"),
					"gif" => Some("image/gif"),
					"webp" => Some("image/webp"),
					"bmp" => Some("image/bmp"),
					"pdf" => Some("application/pdf"),
					#[cfg(feature = "ffmpeg")]
					"mp4" | "mov" | "avi" | "mkv" | "webm" | "flv" | "wmv" | "m4v" => Some("video/mp4"),
					_ => None,
				});

		// Skip files with unsupported extensions
		let mime_type = match mime_type {
			Some(mt) => mt,
			None => {
				// Skip unsupported file types
				return Ok(0);
			}
		};

		// Create appropriate generator for the file type
		let generator = ThumbnailGenerator::for_mime_type(mime_type)?;

		// Check if this is a cloud path or local path
		let is_cloud = Self::is_cloud_path(&entry.relative_path);

		// For cloud files, download to temp file. For local files, use direct path
		let (source_path, temp_file): (std::path::PathBuf, Option<tempfile::NamedTempFile>) =
			if is_cloud {
				// Cloud path - need to download via volume backend
				let volume_manager = ctx.volume_manager().ok_or_else(|| {
					ThumbnailError::other("VolumeManager not available for cloud file")
				})?;

				// Parse the cloud path to get an SdPath
				use crate::domain::addressing::SdPath;
				let sdpath =
					SdPath::from_uri_with_context(&entry.relative_path, &library.core_context())
						.await
						.map_err(|e| {
							ThumbnailError::other(format!("Failed to parse cloud path: {}", e))
						})?;

				// Resolve the volume backend for this path
				let volume = volume_manager
					.resolve_volume_for_sdpath(&sdpath, &library)
					.await
					.map_err(|e| ThumbnailError::other(format!("Failed to resolve volume: {}", e)))?
					.ok_or_else(|| ThumbnailError::other("No volume found for cloud path"))?;

				let backend = volume
					.backend
					.as_ref()
					.ok_or_else(|| ThumbnailError::other("Volume has no backend"))?;

				// Get the backend-relative path (strip s3://bucket/ prefix)
				let backend_path = Self::to_backend_path(&entry.relative_path);

				// Download file content from cloud
				let file_data = backend.read(&backend_path).await.map_err(|e| {
					ThumbnailError::other(format!("Failed to read cloud file: {}", e))
				})?;

				// Write to temporary file
				let mut temp = tempfile::NamedTempFile::new().map_err(|e| {
					ThumbnailError::other(format!("Failed to create temp file: {}", e))
				})?;

				use std::io::Write;
				temp.write_all(&file_data).map_err(|e| {
					ThumbnailError::other(format!("Failed to write temp file: {}", e))
				})?;
				temp.flush().map_err(|e| {
					ThumbnailError::other(format!("Failed to flush temp file: {}", e))
				})?;

				let temp_path = temp.path().to_path_buf();
				ctx.log(format!(
					"Downloaded cloud file {} to temp location",
					entry.relative_path
				));

				(temp_path, Some(temp))
			} else {
				// Local path - use direct filesystem access
				let source_path = library.path().join(&entry.relative_path);

				if !source_path.exists() {
					return Err(ThumbnailError::FileNotFound(entry.relative_path.clone()));
				}

				(source_path, None)
			};

		let mut total_thumbnail_size = 0u64;

		// Generate thumbnails for each configured variant
		for variant_config in &config.variants {
			// Validate parameters
			ThumbnailUtils::validate_thumbnail_params(variant_config.size, variant_config.quality)?;

			// Skip if thumbnail already exists (unless regenerating)
			if !config.regenerate
				&& sidecar_manager
					.exists(
						&library.id(),
						&entry.content_uuid,
						&SidecarKind::Thumb,
						&variant_config.variant,
						&variant_config.format(),
					)
					.await
					.unwrap_or(false)
			{
				continue;
			}

			// Compute sidecar path
			let sidecar_path = sidecar_manager
				.compute_path(
					&library.id(),
					&entry.content_uuid,
					&SidecarKind::Thumb,
					&variant_config.variant,
					&variant_config.format(),
				)
				.await
				.map_err(|e| ThumbnailError::other(format!("Path computation failed: {}", e)))?;

			let thumbnail_path = sidecar_path.absolute_path;

			// Ensure directory exists
			ThumbnailUtils::ensure_thumbnail_dirs(&thumbnail_path).await?;

			// Generate the thumbnail
			let thumbnail_info = generator
				.generate(
					&source_path,
					&thumbnail_path,
					variant_config.size,
					variant_config.quality,
				)
				.await?;

			// Record the sidecar in the database
			sidecar_manager
				.record_sidecar(
					library,
					&entry.content_uuid,
					&SidecarKind::Thumb,
					&variant_config.variant,
					&variant_config.format(),
					thumbnail_info.size_bytes as u64,
					None, // checksum
				)
				.await
				.map_err(|e| ThumbnailError::other(format!("Failed to record sidecar: {}", e)))?;

			total_thumbnail_size += thumbnail_info.size_bytes as u64;

			ctx.log(format!(
				"Generated {} thumbnail ({}x{}) for {} ({}KB)",
				variant_config.variant.as_str(),
				thumbnail_info.dimensions.0,
				thumbnail_info.dimensions.1,
				entry.relative_path,
				thumbnail_info.size_bytes / 1024
			));
		}

		// Extract and store media metadata (only on first variant to avoid duplicate work)
		if !config.variants.is_empty() {
			let db = library.db().conn();

			// Use content_kind_id from the entry (1=Image, 2=Video, 3=Audio)
			match entry.content_kind_id {
				1 => {
					// Image
					let media_data_uuid = Uuid::new_v5(&entry.content_uuid, b"image");
					use crate::ops::media::extract_image_metadata;
					match extract_image_metadata(&source_path, media_data_uuid).await {
						Ok(image_data) => {
							use crate::infra::db::entities::{content_identity, image_media_data};
							use sea_orm::{
								ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, Set,
							};

							// Insert image media data
							if let Ok(inserted) = image_data.insert(db).await {
								// Update content identity with FK
								if let Ok(Some(content)) = content_identity::Entity::find()
									.filter(content_identity::Column::Uuid.eq(entry.content_uuid))
									.one(db)
									.await
								{
									let mut active: content_identity::ActiveModel = content.into();
									active.image_media_data_id = Set(Some(inserted.id));
									let _ = active.update(db).await;

									ctx.log(format!(
										"Extracted image metadata for {}",
										entry.relative_path
									));
								}
							}
						}
						Err(e) => {
							ctx.log(format!("Failed to extract image metadata: {}", e));
						}
					}
				}
				2 => {
					// Video
					#[cfg(feature = "ffmpeg")]
					{
						let media_data_uuid = Uuid::new_v5(&entry.content_uuid, b"video");
						use crate::ops::media::extract_video_metadata;
						match extract_video_metadata(&source_path, media_data_uuid).await {
							Ok(video_data) => {
								use crate::infra::db::entities::{
									content_identity, video_media_data,
								};
								use sea_orm::{
									ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, Set,
								};

								// Insert video media data
								if let Ok(inserted) = video_data.insert(db).await {
									// Update content identity with FK
									if let Ok(Some(content)) = content_identity::Entity::find()
										.filter(
											content_identity::Column::Uuid.eq(entry.content_uuid),
										)
										.one(db)
										.await
									{
										let mut active: content_identity::ActiveModel =
											content.into();
										active.video_media_data_id = Set(Some(inserted.id));
										let _ = active.update(db).await;

										ctx.log(format!(
											"Extracted video metadata for {}",
											entry.relative_path
										));
									}
								}
							}
							Err(e) => {
								ctx.log(format!("Failed to extract video metadata: {}", e));
							}
						}
					}
				}
				3 => {
					// Audio
					#[cfg(feature = "ffmpeg")]
					{
						let media_data_uuid = Uuid::new_v5(&entry.content_uuid, b"audio");
						use crate::ops::media::extract_audio_metadata;
						match extract_audio_metadata(&source_path, media_data_uuid).await {
							Ok(audio_data) => {
								use crate::infra::db::entities::{
									audio_media_data, content_identity,
								};
								use sea_orm::{
									ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, Set,
								};

								// Insert audio media data
								if let Ok(inserted) = audio_data.insert(db).await {
									// Update content identity with FK
									if let Ok(Some(content)) = content_identity::Entity::find()
										.filter(
											content_identity::Column::Uuid.eq(entry.content_uuid),
										)
										.one(db)
										.await
									{
										let mut active: content_identity::ActiveModel =
											content.into();
										active.audio_media_data_id = Set(Some(inserted.id));
										let _ = active.update(db).await;

										ctx.log(format!(
											"Extracted audio metadata for {}",
											entry.relative_path
										));
									}
								}
							}
							Err(e) => {
								ctx.log(format!("Failed to extract audio metadata: {}", e));
							}
						}
					}
				}
				_ => {}
			}
		}

		// Temp file (if any) is automatically cleaned up when dropped here
		drop(temp_file);

		Ok(total_thumbnail_size)
	}
}
