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
}

/// Multi-format thumbnail generator
#[derive(Debug)]
pub enum ThumbnailGenerator {
	#[cfg(target_os = "macos")]
	System(SystemGenerator),
	Image(ImageGenerator),
	Video(VideoGenerator),
	Document(DocumentGenerator),
}

impl ThumbnailGenerator {
	/// Create appropriate generator for a MIME type
	pub fn for_mime_type(mime_type: &str) -> ThumbnailResult<Self> {
		// macOS serves every supported category through the system codecs, so the
		// per-format dispatch only applies where bundled libraries are needed.
		#[cfg(target_os = "macos")]
		{
			if mime_type.starts_with("image/")
				|| mime_type.starts_with("video/")
				|| mime_type == "application/pdf"
			{
				Ok(Self::System(SystemGenerator::new()))
			} else {
				Err(ThumbnailError::unsupported_format(mime_type))
			}
		}

		#[cfg(not(target_os = "macos"))]
		{
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
			#[cfg(target_os = "macos")]
			Self::System(gen) => gen.generate(source_path, output_path, size, quality).await,
			Self::Image(gen) => gen.generate(source_path, output_path, size, quality).await,
			Self::Video(gen) => gen.generate(source_path, output_path, size, quality).await,
			Self::Document(gen) => gen.generate(source_path, output_path, size, quality).await,
		}
	}
}

/// Thumbnail generator backed by the macOS system codecs.
///
/// ImageIO covers every raster format the platform knows, including HEIC and
/// camera RAW, and QuickLook covers PDF pages, video posters and any other
/// document Finder can preview. One path therefore replaces the bundled FFmpeg,
/// libheif and pdfium libraries, which is what lets the default macOS build ship
/// without a native-deps bundle.
#[cfg(target_os = "macos")]
#[derive(Debug)]
pub struct SystemGenerator;

#[cfg(target_os = "macos")]
impl SystemGenerator {
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

		let source_path = source_path.to_path_buf();
		let output_path = output_path.to_path_buf();

		let thumbnail_info = tokio::task::spawn_blocking(move || {
			// ImageIO applies EXIF orientation itself and bounds the longest side
			// to `size`, so no orientation correction or resize is needed here.
			let jpeg = sd_imageio::file_thumbnail_jpeg(
				&source_path,
				i64::from(size),
				f64::from(quality) / 100.0,
			)
			.ok_or_else(|| {
				ThumbnailError::other(format!(
					"No system preview available for {}",
					source_path.display()
				))
			})?;

			// The re-encode keeps this path's output identical in format to the
			// other generators. It decodes a thumbnail rather than the original,
			// so the cost is negligible next to the system decode above.
			let decoded = image::load_from_memory_with_format(&jpeg, image::ImageFormat::Jpeg)
				.map_err(|e| {
					ThumbnailError::other(format!("Failed to decode system preview: {}", e))
				})?;

			let rgb_thumbnail = decoded.to_rgb8();
			let actual_width = rgb_thumbnail.width();
			let actual_height = rgb_thumbnail.height();

			let webp_encoder = webp::Encoder::from_rgb(&rgb_thumbnail, actual_width, actual_height);
			let webp_memory = webp_encoder.encode(quality as f32);
			let webp_data = webp_memory.to_vec();

			std::fs::write(&output_path, &webp_data)?;

			Ok::<ThumbnailInfo, ThumbnailError>(ThumbnailInfo {
				size_bytes: webp_data.len(),
				dimensions: (actual_width, actual_height),
				format: "webp".to_string(),
			})
		})
		.await
		.map_err(|e| ThumbnailError::other(format!("Task join error: {}", e)))??;

		Ok(thumbnail_info)
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

			// The written WebP carries the frame's real aspect ratio; reading
			// its header is the authority on the output dimensions.
			let output_path = output_path.to_path_buf();
			let dimensions = tokio::task::spawn_blocking(move || {
				image::image_dimensions(&output_path)
			})
			.await
			.map_err(|e| ThumbnailError::other(format!("Task join error: {}", e)))?
			.map_err(|e| {
				ThumbnailError::other(format!("Failed to read thumbnail dimensions: {}", e))
			})?;

			Ok(ThumbnailInfo {
				size_bytes: file_size,
				dimensions,
				format: "webp".to_string(),
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

	/// On macOS every supported category resolves to the system codecs, including
	/// video and PDF, which is the point of that path: no bundled library is
	/// needed for any of them.
	#[cfg(target_os = "macos")]
	#[test]
	fn test_generator_for_mime_type() {
		for mime in ["image/jpeg", "image/heic", "video/mp4", "application/pdf"] {
			assert!(matches!(
				ThumbnailGenerator::for_mime_type(mime),
				Ok(ThumbnailGenerator::System(_))
			));
		}

		assert!(ThumbnailGenerator::for_mime_type("text/plain").is_err());
	}

	#[cfg(not(target_os = "macos"))]
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
