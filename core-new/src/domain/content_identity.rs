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

/// Current CAS algorithm version
pub const CURRENT_CAS_VERSION: u8 = 2;

/// Size threshold for sampling vs full hashing (10MB)
pub const SMALL_FILE_THRESHOLD: u64 = 10 * 1024 * 1024;

/// Content hash generator for content identification
pub struct ContentHashGenerator;

impl ContentHashGenerator {
	/// Generate a content hash for a file
	/// Uses sampling for large files, full hash for small files
	pub async fn generate_content_hash(path: &std::path::Path) -> Result<String, ContentHashError> {
		let metadata = tokio::fs::metadata(path).await?;
		let file_size = metadata.len();

		if file_size <= SMALL_FILE_THRESHOLD {
			// Small file: hash entire content
			Self::generate_full_hash(path).await
		} else {
			// Large file: use sampling algorithm
			Self::generate_sampled_hash(path, file_size).await
		}
	}

	/// Generate full SHA-256 hash for small files
	pub async fn generate_full_hash(path: &std::path::Path) -> Result<String, ContentHashError> {
		use sha2::{Digest, Sha256};

		let content = tokio::fs::read(path).await?;
		let mut hasher = Sha256::new();
		hasher.update(&content);
		let hash = hasher.finalize();

		Ok(format!("v{}_full:{:x}", CURRENT_CAS_VERSION, hash))
	}

	/// Generate sampled hash for large files
	/// Samples from beginning, middle, and end of file
	async fn generate_sampled_hash(
		path: &std::path::Path,
		file_size: u64,
	) -> Result<String, ContentHashError> {
		use sha2::{Digest, Sha256};
		use tokio::io::{AsyncReadExt, AsyncSeekExt};

		const SAMPLE_SIZE: u64 = 8192; // 8KB samples
		const NUM_SAMPLES: u64 = 3;

		let mut file = tokio::fs::File::open(path).await?;
		let mut hasher = Sha256::new();

		// Include file size in hash to distinguish files of different sizes
		hasher.update(&file_size.to_le_bytes());

		// Sample from beginning
		let mut buffer = vec![0u8; SAMPLE_SIZE as usize];
		file.seek(std::io::SeekFrom::Start(0)).await?;
		let bytes_read = file.read(&mut buffer).await?;
		hasher.update(&buffer[..bytes_read]);

		// Sample from middle (if file is large enough)
		if file_size > SAMPLE_SIZE * 2 {
			let middle_pos = file_size / 2 - SAMPLE_SIZE / 2;
			file.seek(std::io::SeekFrom::Start(middle_pos)).await?;
			let bytes_read = file.read(&mut buffer).await?;
			hasher.update(&buffer[..bytes_read]);
		}

		// Sample from end (if file is large enough)
		if file_size > SAMPLE_SIZE * NUM_SAMPLES {
			let end_pos = file_size.saturating_sub(SAMPLE_SIZE);
			file.seek(std::io::SeekFrom::Start(end_pos)).await?;
			let bytes_read = file.read(&mut buffer).await?;
			hasher.update(&buffer[..bytes_read]);
		}

		let hash = hasher.finalize();
		Ok(format!("v{}_sampled:{:x}", CURRENT_CAS_VERSION, hash))
	}

	/// Generate content hash from raw content (for in-memory data)
	pub fn generate_from_content(content: &[u8]) -> String {
		use sha2::{Digest, Sha256};

		let mut hasher = Sha256::new();
		hasher.update(content);
		let hash = hasher.finalize();

		format!("v{}_content:{:x}", CURRENT_CAS_VERSION, hash)
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
