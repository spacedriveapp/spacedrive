//! Content identity - for deduplication and content-based features
//! 
//! ContentIdentity is OPTIONAL - files can exist without it.
//! It's only created when content is indexed for deduplication.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use uuid::Uuid;

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
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ContentKind {
    Image,
    Video,
    Audio,
    Document,
    Archive,
    Code,
    Text,
    Database,
    Book,
    Font,
    Mesh,
    Config,
    Encrypted,
    Key,
    Executable,
    Binary,
    Unknown,
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