//! Proxy generation system
//!
//! Generates lower-resolution proxy videos for smooth playback, timeline scrubbing,
//! and editing workflows. Operates as both a processor (for responder) and a job
//! (for batch operations).

pub mod action;
mod config;
mod error;
mod generator;
mod hardware;
pub mod job;
pub mod processor;
mod state;

pub use action::GenerateProxyAction;
pub use config::{ProxyJobConfig, ProxyResolution, ProxyVariantConfig, ProxyVariants};
pub use error::{ProxyError, ProxyResult};
pub use generator::{ProxyGenerator, ProxyInfo};
pub use hardware::{detect_hardware_accel, HardwareAccel};
pub use job::ProxyJob;
pub use processor::ProxyProcessor;
pub use state::{ProxyPhase, ProxyState};

use crate::library::Library;
use crate::ops::sidecar::types::SidecarKind;
use std::path::Path;
use std::sync::Arc;
use tracing::{debug, warn};
use uuid::Uuid;

/// Generate proxy for a single video file
///
/// This function generates proxy variants for a single file and registers
/// them as sidecars. Used by both ProxyProcessor (responder) and
/// ProxyJob (batch operations).
///
/// # Arguments
/// * `library` - The library context
/// * `content_uuid` - The UUID of the content identity
/// * `video_path` - The filesystem path to the video file
/// * `variants` - The proxy variants to generate
/// * `use_hardware_accel` - Whether to use hardware acceleration if available
/// * `preset` - FFmpeg preset (ultrafast, veryfast, fast, medium, slow)
/// * `regenerate` - Whether to regenerate existing proxies
///
/// # Returns
/// Number of proxies successfully generated
pub async fn generate_proxy_for_file(
	library: &Arc<Library>,
	content_uuid: &Uuid,
	video_path: &Path,
	variants: &[ProxyVariantConfig],
	use_hardware_accel: bool,
	preset: &str,
	regenerate: bool,
) -> ProxyResult<usize> {
	// Get sidecar manager
	let sidecar_manager = library
		.core_context()
		.get_sidecar_manager()
		.await
		.ok_or_else(|| ProxyError::other("SidecarManager not available"))?;

	let mut generated_count = 0;

	for variant_config in variants {
		// Check if already exists
		if !regenerate
			&& sidecar_manager
				.exists(
					&library.id(),
					content_uuid,
					&SidecarKind::Proxy,
					&variant_config.variant,
					&variant_config.format(),
				)
				.await
				.unwrap_or(false)
		{
			debug!(
				"Proxy already exists for {}: {}",
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
				&SidecarKind::Proxy,
				&variant_config.variant,
				&variant_config.format(),
			)
			.await
			.map_err(|e| ProxyError::other(format!("Failed to compute path: {}", e)))?;

		// Ensure parent directory exists
		if let Some(parent) = output_path.absolute_path.parent() {
			tokio::fs::create_dir_all(parent).await?;
		}

		// Generate proxy using generator
		let generator = ProxyGenerator::new(
			variant_config.clone(),
			preset.to_string(),
			use_hardware_accel,
		);

		match generator
			.generate(video_path, &output_path.absolute_path)
			.await
		{
			Ok(proxy_info) => {
				// Record the sidecar
				if let Err(e) = sidecar_manager
					.record_sidecar(
						library,
						content_uuid,
						&SidecarKind::Proxy,
						&variant_config.variant,
						&variant_config.format(),
						proxy_info.size_bytes,
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
						"✓ Generated proxy {}: {} MB in {}s ({:.1}× realtime)",
						variant_config.variant.as_str(),
						proxy_info.size_bytes / (1024 * 1024),
						proxy_info.encoding_time_secs,
						proxy_info.average_speed_multiplier
					);
					generated_count += 1;
				}
			}
			Err(e) => {
				warn!(
					"Failed to generate proxy {} for {}: {}",
					variant_config.variant.as_str(),
					video_path.display(),
					e
				);
			}
		}
	}

	Ok(generated_count)
}
