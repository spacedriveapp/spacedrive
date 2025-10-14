//! Entry - the core file/directory representation in Spacedrive
//!
//! An Entry represents any filesystem item (file, directory, symlink) that
//! Spacedrive knows about. It's the foundation of the VDFS.

use crate::domain::addressing::SdPath;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use specta::Type;
use uuid::Uuid;

/// Represents any filesystem entry (file or directory) in the VDFS
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
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
	pub inode: Option<u64>, // Unix/macOS
	pub file_id: Option<u64>, // Windows

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
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Type)]
pub enum EntryKind {
	/// Regular file
	File {
		/// File extension (without dot)
		extension: Option<String>,
	},

	/// Directory
	Directory,

	/// Symbolic link
	Symlink {
		/// Target path
		target: String,
	},
}

/// How SdPath is stored in the database
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct SdPathSerialized {
	/// Device where this entry exists
	pub device_id: Uuid,

	/// Normalized path on that device
	pub path: String,
}

impl SdPathSerialized {
	/// Create from an SdPath
	pub fn from_sdpath(sdpath: &SdPath) -> Option<Self> {
		match sdpath {
			SdPath::Physical { device_id, path } => Some(Self {
				device_id: *device_id,
				path: path.to_string_lossy().to_string(),
			}),
			SdPath::Cloud { volume_id, path } => Some(Self {
				device_id: *volume_id, // Use volume_id as device_id for cloud paths
				path: path.clone(),
			}),
			SdPath::Content { .. } => None, // Can't serialize content paths to this format
		}
	}

	/// Convert back to SdPath
	pub fn to_sdpath(&self) -> SdPath {
		SdPath::Physical {
			device_id: self.device_id,
			path: self.path.clone().into(),
		}
	}
}

impl Entry {
	/// Create a new Entry from filesystem metadata
	pub fn new(sd_path: SdPath, metadata: std::fs::Metadata) -> Self {
		let name = sd_path.file_name().unwrap_or("unknown").to_string();

		let kind = if metadata.is_dir() {
			EntryKind::Directory
		} else if metadata.is_symlink() {
			EntryKind::Symlink {
				target: String::new(), // Would need to read link
			}
		} else {
			let extension = sd_path
				.path()
				.and_then(|p| p.extension())
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
			sd_path: SdPathSerialized::from_sdpath(&sd_path)
				.expect("Entry requires a physical path"),
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

/// Conversion from database model to domain Entry
impl TryFrom<(crate::infra::db::entities::entry::Model, SdPath)> for Entry {
	type Error = anyhow::Error;

	fn try_from(
		(entry_model, parent_sd_path): (crate::infra::db::entities::entry::Model, SdPath),
	) -> Result<Self, Self::Error> {
		let device_uuid = match &parent_sd_path {
			SdPath::Physical { device_id, .. } => *device_id,
			SdPath::Cloud { volume_id, .. } => *volume_id,
			SdPath::Content { .. } => {
				return Err(anyhow::anyhow!(
					"Content-addressed paths not supported for directory listing"
				))
			}
		};

		// Construct the full path properly to avoid double slashes
		// TODO: validation should happen on SdPath imo
		let full_path = if entry_model.parent_id.is_none() {
			format!("/{}", entry_model.name)
		} else {
			let parent_path = parent_sd_path.display().to_string();
			if parent_path.ends_with('/') {
				format!("{}{}", parent_path, entry_model.name)
			} else {
				format!("{}/{}", parent_path, entry_model.name)
			}
		};

		Ok(Entry {
			id: entry_model.uuid.unwrap_or_else(|| Uuid::new_v4()),
			sd_path: SdPathSerialized {
				device_id: device_uuid,
				path: full_path,
			},
			name: entry_model.name,
			kind: match entry_model.kind {
				0 => EntryKind::File {
					extension: entry_model.extension,
				},
				1 => EntryKind::Directory,
				2 => EntryKind::Symlink {
					target: "".to_string(), // TODO: Get from database
				},
				_ => EntryKind::File {
					extension: entry_model.extension,
				},
			},
			size: Some(entry_model.size as u64),
			created_at: Some(entry_model.created_at),
			modified_at: Some(entry_model.modified_at),
			accessed_at: entry_model.accessed_at,
			inode: entry_model.inode.map(|i| i as u64),
			file_id: None,
			parent_id: entry_model.parent_id.map(|_| Uuid::new_v4()), // TODO: Proper UUID conversion
			location_id: None,
			metadata_id: entry_model
				.metadata_id
				.map(|_| Uuid::new_v4())
				.unwrap_or_else(Uuid::new_v4),
			content_id: entry_model.content_id.map(|_| Uuid::new_v4()), // TODO: Proper UUID conversion
			first_seen_at: entry_model.created_at,
			last_indexed_at: Some(entry_model.created_at),
		})
	}
}
