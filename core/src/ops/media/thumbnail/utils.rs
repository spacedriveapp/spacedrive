//! Thumbnail utility functions

use super::error::{ThumbnailError, ThumbnailResult};
use std::path::Path;

/// Utility functions for thumbnail operations
pub struct ThumbnailUtils;

impl ThumbnailUtils {
	/// Check if a file type supports thumbnail generation
	pub fn is_thumbnail_supported(mime_type: &str) -> bool {
		match mime_type {
			mime if mime.starts_with("image/") => true,
			// Video posters come from QuickLook on macOS and from FFmpeg
			// elsewhere, so macOS supports video without the feature.
			mime if mime.starts_with("video/") => {
				cfg!(any(target_os = "macos", feature = "ffmpeg"))
			}
			"application/pdf" => true,
			_ => false,
		}
	}

	/// Validate thumbnail generation parameters
	pub fn validate_thumbnail_params(size: u32, quality: u8) -> ThumbnailResult<()> {
		if size == 0 || size > 4096 {
			return Err(ThumbnailError::InvalidSize(size));
		}

		if quality > 100 {
			return Err(ThumbnailError::InvalidQuality(quality));
		}

		Ok(())
	}

	/// Create thumbnail directory structure
	pub async fn ensure_thumbnail_dirs(thumbnail_path: &Path) -> ThumbnailResult<()> {
		if let Some(parent) = thumbnail_path.parent() {
			tokio::fs::create_dir_all(parent).await?;
		}
		Ok(())
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_is_thumbnail_supported() {
		assert!(ThumbnailUtils::is_thumbnail_supported("image/jpeg"));
		assert!(ThumbnailUtils::is_thumbnail_supported("image/png"));

		#[cfg(any(target_os = "macos", feature = "ffmpeg"))]
		{
			assert!(ThumbnailUtils::is_thumbnail_supported("video/mp4"));
		}

		#[cfg(not(any(target_os = "macos", feature = "ffmpeg")))]
		{
			assert!(!ThumbnailUtils::is_thumbnail_supported("video/mp4"));
		}

		assert!(ThumbnailUtils::is_thumbnail_supported("application/pdf"));
		assert!(!ThumbnailUtils::is_thumbnail_supported("text/plain"));
		assert!(!ThumbnailUtils::is_thumbnail_supported("application/json"));
	}

	#[test]
	fn test_validate_thumbnail_params() {
		assert!(ThumbnailUtils::validate_thumbnail_params(256, 85).is_ok());
		assert!(ThumbnailUtils::validate_thumbnail_params(0, 85).is_err());
		assert!(ThumbnailUtils::validate_thumbnail_params(5000, 85).is_err());
		assert!(ThumbnailUtils::validate_thumbnail_params(256, 101).is_err());
	}
}
