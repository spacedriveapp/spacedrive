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
			mime if mime.starts_with("video/") => {
				#[cfg(feature = "ffmpeg")]
				{
					true
				}
				#[cfg(not(feature = "ffmpeg"))]
				{
					false
				}
			}
			"application/pdf" => true,
			_ => false,
		}
	}

	/// Get the thumbnail file extension for a given format
	pub fn get_thumbnail_extension(_format: &str) -> &'static str {
		"webp" // All thumbnails are WebP format
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

	/// Generate shard path for a CAS ID
	pub fn get_shard_path(cas_id: &str) -> ThumbnailResult<(String, String)> {
		if cas_id.len() < 4 {
			return Err(ThumbnailError::other("CAS ID too short for sharding"));
		}

		let shard1 = cas_id[0..2].to_string();
		let shard2 = cas_id[2..4].to_string();

		Ok((shard1, shard2))
	}

	/// Build thumbnail filename
	pub fn build_thumbnail_filename(cas_id: &str, size: u32) -> String {
		format!("{}_{}.webp", cas_id, size)
	}

	/// Build full thumbnail path with sharding
	pub fn build_thumbnail_path(
		thumbnails_dir: &Path,
		cas_id: &str,
		size: u32,
	) -> ThumbnailResult<std::path::PathBuf> {
		let (shard1, shard2) = Self::get_shard_path(cas_id)?;
		let filename = Self::build_thumbnail_filename(cas_id, size);

		Ok(thumbnails_dir.join(shard1).join(shard2).join(filename))
	}

	/// Check if a thumbnail file exists
	pub async fn thumbnail_exists(path: &Path) -> bool {
		tokio::fs::metadata(path).await.is_ok()
	}

	/// Create thumbnail directory structure
	pub async fn ensure_thumbnail_dirs(thumbnail_path: &Path) -> ThumbnailResult<()> {
		if let Some(parent) = thumbnail_path.parent() {
			tokio::fs::create_dir_all(parent).await?;
		}
		Ok(())
	}

	/// Calculate total file size for a list of files
	pub async fn calculate_total_size(paths: &[std::path::PathBuf]) -> u64 {
		let mut total_size = 0;
		for path in paths {
			if let Ok(metadata) = tokio::fs::metadata(path).await {
				total_size += metadata.len();
			}
		}
		total_size
	}

	/// Clean up orphaned thumbnails (not implemented yet)
	pub async fn cleanup_orphaned_thumbnails(
		_thumbnails_dir: &Path,
		_valid_cas_ids: &[String],
	) -> ThumbnailResult<u64> {
		// TODO: Implement cleanup logic
		// 1. Scan thumbnail directory
		// 2. Extract CAS IDs from filenames
		// 3. Check against valid_cas_ids
		// 4. Remove orphaned files
		// 5. Return number of cleaned files
		Ok(0)
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_is_thumbnail_supported() {
		assert!(ThumbnailUtils::is_thumbnail_supported("image/jpeg"));
		assert!(ThumbnailUtils::is_thumbnail_supported("image/png"));
		assert!(ThumbnailUtils::is_thumbnail_supported("video/mp4"));
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

	#[test]
	fn test_get_shard_path() {
		let (shard1, shard2) = ThumbnailUtils::get_shard_path("abcdef123456").unwrap();
		assert_eq!(shard1, "ab");
		assert_eq!(shard2, "cd");

		assert!(ThumbnailUtils::get_shard_path("abc").is_err());
	}

	#[test]
	fn test_build_thumbnail_filename() {
		let filename = ThumbnailUtils::build_thumbnail_filename("abcdef123456", 256);
		assert_eq!(filename, "abcdef123456_256.webp");
	}

	#[test]
	fn test_build_thumbnail_path() {
		let base = std::path::Path::new("/thumbnails");
		let path = ThumbnailUtils::build_thumbnail_path(base, "abcdef123456", 256).unwrap();
		assert_eq!(
			path,
			std::path::Path::new("/thumbnails/ab/cd/abcdef123456_256.webp")
		);
	}
}
