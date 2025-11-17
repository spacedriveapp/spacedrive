//! Thumbstrip generation engine
//!
//! Generates grid-based storyboard thumbnails from video files using the sd-ffmpeg crate.

use super::{
	error::{ThumbstripError, ThumbstripResult},
	ThumbstripVariantConfig,
};
use image::{imageops, DynamicImage, RgbImage};
use serde::{Deserialize, Serialize};
use std::{ops::Deref, path::Path};
use tokio::task::spawn_blocking;
use tracing::{debug, warn};
use webp::Encoder;

/// Information about a generated thumbstrip
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThumbstripInfo {
	pub size_bytes: usize,
	pub dimensions: (u32, u32),
	pub columns: u32,
	pub rows: u32,
	pub thumbnail_size: (u32, u32),
	pub total_frames: u32,
	pub format: String,
}

/// Video frame data (from sd-ffmpeg)
pub struct VideoFrame {
	pub data: Vec<u8>,
	pub width: u32,
	pub height: u32,
	pub rotation: f64,
}

/// Thumbstrip generator
pub struct ThumbstripGenerator {
	config: ThumbstripVariantConfig,
}

impl ThumbstripGenerator {
	/// Create a new thumbstrip generator
	pub fn new(config: ThumbstripVariantConfig) -> Self {
		Self { config }
	}

	/// Generate a thumbstrip from a video file
	#[cfg(feature = "ffmpeg")]
	pub async fn generate(
		&self,
		video_path: impl AsRef<Path> + Send,
		output_path: impl AsRef<Path> + Send,
	) -> ThumbstripResult<ThumbstripInfo> {
		use sd_ffmpeg::{FrameDecoder, ThumbnailSize};

		let config = self.config.clone();
		let video_path = video_path.as_ref().to_path_buf();
		let output_path = output_path.as_ref().to_path_buf();

		spawn_blocking(move || {
			// Create frame decoder
			let mut decoder = FrameDecoder::new(&video_path, true, false)
				.map_err(|e| ThumbstripError::FFmpeg(e.to_string()))?;

			// Get video duration
			let duration = decoder
				.get_duration_secs()
				.ok_or(ThumbstripError::other("Video has no duration"))?;

			// Calculate timestamps for frame extraction
			let timestamps = calculate_timestamps(config.total_frames(), duration);

			debug!(
				"Extracting {} frames from {:.1}s video",
				timestamps.len(),
				duration
			);

			// Extract frames at each timestamp
			let mut frames = Vec::with_capacity(timestamps.len());
			let thumbnail_size = ThumbnailSize::Scale(config.thumbnail_size);

			for (idx, timestamp) in timestamps.iter().enumerate() {
				// Seek to timestamp
				if let Err(e) = decoder.seek(*timestamp as i64) {
					warn!(
						"Failed to seek to {}s (frame {}): {:?}, skipping",
						timestamp, idx, e
					);
					continue;
				}

				// Decode frame at this position
				if let Err(e) = decoder.decode_video_frame() {
					warn!("Failed to decode frame {}: {:?}, skipping", idx, e);
					continue;
				}

				// Get scaled frame
				match decoder.get_scaled_video_frame(Some(thumbnail_size), true) {
					Ok(frame) => {
						frames.push(VideoFrame {
							data: frame.data,
							width: frame.width,
							height: frame.height,
							rotation: frame.rotation,
						});
					}
					Err(e) => {
						warn!("Failed to scale frame {}: {:?}, skipping", idx, e);
						continue;
					}
				}
			}

			if frames.is_empty() {
				return Err(ThumbstripError::NoFrames);
			}

			debug!("Successfully extracted {} frames", frames.len());

			// Compose frames into grid
			let thumbstrip_image = compose_grid(&frames, config.columns, config.rows)?;

			// Get dimensions
			let (width, height) = (thumbstrip_image.width(), thumbstrip_image.height());
			let thumb_width = frames[0].width;
			let thumb_height = frames[0].height;

			// Encode to WebP
			let webp_data = encode_to_webp(&thumbstrip_image, config.quality)?;

			// Write to file
			std::fs::write(&output_path, &webp_data).map_err(|e| ThumbstripError::Io(e))?;

			debug!(
				"Generated thumbstrip: {}x{} grid, {} total frames, {} KB",
				config.columns,
				config.rows,
				frames.len(),
				webp_data.len() / 1024
			);

			Ok(ThumbstripInfo {
				size_bytes: webp_data.len(),
				dimensions: (width, height),
				columns: config.columns,
				rows: config.rows,
				thumbnail_size: (thumb_width, thumb_height),
				total_frames: frames.len() as u32,
				format: "webp".to_string(),
			})
		})
		.await
		.map_err(|e| ThumbstripError::other(format!("Task join error: {}", e)))?
	}

	/// Generate without ffmpeg feature (returns error)
	#[cfg(not(feature = "ffmpeg"))]
	pub async fn generate(
		&self,
		_video_path: impl AsRef<Path> + Send,
		_output_path: impl AsRef<Path> + Send,
	) -> ThumbstripResult<ThumbstripInfo> {
		Err(ThumbstripError::other(
			"Thumbstrip generation requires FFmpeg feature to be enabled",
		))
	}
}

/// Calculate evenly-spaced timestamps throughout the video
fn calculate_timestamps(frame_count: u32, duration: f64) -> Vec<f64> {
	// Skip very first and last frame to avoid black frames
	let start_offset = duration * 0.05; // Start 5% into video
	let end_offset = duration * 0.95; // End at 95% of video
	let usable_duration = end_offset - start_offset;

	(0..frame_count)
		.map(|i| {
			if frame_count == 1 {
				start_offset + usable_duration / 2.0
			} else {
				start_offset + (usable_duration * f64::from(i)) / f64::from(frame_count - 1)
			}
		})
		.collect()
}

/// Compose individual frames into a grid layout
fn compose_grid(frames: &[VideoFrame], columns: u32, rows: u32) -> ThumbstripResult<DynamicImage> {
	if frames.is_empty() {
		return Err(ThumbstripError::NoFrames);
	}

	let thumb_width = frames[0].width;
	let thumb_height = frames[0].height;

	let grid_width = thumb_width * columns;
	let grid_height = thumb_height * rows;

	let mut output = RgbImage::new(grid_width, grid_height);

	// Fill with black background (for incomplete grids)
	for pixel in output.pixels_mut() {
		*pixel = image::Rgb([0, 0, 0]);
	}

	for (idx, frame) in frames.iter().enumerate() {
		let row = idx as u32 / columns;
		let col = idx as u32 % columns;

		// Stop if we exceed grid dimensions
		if row >= rows {
			break;
		}

		let x_offset = col * thumb_width;
		let y_offset = row * thumb_height;

		// Create image from frame data
		let frame_image = RgbImage::from_raw(frame.width, frame.height, frame.data.clone())
			.ok_or_else(|| {
				ThumbstripError::ImageProcessing(
					"Failed to create image from frame data".to_string(),
				)
			})?;

		// Copy frame into output grid
		imageops::replace(&mut output, &frame_image, x_offset.into(), y_offset.into());
	}

	Ok(DynamicImage::ImageRgb8(output))
}

/// Encode image to WebP format
fn encode_to_webp(image: &DynamicImage, quality: u8) -> ThumbstripResult<Vec<u8>> {
	if quality > 100 {
		return Err(ThumbstripError::InvalidQuality(quality));
	}

	let quality_f32 = f32::from(quality);

	let encoder = Encoder::from_image(image).map_err(|e| {
		ThumbstripError::ImageProcessing(format!("WebP encoder creation failed: {}", e))
	})?;

	// WebPMemory is !Send, so we deref to &[u8] and clone
	Ok(encoder.encode(quality_f32).deref().to_vec())
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_timestamp_calculation() {
		let timestamps = calculate_timestamps(25, 100.0);

		assert_eq!(timestamps.len(), 25);
		// First timestamp should be around 5% into video
		assert!(timestamps[0] > 4.0 && timestamps[0] < 6.0);
		// Last timestamp should be around 95% into video
		assert!(timestamps[24] > 94.0 && timestamps[24] < 96.0);
	}

	#[test]
	fn test_timestamp_calculation_single_frame() {
		let timestamps = calculate_timestamps(1, 100.0);
		assert_eq!(timestamps.len(), 1);
		// Single frame should be at 50% (middle)
		assert!(timestamps[0] > 49.0 && timestamps[0] < 51.0);
	}
}
