//! Content identity - for deduplication and content-based features
//! 
//! ContentIdentity is OPTIONAL - files can exist without it.
//! It's only created when content is indexed for deduplication.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use uuid::Uuid;
use int_enum::IntEnum;

/// Represents unique content for deduplication
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContentIdentity {
    /// Unique identifier
    pub id: Uuid,
    
    /// Full file hash (if computed)
    pub full_hash: Option<String>,
    
    /// Content-addressed storage ID (sampled hash)
    pub cas_id: String,
    
    /// Version of the CAS algorithm used
    pub cas_version: u8,
    
    /// MIME type of the content
    pub mime_type: Option<String>,
    
    /// Kind of content
    pub kind: ContentKind,
    
    /// Extracted media metadata (EXIF, video info, etc.)
    pub media_data: Option<MediaData>,
    
    /// Extracted text content for search
    pub text_content: Option<String>,
    
    /// Combined size of all entries with this content
    pub total_size: u64,
    
    /// Number of entries that share this content
    pub entry_count: u32,
    
    /// When this content was first seen
    pub first_seen_at: DateTime<Utc>,
    
    /// When this content was last verified
    pub last_verified_at: DateTime<Utc>,
}

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

impl ContentIdentity {
    /// Create a new ContentIdentity
    pub fn new(cas_id: String, cas_version: u8) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4(),
            full_hash: None,
            cas_id,
            cas_version,
            mime_type: None,
            kind: ContentKind::Unknown,
            media_data: None,
            text_content: None,
            total_size: 0,
            entry_count: 1,
            first_seen_at: now,
            last_verified_at: now,
        }
    }
    
    /// Increment the entry count
    pub fn increment_entry_count(&mut self) {
        self.entry_count += 1;
    }
    
    /// Decrement the entry count
    pub fn decrement_entry_count(&mut self) {
        if self.entry_count > 0 {
            self.entry_count -= 1;
        }
    }
    
    /// Update total size
    pub fn add_size(&mut self, size: u64) {
        self.total_size += size;
    }
    
    /// Remove size
    pub fn remove_size(&mut self, size: u64) {
        self.total_size = self.total_size.saturating_sub(size);
    }
    
    /// Set MIME type and content kind from file type
    pub fn set_from_file_type(&mut self, file_type: &crate::file_type::FileType) {
        // ContentKind is used directly in FileType now
        self.kind = file_type.category.clone();
        
        // Use the primary MIME type if available
        if let Some(mime) = file_type.primary_mime_type() {
            self.mime_type = Some(mime.to_string());
        }
    }
    
    /// Determine content kind from MIME type (fallback method)
    pub fn set_mime_type(&mut self, mime_type: String) {
        self.kind = match mime_type.split('/').next() {
            Some("image") => ContentKind::Image,
            Some("video") => ContentKind::Video,
            Some("audio") => ContentKind::Audio,
            Some("text") => ContentKind::Text,
            _ if mime_type.contains("pdf") => ContentKind::Document,
            _ if mime_type.contains("zip") || mime_type.contains("tar") => ContentKind::Archive,
            _ => ContentKind::Unknown,
        };
        self.mime_type = Some(mime_type);
    }
    
    /// Check if this content is orphaned (no entries reference it)
    pub fn is_orphaned(&self) -> bool {
        self.entry_count == 0
    }
}

/// Current CAS algorithm version
pub const CURRENT_CAS_VERSION: u8 = 2;

/// Size threshold for sampling vs full hashing (10MB)
pub const SMALL_FILE_THRESHOLD: u64 = 10 * 1024 * 1024;

/// Content-Addressable Storage ID generator
pub struct CasGenerator;

impl CasGenerator {
    /// Generate a CAS ID for a file
    /// Uses sampling for large files, full hash for small files
    pub async fn generate_cas_id(path: &std::path::Path) -> Result<String, CasError> {
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
    async fn generate_full_hash(path: &std::path::Path) -> Result<String, CasError> {
        use sha2::{Sha256, Digest};
        
        let content = tokio::fs::read(path).await?;
        let mut hasher = Sha256::new();
        hasher.update(&content);
        let hash = hasher.finalize();
        
        Ok(format!("v{}_full:{:x}", CURRENT_CAS_VERSION, hash))
    }
    
    /// Generate sampled hash for large files
    /// Samples from beginning, middle, and end of file
    async fn generate_sampled_hash(path: &std::path::Path, file_size: u64) -> Result<String, CasError> {
        use sha2::{Sha256, Digest};
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
    
    /// Generate CAS ID from raw content (for in-memory data)
    pub fn generate_from_content(content: &[u8]) -> String {
        use sha2::{Sha256, Digest};
        
        let mut hasher = Sha256::new();
        hasher.update(content);
        let hash = hasher.finalize();
        
        format!("v{}_content:{:x}", CURRENT_CAS_VERSION, hash)
    }
    
    /// Verify a CAS ID matches the current content of a file
    pub async fn verify_cas_id(path: &std::path::Path, expected_cas_id: &str) -> Result<bool, CasError> {
        let current_cas_id = Self::generate_cas_id(path).await?;
        Ok(current_cas_id == expected_cas_id)
    }
}

/// Errors that can occur during CAS ID generation
#[derive(Debug, thiserror::Error)]
pub enum CasError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    
    #[error("Invalid file path")]
    InvalidPath,
    
    #[error("File too large to process")]
    FileTooLarge,
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_content_identity_creation() {
        let content = ContentIdentity::new("v2:abc123".to_string(), 2);
        assert_eq!(content.cas_id, "v2:abc123");
        assert_eq!(content.cas_version, 2);
        assert_eq!(content.entry_count, 1);
        assert_eq!(content.kind, ContentKind::Unknown);
    }
    
    #[test]
    fn test_mime_type_detection() {
        let mut content = ContentIdentity::new("v2:abc123".to_string(), 2);
        
        content.set_mime_type("image/jpeg".to_string());
        assert_eq!(content.kind, ContentKind::Image);
        
        content.set_mime_type("video/mp4".to_string());
        assert_eq!(content.kind, ContentKind::Video);
        
        content.set_mime_type("application/pdf".to_string());
        assert_eq!(content.kind, ContentKind::Document);
    }
}