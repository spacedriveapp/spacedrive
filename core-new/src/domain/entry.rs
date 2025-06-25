//! Entry - the core file/directory representation in Spacedrive
//!
//! An Entry represents any filesystem item (file, directory, symlink) that
//! Spacedrive knows about. It's the foundation of the VDFS.

use crate::shared::types::SdPath;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Represents any filesystem entry (file or directory) in the VDFS
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Entry {
    /// Unique identifier for this entry
    pub id: Uuid,

    /// The virtual path including device context
    pub sd_path: SdPathSerialized,

    /// File/directory name
    pub name: String,

    /// Type of entry
    pub kind: EntryKind,

    /// Size in bytes (None for directories)
    pub size: Option<u64>,

    /// Filesystem timestamps
    pub created_at: Option<DateTime<Utc>>,
    pub modified_at: Option<DateTime<Utc>>,
    pub accessed_at: Option<DateTime<Utc>>,

    /// Platform-specific identifiers
    pub inode: Option<u64>,      // Unix/macOS
    pub file_id: Option<u64>,     // Windows

    /// Parent directory entry ID
    pub parent_id: Option<Uuid>,

    /// Location this entry belongs to (if indexed)
    pub location_id: Option<Uuid>,

    /// User metadata (ALWAYS exists - key innovation!)
    pub metadata_id: Uuid,

    /// Content identity for deduplication (optional)
    pub content_id: Option<Uuid>,

    /// Tracking information
    pub first_seen_at: DateTime<Utc>,
    pub last_indexed_at: Option<DateTime<Utc>>,
}

/// Type of filesystem entry
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum EntryKind {
    /// Regular file
    File {
        /// File extension (without dot)
        extension: Option<String>
    },

    /// Directory
    Directory,

    /// Symbolic link
    Symlink {
        /// Target path
        target: String
    },
}

/// How SdPath is stored in the database
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SdPathSerialized {
    /// Device where this entry exists
    pub device_id: Uuid,

    /// Normalized path on that device
    pub path: String,

    /// Optional library context
    pub library_id: Option<Uuid>,
}

impl SdPathSerialized {
    /// Create from an SdPath
    pub fn from_sdpath(sdpath: &SdPath) -> Self {
        Self {
            device_id: sdpath.device_id,
            path: sdpath.path.to_string_lossy().to_string(),
            library_id: sdpath.library_id,
        }
    }

    /// Convert back to SdPath
    pub fn to_sdpath(&self) -> SdPath {
        SdPath {
            device_id: self.device_id,
            path: self.path.clone().into(),
            library_id: self.library_id,
        }
    }
}

impl Entry {
    /// Create a new Entry from filesystem metadata
    pub fn new(sd_path: SdPath, metadata: std::fs::Metadata) -> Self {
        let name = sd_path
            .path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string();

        let kind = if metadata.is_dir() {
            EntryKind::Directory
        } else if metadata.is_symlink() {
            EntryKind::Symlink {
                target: String::new(), // Would need to read link
            }
        } else {
            let extension = sd_path
                .path
                .extension()
                .and_then(|e| e.to_str())
                .map(|e| e.to_string());
            EntryKind::File { extension }
        };

        let size = if metadata.is_file() {
            Some(metadata.len())
        } else {
            None
        };

        Self {
            id: Uuid::new_v4(),
            sd_path: SdPathSerialized::from_sdpath(&sd_path),
            name,
            kind,
            size,
            created_at: metadata.created().ok().map(|t| t.into()),
            modified_at: metadata.modified().ok().map(|t| t.into()),
            accessed_at: metadata.accessed().ok().map(|t| t.into()),
            inode: None, // Platform-specific, would need conditional compilation
            file_id: None,
            parent_id: None,
            location_id: None,
            metadata_id: Uuid::new_v4(), // Will create UserMetadata with this ID
            content_id: None,
            first_seen_at: Utc::now(),
            last_indexed_at: None,
        }
    }

    /// Check if this is a file
    pub fn is_file(&self) -> bool {
        matches!(self.kind, EntryKind::File { .. })
    }

    /// Check if this is a directory
    pub fn is_directory(&self) -> bool {
        matches!(self.kind, EntryKind::Directory)
    }

    /// Get the file extension if this is a file
    pub fn extension(&self) -> Option<&str> {
        match &self.kind {
            EntryKind::File { extension } => extension.as_deref(),
            _ => None,
        }
    }

    /// Get the SdPath for this entry
    pub fn sd_path(&self) -> SdPath {
        self.sd_path.to_sdpath()
    }
}