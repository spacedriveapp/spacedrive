//! Thumbstrip generation system
//!
//! Generates grid-based storyboard thumbnails from video files for quick visual overview
//! and timeline scrubbing. Operates as both a processor (for responder) and a job
//! (for batch operations).

pub mod action;

#[cfg(feature = "ffmpeg")]
mod config;
#[cfg(feature = "ffmpeg")]
mod error;
#[cfg(feature = "ffmpeg")]
mod generator;
#[cfg(feature = "ffmpeg")]
pub mod job;
#[cfg(feature = "ffmpeg")]
pub mod processor;
#[cfg(feature = "ffmpeg")]
mod state;

pub use action::GenerateThumbstripAction;

#[cfg(feature = "ffmpeg")]
pub use config::{ThumbstripJobConfig, ThumbstripVariantConfig, ThumbstripVariants};
#[cfg(feature = "ffmpeg")]
pub use error::{ThumbstripError, ThumbstripResult};
#[cfg(feature = "ffmpeg")]
pub use generator::{ThumbstripGenerator, ThumbstripInfo};
#[cfg(feature = "ffmpeg")]
pub use job::ThumbstripJob;
#[cfg(feature = "ffmpeg")]
pub use processor::ThumbstripProcessor;
#[cfg(feature = "ffmpeg")]
pub use state::{ThumbstripPhase, ThumbstripState};

#[cfg(feature = "ffmpeg")]
use crate::library::Library;
#[cfg(feature = "ffmpeg")]
use crate::ops::sidecar::types::SidecarKind;
#[cfg(feature = "ffmpeg")]
use std::path::Path;
#[cfg(feature = "ffmpeg")]
use std::sync::Arc;
#[cfg(feature = "ffmpeg")]
use tracing::{info, warn};
#[cfg(feature = "ffmpeg")]
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

	info!(
		"Generating {} thumbstrip variants for {} (regenerate: {})",
		variants.len(),
		video_path.display(),
		regenerate
	);

	for variant_config in variants {
		info!(
			"Processing variant: {} for {}",
			variant_config.variant.as_str(),
			content_uuid
		);

		// Check if already exists
		let exists_result = sidecar_manager
			.exists(
				&library.id(),
				content_uuid,
				&SidecarKind::Thumbstrip,
				&variant_config.variant,
				&variant_config.format(),
			)
			.await;

		match exists_result {
			Ok(true) if !regenerate => {
				info!(
					"Thumbstrip already exists for {}: {}, skipping",
					content_uuid,
					variant_config.variant.as_str()
				);
				continue;
			}
			Ok(false) => {
				info!(
					"Thumbstrip does not exist for {}: {}, will generate",
					content_uuid,
					variant_config.variant.as_str()
				);
			}
			Ok(true) => {
				info!(
					"Thumbstrip exists but regenerate=true for {}: {}, will regenerate",
					content_uuid,
					variant_config.variant.as_str()
				);
			}
			Err(e) => {
				warn!(
					"Failed to check thumbstrip existence for {}: {}, will attempt generation anyway: {}",
					content_uuid,
					variant_config.variant.as_str(),
					e
				);
			}
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
					info!(
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
	_library: &std::sync::Arc<crate::library::Library>,
	_content_uuid: &uuid::Uuid,
	_video_path: &std::path::Path,
	_variants: &[()], // Can't reference ThumbstripVariantConfig without feature
	_regenerate: bool,
) -> Result<usize, String> {
	Err("Thumbstrip generation requires FFmpeg feature to be enabled".to_string())
}
