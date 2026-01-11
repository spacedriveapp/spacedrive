//! Thumbnail generation engine using existing Spacedrive crates

use super::error::{ThumbnailError, ThumbnailResult};
use sd_media_metadata::exif::Orientation;
use serde::{Deserialize, Serialize};
use std::path::Path;

/// Information about a generated thumbnail
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThumbnailInfo {
	pub size_bytes: usize,
	pub dimensions: (u32, u32),
	pub format: String,
	pub blurhash: Option<String>,
}

/// Multi-format thumbnail generator
#[derive(Debug)]
pub enum ThumbnailGenerator {
	Image(ImageGenerator),
	Video(VideoGenerator),
	Document(DocumentGenerator),
}

impl ThumbnailGenerator {
	/// Create appropriate generator for a MIME type
	pub fn for_mime_type(mime_type: &str) -> ThumbnailResult<Self> {
		match mime_type {
			mime if mime.starts_with("image/") => Ok(Self::Image(ImageGenerator::new())),
			mime if mime.starts_with("video/") => {
				#[cfg(feature = "ffmpeg")]
				{
					Ok(Self::Video(VideoGenerator::new()))
				}
				#[cfg(not(feature = "ffmpeg"))]
				{
					Err(ThumbnailError::other(
						"Video thumbnail generation requires FFmpeg feature to be enabled",
					))
				}
			}
			"application/pdf" => Ok(Self::Document(DocumentGenerator::new())),
			_ => Err(ThumbnailError::unsupported_format(mime_type)),
		}
	}

	/// Generate thumbnail
	pub async fn generate(
		&self,
		source_path: &Path,
		output_path: &Path,
		size: u32,
		quality: u8,
	) -> ThumbnailResult<ThumbnailInfo> {
		match self {
			Self::Image(gen) => gen.generate(source_path, output_path, size, quality).await,
			Self::Video(gen) => gen.generate(source_path, output_path, size, quality).await,
			Self::Document(gen) => gen.generate(source_path, output_path, size, quality).await,
		}
	}
}

/// Image thumbnail generator using sd-images crate
#[derive(Debug)]
pub struct ImageGenerator;

impl ImageGenerator {
	pub fn new() -> Self {
		Self
	}

	pub async fn generate(
		&self,
		source_path: &Path,
		output_path: &Path,
		size: u32,
		quality: u8,
	) -> ThumbnailResult<ThumbnailInfo> {
		if quality > 100 {
			return Err(ThumbnailError::InvalidQuality(quality));
		}

		// Ensure output directory exists
		if let Some(parent) = output_path.parent() {
			tokio::fs::create_dir_all(parent).await?;
		}

		// Use tokio::task::spawn_blocking for CPU-intensive image processing
		let source_path = source_path.to_path_buf();
		let output_path = output_path.to_path_buf();

		let thumbnail_info = tokio::task::spawn_blocking(move || {
			// Use sd-images to load and process the image
			let mut img = sd_images::format_image(&source_path)
				.map_err(|e| ThumbnailError::other(format!("Failed to load image: {}", e)))?;

			// Apply EXIF orientation correction if available
			if let Some(orientation) = Orientation::from_path(&source_path) {
				img = orientation.correct_thumbnail(img);
			}

			// Blurhash generation disabled for performance
			let blurhash: Option<String> = None;

			// Calculate target dimensions maintaining aspect ratio
			let (original_width, original_height) = (img.width(), img.height());
			let (target_width, target_height) =
				calculate_dimensions(original_width, original_height, size);

			// Resize using high-quality algorithm
			let thumbnail = img.resize(
				target_width,
				target_height,
				image::imageops::FilterType::Lanczos3,
			);

			// Convert to RGB8 for consistency
			let rgb_thumbnail = thumbnail.to_rgb8();

			// Get actual dimensions from the resized image (may differ from calculated due to rounding)
			let actual_width = rgb_thumbnail.width();
			let actual_height = rgb_thumbnail.height();

			// Verify buffer size matches expected dimensions
			let expected_size = (actual_width * actual_height * 3) as usize;
			let actual_size = rgb_thumbnail.as_raw().len();

			if expected_size != actual_size {
				return Err(ThumbnailError::other(format!(
					"Image buffer size mismatch: expected {} bytes for {}x{}, got {} bytes",
					expected_size, actual_width, actual_height, actual_size
				)));
			}

			// Encode as WebP using actual dimensions
			let webp_encoder = webp::Encoder::from_rgb(&rgb_thumbnail, actual_width, actual_height);
			let webp_memory = webp_encoder.encode(quality as f32);
			let webp_data = webp_memory.to_vec();

			// Write to file
			std::fs::write(&output_path, &webp_data)?;

			Ok::<ThumbnailInfo, ThumbnailError>(ThumbnailInfo {
				size_bytes: webp_data.len(),
				dimensions: (actual_width, actual_height),
				format: "webp".to_string(),
				blurhash,
			})
		})
		.await
		.map_err(|e| ThumbnailError::other(format!("Task join error: {}", e)))??;

		Ok(thumbnail_info)
	}
}

/// Video thumbnail generator using sd-ffmpeg crate
#[derive(Debug)]
pub struct VideoGenerator;

impl VideoGenerator {
	pub fn new() -> Self {
		Self
	}

	pub async fn generate(
		&self,
		source_path: &Path,
		output_path: &Path,
		size: u32,
		quality: u8,
	) -> ThumbnailResult<ThumbnailInfo> {
		#[cfg(feature = "ffmpeg")]
		{
			if quality > 100 {
				return Err(ThumbnailError::InvalidQuality(quality));
			}

			// Blurhash generation disabled for performance
			let blurhash: Option<String> = None;

			// Use sd-ffmpeg helper function to generate thumbnail
			sd_ffmpeg::to_thumbnail(
				source_path,
				output_path,
				sd_ffmpeg::ThumbnailSize::Scale(size),
				quality as f32,
			)
			.await
			.map_err(|e| {
				ThumbnailError::video_processing(format!("FFmpeg processing failed: {}", e))
			})?;

			// Get file size and return info
			let file_size = tokio::fs::metadata(output_path).await?.len() as usize;

			// Calculate approximate dimensions (actual dimensions would require parsing FFmpeg output)
			let dimensions = calculate_video_dimensions(size);

			Ok(ThumbnailInfo {
				size_bytes: file_size,
				dimensions,
				format: "webp".to_string(),
				blurhash,
			})
		}

		#[cfg(not(feature = "ffmpeg"))]
		{
			let _ = (source_path, output_path, size, quality); // Suppress unused variable warnings
			Err(ThumbnailError::other(
				"Video thumbnail generation requires FFmpeg feature to be enabled",
			))
		}
	}
}

/// Document thumbnail generator using sd-images crate (PDF support)
#[derive(Debug)]
pub struct DocumentGenerator;

impl DocumentGenerator {
	pub fn new() -> Self {
		Self
	}

	pub async fn generate(
		&self,
		source_path: &Path,
		output_path: &Path,
		size: u32,
		quality: u8,
	) -> ThumbnailResult<ThumbnailInfo> {
		if quality > 100 {
			return Err(ThumbnailError::InvalidQuality(quality));
		}

		// Ensure output directory exists
		if let Some(parent) = output_path.parent() {
			tokio::fs::create_dir_all(parent).await?;
		}

		// Use tokio::task::spawn_blocking for CPU-intensive PDF processing
		let source_path = source_path.to_path_buf();
		let output_path = output_path.to_path_buf();

		let thumbnail_info = tokio::task::spawn_blocking(move || {
			// Use sd-images to handle PDF (it supports PDF through pdfium-render)
			let mut img = sd_images::format_image(&source_path)
				.map_err(|e| ThumbnailError::other(format!("Failed to load PDF: {}", e)))?;

			// Apply EXIF orientation correction if available
			if let Some(orientation) = Orientation::from_path(&source_path) {
				img = orientation.correct_thumbnail(img);
			}

			// Blurhash generation disabled for performance
			let blurhash: Option<String> = None;

			// Calculate target dimensions maintaining aspect ratio
			let (original_width, original_height) = (img.width(), img.height());
			let (target_width, target_height) =
				calculate_dimensions(original_width, original_height, size);

			// Resize using high-quality algorithm
			let thumbnail = img.resize(
				target_width,
				target_height,
				image::imageops::FilterType::Lanczos3,
			);

			// Convert to RGB8 for WebP encoding
			let rgb_thumbnail = thumbnail.to_rgb8();

			// Get actual dimensions from the resized image
			let actual_width = rgb_thumbnail.width();
			let actual_height = rgb_thumbnail.height();

			// Encode as WebP using actual dimensions
			let webp_encoder = webp::Encoder::from_rgb(&rgb_thumbnail, actual_width, actual_height);
			let webp_memory = webp_encoder.encode(quality as f32);
			let webp_data = webp_memory.to_vec();

			// Write to file
			std::fs::write(&output_path, &webp_data)?;

			Ok::<ThumbnailInfo, ThumbnailError>(ThumbnailInfo {
				size_bytes: webp_data.len(),
				dimensions: (actual_width, actual_height),
				format: "webp".to_string(),
				blurhash,
			})
		})
		.await
		.map_err(|e| ThumbnailError::other(format!("Task join error: {}", e)))??;

		Ok(thumbnail_info)
	}
}

/// Calculate target dimensions maintaining aspect ratio
fn calculate_dimensions(width: u32, height: u32, target_size: u32) -> (u32, u32) {
	let aspect_ratio = width as f32 / height as f32;

	if width > height {
		// Landscape
		let target_width = target_size;
		let target_height = (target_size as f32 / aspect_ratio) as u32;
		(target_width, target_height.max(1))
	} else {
		// Portrait or square
		let target_height = target_size;
		let target_width = (target_size as f32 * aspect_ratio) as u32;
		(target_width.max(1), target_height)
	}
}

/// Calculate approximate video thumbnail dimensions
/// In practice, this would need to be extracted from the actual video metadata
fn calculate_video_dimensions(target_size: u32) -> (u32, u32) {
	// Assume 16:9 aspect ratio for now (most common)
	// This is a simplified approach - in practice we'd get actual dimensions from FFmpeg
	let aspect_ratio = 16.0 / 9.0;

	let target_width = target_size;
	let target_height = (target_size as f32 / aspect_ratio) as u32;

	(target_width, target_height.max(1))
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_calculate_dimensions() {
		// Landscape image
		let (w, h) = calculate_dimensions(1920, 1080, 256);
		assert_eq!(w, 256);
		assert_eq!(h, 144);

		// Portrait image
		let (w, h) = calculate_dimensions(1080, 1920, 256);
		assert_eq!(w, 144);
		assert_eq!(h, 256);

		// Square image
		let (w, h) = calculate_dimensions(1000, 1000, 256);
		assert_eq!(w, 256);
		assert_eq!(h, 256);
	}

	#[test]
	fn test_generator_for_mime_type() {
		assert!(matches!(
			ThumbnailGenerator::for_mime_type("image/jpeg"),
			Ok(ThumbnailGenerator::Image(_))
		));

		#[cfg(feature = "ffmpeg")]
		{
			assert!(matches!(
				ThumbnailGenerator::for_mime_type("video/mp4"),
				Ok(ThumbnailGenerator::Video(_))
			));
		}

		#[cfg(not(feature = "ffmpeg"))]
		{
			assert!(ThumbnailGenerator::for_mime_type("video/mp4").is_err());
		}

		assert!(matches!(
			ThumbnailGenerator::for_mime_type("application/pdf"),
			Ok(ThumbnailGenerator::Document(_))
		));

		assert!(ThumbnailGenerator::for_mime_type("text/plain").is_err());
	}
}
