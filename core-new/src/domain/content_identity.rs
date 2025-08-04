//! Content domain types - for content identification and media metadata
//!
//! This module contains domain types used by the content identification system.
//! The actual ContentIdentity persistence is handled by database entities.

use chrono::{DateTime, Utc};
use int_enum::IntEnum;
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;

/// Type of content
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, IntEnum)]
#[serde(rename_all = "snake_case")]
#[repr(i32)]
pub enum ContentKind {
	Unknown = 0,
	Image = 1,
	Video = 2,
	Audio = 3,
	Document = 4,
	Archive = 5,
	Code = 6,
	Text = 7,
	Database = 8,
	Book = 9,
	Font = 10,
	Mesh = 11,
	Config = 12,
	Encrypted = 13,
	Key = 14,
	Executable = 15,
	Binary = 16,
}

impl std::fmt::Display for ContentKind {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		let s = match self {
			ContentKind::Unknown => "unknown",
			ContentKind::Image => "image",
			ContentKind::Video => "video",
			ContentKind::Audio => "audio",
			ContentKind::Document => "document",
			ContentKind::Archive => "archive",
			ContentKind::Code => "code",
			ContentKind::Text => "text",
			ContentKind::Database => "database",
			ContentKind::Book => "book",
			ContentKind::Font => "font",
			ContentKind::Mesh => "mesh",
			ContentKind::Config => "config",
			ContentKind::Encrypted => "encrypted",
			ContentKind::Key => "key",
			ContentKind::Executable => "executable",
			ContentKind::Binary => "binary",
		};
		write!(f, "{}", s)
	}
}

/// Media-specific metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MediaData {
	/// Width in pixels (for images/video)
	pub width: Option<u32>,

	/// Height in pixels (for images/video)
	pub height: Option<u32>,

	/// Duration in seconds (for audio/video)
	pub duration: Option<f64>,

	/// Bitrate in bits per second
	pub bitrate: Option<u32>,

	/// Frame rate (for video)
	pub fps: Option<f32>,

	/// EXIF data (for images)
	pub exif: Option<ExifData>,

	/// Additional metadata as JSON
	pub extra: JsonValue,
}

/// EXIF metadata for images
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExifData {
	/// Camera make
	pub make: Option<String>,

	/// Camera model
	pub model: Option<String>,

	/// Date taken
	pub date_taken: Option<DateTime<Utc>>,

	/// GPS coordinates
	pub gps: Option<GpsCoordinates>,

	/// ISO speed
	pub iso: Option<u32>,

	/// Aperture (f-stop)
	pub aperture: Option<f32>,

	/// Shutter speed in seconds
	pub shutter_speed: Option<f32>,

	/// Focal length in mm
	pub focal_length: Option<f32>,
}

/// GPS coordinates
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GpsCoordinates {
	pub latitude: f64,
	pub longitude: f64,
	pub altitude: Option<f32>,
}

impl ContentKind {
	/// Determine content kind from MIME type
	pub fn from_mime_type(mime_type: &str) -> Self {
		match mime_type.split('/').next() {
			Some("image") => ContentKind::Image,
			Some("video") => ContentKind::Video,
			Some("audio") => ContentKind::Audio,
			Some("text") => ContentKind::Text,
			_ if mime_type.contains("pdf") => ContentKind::Document,
			_ if mime_type.contains("zip") || mime_type.contains("tar") => ContentKind::Archive,
			_ => ContentKind::Unknown,
		}
	}

	/// Get content kind from file type
	pub fn from_file_type(file_type: &crate::file_type::FileType) -> Self {
		file_type.category
	}
}

/// Size threshold for sampling vs full hashing (100KB)
pub const MINIMUM_FILE_SIZE: u64 = 1024 * 100;

/// Sample configuration constants
const SAMPLE_COUNT: u64 = 4;
const SAMPLE_SIZE: u64 = 1024 * 10; // 10KB
const HEADER_OR_FOOTER_SIZE: u64 = 1024 * 8; // 8KB

/// Content hash generator for content identification
pub struct ContentHashGenerator;

impl ContentHashGenerator {
	/// Generate a content hash for a file
	/// Uses sampling for large files, full hash for small files
	pub async fn generate_content_hash(path: &std::path::Path) -> Result<String, ContentHashError> {
		let metadata = tokio::fs::metadata(path).await?;
		let file_size = metadata.len();

		if file_size <= MINIMUM_FILE_SIZE {
			// Small file: hash entire content
			Self::generate_full_hash(path, file_size).await
		} else {
			// Large file: use sampling algorithm
			Self::generate_sampled_hash(path, file_size).await
		}
	}

	/// Generate full BLAKE3 hash for small files
	pub async fn generate_full_hash(path: &std::path::Path, size: u64) -> Result<String, ContentHashError> {
		use blake3::Hasher;

		let mut hasher = Hasher::new();
		hasher.update(&size.to_le_bytes());
		
		let content = tokio::fs::read(path).await?;
		hasher.update(&content);
		
		Ok(hasher.finalize().to_hex()[..16].to_string())
	}

	/// Generate sampled hash for large files using V1 algorithm
	/// Samples header, 4 evenly spaced samples, and footer
	#[allow(clippy::cast_possible_truncation)]
	#[allow(clippy::cast_possible_wrap)]
	async fn generate_sampled_hash(
		path: &std::path::Path,
		size: u64,
	) -> Result<String, ContentHashError> {
		use blake3::Hasher;
		use tokio::io::{AsyncReadExt, AsyncSeekExt};

		let mut hasher = Hasher::new();
		hasher.update(&size.to_le_bytes());

		let mut file = tokio::fs::File::open(path).await?;
		let mut buf = vec![0; SAMPLE_SIZE as usize].into_boxed_slice();

		// Hashing the header
		let mut current_pos = file
			.read_exact(&mut buf[..HEADER_OR_FOOTER_SIZE as usize])
			.await? as u64;
		hasher.update(&buf[..HEADER_OR_FOOTER_SIZE as usize]);

		// Sample hashing the inner content of the file
		let seek_jump = (size - HEADER_OR_FOOTER_SIZE * 2) / SAMPLE_COUNT;
		loop {
			file.read_exact(&mut buf).await?;
			hasher.update(&buf);

			if current_pos >= (HEADER_OR_FOOTER_SIZE + seek_jump * (SAMPLE_COUNT - 1)) {
				break;
			}

			current_pos = file.seek(std::io::SeekFrom::Start(current_pos + seek_jump)).await?;
		}

		// Hashing the footer
		file.seek(std::io::SeekFrom::End(-(HEADER_OR_FOOTER_SIZE as i64)))
			.await?;
		file.read_exact(&mut buf[..HEADER_OR_FOOTER_SIZE as usize])
			.await?;
		hasher.update(&buf[..HEADER_OR_FOOTER_SIZE as usize]);

		Ok(hasher.finalize().to_hex()[..16].to_string())
	}

	/// Generate content hash from raw content (for in-memory data)
	pub fn generate_from_content(content: &[u8]) -> String {
		use blake3::Hasher;

		let mut hasher = Hasher::new();
		hasher.update(&(content.len() as u64).to_le_bytes());
		hasher.update(content);

		hasher.finalize().to_hex()[..16].to_string()
	}

	/// Verify a content hash matches the current content of a file
	pub async fn verify_content_hash(
		path: &std::path::Path,
		expected_hash: &str,
	) -> Result<bool, ContentHashError> {
		let current_hash = Self::generate_content_hash(path).await?;
		Ok(current_hash == expected_hash)
	}
}

/// Errors that can occur during content hash generation
#[derive(Debug, thiserror::Error)]
pub enum ContentHashError {
	#[error("IO error: {0}")]
	Io(#[from] std::io::Error),

	#[error("Invalid file path")]
	InvalidPath,

	#[error("File too large to process")]
	FileTooLarge,
}
