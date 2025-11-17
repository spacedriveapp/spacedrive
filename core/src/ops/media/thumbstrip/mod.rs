//! Thumbstrip generation system
//!
//! Generates grid-based storyboard thumbnails from video files for quick visual overview
//! and timeline scrubbing. Operates as both a processor (for responder) and a job
//! (for batch operations).

pub mod action;
mod config;
mod error;
mod generator;
pub mod job;
pub mod processor;
mod state;

pub use action::GenerateThumbstripAction;
pub use config::{ThumbstripJobConfig, ThumbstripVariantConfig, ThumbstripVariants};
pub use error::{ThumbstripError, ThumbstripResult};
pub use generator::{ThumbstripGenerator, ThumbstripInfo};
pub use job::ThumbstripJob;
pub use processor::ThumbstripProcessor;
pub use state::{ThumbstripPhase, ThumbstripState};

use crate::library::Library;
use crate::ops::sidecar::types::SidecarKind;
use std::path::Path;
use std::sync::Arc;
use tracing::{debug, warn};
use uuid::Uuid;

/// Generate thumbstrip for a single video file
///
/// This function generates thumbstrip variants for a single file and registers
/// them as sidecars. Used by both ThumbstripProcessor (responder) and
/// ThumbstripJob (batch operations).
///
/// # Arguments
/// * `library` - The library context
/// * `content_uuid` - The UUID of the content identity
/// * `video_path` - The filesystem path to the video file
/// * `variants` - The thumbstrip variants to generate
/// * `regenerate` - Whether to regenerate existing thumbstrips
///
/// # Returns
/// Number of thumbstrips successfully generated
#[cfg(feature = "ffmpeg")]
pub async fn generate_thumbstrip_for_file(
	library: &Arc<Library>,
	content_uuid: &Uuid,
	video_path: &Path,
	variants: &[ThumbstripVariantConfig],
	regenerate: bool,
) -> ThumbstripResult<usize> {
	// Get sidecar manager
	let sidecar_manager = library
		.core_context()
		.get_sidecar_manager()
		.await
		.ok_or_else(|| ThumbstripError::other("SidecarManager not available"))?;

	let mut generated_count = 0;

	for variant_config in variants {
		// Check if already exists
		if !regenerate
			&& sidecar_manager
				.exists(
					&library.id(),
					content_uuid,
					&SidecarKind::Thumbstrip,
					&variant_config.variant,
					&variant_config.format(),
				)
				.await
				.unwrap_or(false)
		{
			debug!(
				"Thumbstrip already exists for {}: {}",
				content_uuid,
				variant_config.variant.as_str()
			);
			continue;
		}

		// Compute output path
		let output_path = sidecar_manager
			.compute_path(
				&library.id(),
				content_uuid,
				&SidecarKind::Thumbstrip,
				&variant_config.variant,
				&variant_config.format(),
			)
			.await
			.map_err(|e| ThumbstripError::other(format!("Failed to compute path: {}", e)))?;

		// Ensure parent directory exists
		if let Some(parent) = output_path.absolute_path.parent() {
			tokio::fs::create_dir_all(parent).await?;
		}

		// Generate thumbstrip using generator
		let generator = ThumbstripGenerator::new(variant_config.clone());
		match generator
			.generate(video_path, &output_path.absolute_path)
			.await
		{
			Ok(thumbstrip_info) => {
				// Record the sidecar
				if let Err(e) = sidecar_manager
					.record_sidecar(
						library,
						content_uuid,
						&SidecarKind::Thumbstrip,
						&variant_config.variant,
						&variant_config.format(),
						thumbstrip_info.size_bytes as u64,
						None,
					)
					.await
				{
					warn!(
						"Failed to record sidecar for {}: {}",
						variant_config.variant.as_str(),
						e
					);
				} else {
					debug!(
						"✓ Generated thumbstrip {}: {}x{} ({} frames, {} KB)",
						variant_config.variant.as_str(),
						thumbstrip_info.dimensions.0,
						thumbstrip_info.dimensions.1,
						thumbstrip_info.total_frames,
						thumbstrip_info.size_bytes / 1024
					);
					generated_count += 1;
				}
			}
			Err(e) => {
				warn!(
					"Failed to generate thumbstrip {} for {}: {}",
					variant_config.variant.as_str(),
					video_path.display(),
					e
				);
			}
		}
	}

	Ok(generated_count)
}

/// Generate thumbstrip without ffmpeg feature (returns error)
#[cfg(not(feature = "ffmpeg"))]
pub async fn generate_thumbstrip_for_file(
	_library: &Arc<Library>,
	_content_uuid: &Uuid,
	_video_path: &Path,
	_variants: &[ThumbstripVariantConfig],
	_regenerate: bool,
) -> ThumbstripResult<usize> {
	Err(ThumbstripError::other(
		"Thumbstrip generation requires FFmpeg feature to be enabled",
	))
}
