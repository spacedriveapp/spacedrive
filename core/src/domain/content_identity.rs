//! Content domain types - for content identification and media metadata
//!
//! This module contains domain types used by the content identification system.
//! The actual ContentIdentity persistence is handled by database entities.

use chrono::{DateTime, Utc};
use int_enum::IntEnum;
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use specta::Type;
use uuid::Uuid;

use crate::infra::db::entities::*;
use crate::volume::VolumeBackend;

/// Domain representation of content identity
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct ContentIdentity {
	pub uuid: Uuid,
	pub kind: ContentKind,
	pub content_hash: String,
	pub integrity_hash: Option<String>,
	pub mime_type_id: Option<i32>,
	pub text_content: Option<String>,
	pub total_size: i64,
	pub entry_count: i32,
	pub first_seen_at: DateTime<Utc>,
	pub last_verified_at: DateTime<Utc>,
}

/// Type of content
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, IntEnum, Type)]
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
	Spreadsheet = 17,
	Presentation = 18,
	Email = 19,
	Calendar = 20,
	Contact = 21,
	Web = 22,
	Shortcut = 23,
	Package = 24,
	ModelEntry = 25,
}

// Translate database entity into domain model
impl From<content_identity::Model> for ContentIdentity {
	fn from(model: content_identity::Model) -> Self {
		Self {
			uuid: model.uuid.unwrap_or_else(Uuid::new_v4),
			kind: ContentKind::try_from(model.kind_id).unwrap_or(ContentKind::Unknown),
			content_hash: model.content_hash,
			integrity_hash: model.integrity_hash,
			mime_type_id: model.mime_type_id,
			text_content: model.text_content,
			total_size: model.total_size,
			entry_count: model.entry_count,
			first_seen_at: model.first_seen_at,
			last_verified_at: model.last_verified_at,
		}
	}
}

impl ContentKind {
	/// Get content kind from file type
	pub fn from_file_type(file_type: &crate::filetype::FileType) -> Self {
		file_type.category
	}

	/// Convert from database ID to ContentKind
	pub fn from_id(id: i32) -> Self {
		match id {
			0 => Self::Unknown,
			1 => Self::Image,
			2 => Self::Video,
			3 => Self::Audio,
			4 => Self::Document,
			5 => Self::Archive,
			6 => Self::Code,
			7 => Self::Text,
			8 => Self::Database,
			9 => Self::Book,
			10 => Self::Font,
			11 => Self::Mesh,
			12 => Self::Config,
			13 => Self::Encrypted,
			14 => Self::Key,
			15 => Self::Executable,
			16 => Self::Binary,
			17 => Self::Spreadsheet,
			18 => Self::Presentation,
			19 => Self::Email,
			20 => Self::Calendar,
			21 => Self::Contact,
			22 => Self::Web,
			23 => Self::Shortcut,
			24 => Self::Package,
			_ => Self::Unknown,
		}
	}
}

/// Size threshold for sampling vs full hashing (100KB)
pub const MINIMUM_FILE_SIZE: u64 = 1024 * 100;

/// Sample configuration constants (public for cloud backend usage)
pub const SAMPLE_COUNT: u64 = 4;
pub const SAMPLE_SIZE: u64 = 1024 * 10; // 10KB
pub const HEADER_OR_FOOTER_SIZE: u64 = 1024 * 8; // 8KB

/// Content hash generator for content identification
pub struct ContentHashGenerator;

impl ContentHashGenerator {
	/// Generate a content hash for a local file (uses LocalBackend internally)
	///
	/// For local files, this wraps the path in a LocalBackend and calls the
	/// backend-based implementation. This ensures consistent hashing across
	/// local and cloud storage.
	pub async fn generate_content_hash(path: &std::path::Path) -> Result<String, ContentHashError> {
		// Create a LocalBackend for this path
		let backend = crate::volume::LocalBackend::new(path.parent().unwrap_or(path));

		// Get file size using backend
		let metadata = backend
			.metadata(path)
			.await
			.map_err(|e| ContentHashError::Io(std::io::Error::new(std::io::ErrorKind::Other, e)))?;

		Self::generate_content_hash_with_backend(&backend, path, metadata.size).await
	}

	/// Generate content hash from raw content (for in-memory data)
	pub fn generate_from_content(content: &[u8]) -> String {
		use blake3::Hasher;

		let mut hasher = Hasher::new();
		hasher.update(&(content.len() as u64).to_le_bytes());
		hasher.update(content);

		hasher.finalize().to_hex()[..16].to_string()
	}

	/// Generate content hash using a volume backend
	///
	/// This uses the same sampling algorithm but works with any VolumeBackend,
	/// enabling efficient content hashing for cloud files without full downloads.
	pub async fn generate_content_hash_with_backend(
		backend: &dyn crate::volume::VolumeBackend,
		path: &std::path::Path,
		size: u64,
	) -> Result<String, ContentHashError> {
		// Reject empty files - they should not have content identities
		// All empty files would have the same hash, causing them to be treated as identical
		if size == 0 {
			return Err(ContentHashError::EmptyFile);
		}

		if size <= MINIMUM_FILE_SIZE {
			// Small file: read entire content
			Self::generate_full_hash_with_backend(backend, path, size).await
		} else {
			// Large file: use sampling with ranged reads
			Self::generate_sampled_hash_with_backend(backend, path, size).await
		}
	}

	/// Generate full hash using backend (for small files)
	async fn generate_full_hash_with_backend(
		backend: &dyn crate::volume::VolumeBackend,
		path: &std::path::Path,
		size: u64,
	) -> Result<String, ContentHashError> {
		use blake3::Hasher;

		let mut hasher = Hasher::new();
		hasher.update(&size.to_le_bytes());

		let content = backend
			.read(path)
			.await
			.map_err(|e| ContentHashError::Io(std::io::Error::new(std::io::ErrorKind::Other, e)))?;
		hasher.update(&content);

		Ok(hasher.finalize().to_hex()[..16].to_string())
	}

	/// Generate sampled hash using backend ranged reads (efficient for cloud)
	///
	/// This implements the same sampling algorithm as `generate_sampled_hash`
	/// but uses ranged reads, transferring only ~58KB for large files.
	async fn generate_sampled_hash_with_backend(
		backend: &dyn crate::volume::VolumeBackend,
		path: &std::path::Path,
		size: u64,
	) -> Result<String, ContentHashError> {
		use blake3::Hasher;

		let mut hasher = Hasher::new();
		hasher.update(&size.to_le_bytes());

		// Header (8KB)
		let header = backend
			.read_range(path, 0..HEADER_OR_FOOTER_SIZE)
			.await
			.map_err(|e| ContentHashError::Io(std::io::Error::new(std::io::ErrorKind::Other, e)))?;
		hasher.update(&header);

		// 4 samples (10KB each) evenly spaced
		let seek_jump = (size - HEADER_OR_FOOTER_SIZE * 2) / SAMPLE_COUNT;
		let mut current_pos = HEADER_OR_FOOTER_SIZE;

		for _ in 0..SAMPLE_COUNT {
			let sample = backend
				.read_range(path, current_pos..current_pos + SAMPLE_SIZE)
				.await
				.map_err(|e| {
					ContentHashError::Io(std::io::Error::new(std::io::ErrorKind::Other, e))
				})?;
			hasher.update(&sample);
			current_pos += seek_jump;
		}

		// Footer (8KB)
		let footer_start = size - HEADER_OR_FOOTER_SIZE;
		let footer = backend
			.read_range(path, footer_start..size)
			.await
			.map_err(|e| ContentHashError::Io(std::io::Error::new(std::io::ErrorKind::Other, e)))?;
		hasher.update(&footer);

		Ok(hasher.finalize().to_hex()[..16].to_string())
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

	#[error("Empty files cannot have content identity")]
	EmptyFile,
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
			ContentKind::Spreadsheet => "spreadsheet",
			ContentKind::Presentation => "presentation",
			ContentKind::Email => "email",
			ContentKind::Calendar => "calendar",
			ContentKind::Contact => "contact",
			ContentKind::Web => "web",
			ContentKind::Shortcut => "shortcut",
			ContentKind::Package => "package",
			ContentKind::ModelEntry => "model_entry",
		};
		write!(f, "{}", s)
	}
}

impl From<&str> for ContentKind {
	fn from(name: &str) -> Self {
		match name {
			"image" => ContentKind::Image,
			"video" => ContentKind::Video,
			"audio" => ContentKind::Audio,
			"document" => ContentKind::Document,
			"archive" => ContentKind::Archive,
			"code" => ContentKind::Code,
			"text" => ContentKind::Text,
			"database" => ContentKind::Database,
			"book" => ContentKind::Book,
			"font" => ContentKind::Font,
			"mesh" => ContentKind::Mesh,
			"config" => ContentKind::Config,
			"encrypted" => ContentKind::Encrypted,
			"key" => ContentKind::Key,
			"executable" => ContentKind::Executable,
			"binary" => ContentKind::Binary,
			"spreadsheet" => ContentKind::Spreadsheet,
			"presentation" => ContentKind::Presentation,
			"email" => ContentKind::Email,
			"calendar" => ContentKind::Calendar,
			"contact" => ContentKind::Contact,
			"web" => ContentKind::Web,
			"shortcut" => ContentKind::Shortcut,
			"package" => ContentKind::Package,
			"model_entry" => ContentKind::ModelEntry,
			_ => ContentKind::Unknown,
		}
	}
}

impl From<String> for ContentKind {
	fn from(name: String) -> Self {
		Self::from(name.as_str())
	}
}
