//! File domain model - the forward-facing file representation
//!
//! This module provides the File domain type that aggregates data from Entry,
//! ContentIdentity, Tags, and Sidecars into a developer-friendly interface.
//! The File struct is computed from pre-fetched data rather than fetching
//! individual pieces on demand.

use crate::domain::{
	addressing::SdPath,
	content_identity::{ContentIdentity, ContentKind},
	entry::Entry,
	tag::Tag,
};
use crate::ops::sidecar::types::{SidecarFormat, SidecarKind, SidecarStatus, SidecarVariant};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use specta::Type;
use uuid::Uuid;

/// Represents a file within the Spacedrive VDFS.
///
/// This is a computed domain model that aggregates data from Entry, ContentIdentity,
/// Tags, and Sidecars. It provides a rich, developer-friendly interface without
/// duplicating data in the database.
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct File {
	/// The unique identifier of the file entry
	pub id: Uuid,

	/// The universal path to the file in Spacedrive's VDFS
	pub sd_path: SdPath,

	/// The name of the file, including the extension
	pub name: String,

	/// The size of the file in bytes
	pub size: u64,

	/// Information about the file's content, including its content hash
	pub content_identity: Option<ContentIdentity>,

	/// A list of other paths that share the same content identity
	pub alternate_paths: Vec<SdPath>,

	/// The semantic tags associated with this file
	pub tags: Vec<Tag>,

	/// A list of sidecars associated with this file
	pub sidecars: Vec<Sidecar>,

	/// Timestamps for creation, modification, and access
	pub created_at: DateTime<Utc>,
	pub modified_at: DateTime<Utc>,
	pub accessed_at: Option<DateTime<Utc>>,

	/// Additional computed fields
	pub content_kind: ContentKind,
	pub extension: Option<String>,
	// pub is_directory: bool, TODO: add this
	pub is_local: bool,
}

/// Domain representation of a sidecar
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct Sidecar {
	pub id: i32,
	pub content_uuid: Uuid,
	pub kind: String,
	pub variant: String,
	pub format: String,
	pub status: String,
	pub size: i64,
	pub created_at: DateTime<Utc>,
	pub updated_at: DateTime<Utc>,
}

/// Data required to construct a File from pre-fetched database data
#[derive(Debug, Clone)]
pub struct FileConstructionData {
	pub entry: Entry,
	pub content_identity: Option<ContentIdentity>,
	pub tags: Vec<Tag>,
	pub sidecars: Vec<Sidecar>,
	pub alternate_paths: Vec<SdPath>,
}

impl File {
	/// Construct a File from pre-fetched data
	///
	/// This method assumes all required data has already been fetched via
	/// database joins. It does not perform any database operations.
	pub fn from_data(data: FileConstructionData) -> Self {
		let FileConstructionData {
			entry,
			content_identity,
			tags,
			sidecars,
			alternate_paths,
		} = data;

		let sd_path = entry.sd_path();
		let is_local = sd_path.is_local();
		let extension = entry.extension().map(|s| s.to_string());
		let content_kind = content_identity
			.as_ref()
			.map(|ci| ci.kind)
			.unwrap_or(ContentKind::Unknown);

		Self {
			id: entry.id,
			sd_path,
			name: entry.name,
			size: entry.size.unwrap_or(0),
			content_identity,
			alternate_paths,
			tags,
			sidecars,
			created_at: entry.created_at.unwrap_or_else(Utc::now),
			modified_at: entry.modified_at.unwrap_or_else(Utc::now),
			accessed_at: entry.accessed_at,
			content_kind,
			extension,
			is_local,
		}
	}

	/// Construct a File from just an Entry (minimal data)
	///
	/// Useful when you only have basic entry data and want a File representation
	/// without additional metadata.
	pub fn from_entry(entry: Entry) -> Self {
		let sd_path = entry.sd_path();
		let is_local = sd_path.is_local();
		let extension = entry.extension().map(|s| s.to_string());
		let content_kind = ContentKind::Unknown; // Will be determined later when content is processed

		Self {
			id: entry.id,
			sd_path,
			name: entry.name,
			size: entry.size.unwrap_or(0),
			content_identity: None,
			alternate_paths: Vec::new(),
			tags: Vec::new(),
			sidecars: Vec::new(),
			created_at: entry.created_at.unwrap_or_else(Utc::now),
			modified_at: entry.modified_at.unwrap_or_else(Utc::now),
			accessed_at: entry.accessed_at,
			content_kind,
			extension,
			is_local,
		}
	}

	/// Check if this file has content identity information
	pub fn has_content_identity(&self) -> bool {
		self.content_identity.is_some()
	}

	/// Check if this file has any sidecars
	pub fn has_sidecars(&self) -> bool {
		!self.sidecars.is_empty()
	}

	/// Check if this file has any tags
	pub fn has_tags(&self) -> bool {
		!self.tags.is_empty()
	}

	/// Get sidecars of a specific kind
	pub fn sidecars_by_kind(&self, kind: &str) -> Vec<&Sidecar> {
		self.sidecars
			.iter()
			.filter(|sidecar| sidecar.kind == kind)
			.collect()
	}

	/// Get sidecars that are ready (not pending or failed)
	pub fn ready_sidecars(&self) -> Vec<&Sidecar> {
		self.sidecars
			.iter()
			.filter(|sidecar| sidecar.status == "Ready")
			.collect()
	}

	/// Check if this file has alternate paths (duplicates)
	pub fn has_duplicates(&self) -> bool {
		!self.alternate_paths.is_empty()
	}

	/// Get the total number of copies of this file across all devices
	pub fn total_copies(&self) -> usize {
		self.alternate_paths.len() + 1 // +1 for the original path
	}

	/// Get a display-friendly path string
	pub fn display_path(&self) -> String {
		self.sd_path.display()
	}

	/// Check if this is a media file
	pub fn is_media(&self) -> bool {
		matches!(
			self.content_kind,
			ContentKind::Image | ContentKind::Video | ContentKind::Audio
		)
	}

	/// Check if this is a document
	pub fn is_document(&self) -> bool {
		matches!(
			self.content_kind,
			ContentKind::Document | ContentKind::Text | ContentKind::Book
		)
	}

	/// Check if this is an archive
	pub fn is_archive(&self) -> bool {
		self.content_kind == ContentKind::Archive
	}
}

impl Sidecar {
	/// Create a new Sidecar from database entity data
	pub fn from_entity(
		id: i32,
		content_uuid: Uuid,
		kind: SidecarKind,
		variant: SidecarVariant,
		format: SidecarFormat,
		status: SidecarStatus,
		size: i64,
		created_at: DateTime<Utc>,
		updated_at: DateTime<Utc>,
	) -> Self {
		Self {
			id,
			content_uuid,
			kind: kind.to_string(),
			variant: variant.to_string(),
			format: format.to_string(),
			status: status.to_string(),
			size,
			created_at,
			updated_at,
		}
	}

	/// Check if this sidecar is ready for use
	pub fn is_ready(&self) -> bool {
		self.status == "Ready"
	}

	/// Check if this sidecar failed to generate
	pub fn is_failed(&self) -> bool {
		self.status == "Failed"
	}

	/// Check if this sidecar is still being generated
	pub fn is_pending(&self) -> bool {
		self.status == "Pending"
	}

	/// Get the file extension for this sidecar
	pub fn file_extension(&self) -> &str {
		// TODO: Implement proper extension mapping
		match self.format.as_str() {
			"Webp" => "webp",
			"Jpeg" => "jpg",
			"Png" => "png",
			_ => "bin",
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::domain::addressing::SdPath;

	#[test]
	fn test_file_from_entry() {
		let device_id = Uuid::new_v4();
		let entry = Entry {
			id: Uuid::new_v4(),
			sd_path: crate::domain::entry::SdPathSerialized {
				device_id,
				path: "/test/file.txt".to_string(),
			},
			name: "file.txt".to_string(),
			kind: crate::domain::entry::EntryKind::File {
				extension: Some("txt".to_string()),
			},
			size: Some(1024),
			created_at: Some(Utc::now()),
			modified_at: Some(Utc::now()),
			accessed_at: None,
			inode: None,
			file_id: None,
			parent_id: None,
			location_id: None,
			metadata_id: Uuid::new_v4(),
			content_id: None,
			first_seen_at: Utc::now(),
			last_indexed_at: None,
		};

		let file = File::from_entry(entry);

		assert_eq!(file.name, "file.txt");
		assert_eq!(file.size, 1024);
		assert_eq!(file.extension, Some("txt".to_string()));
		assert!(!file.has_content_identity());
		assert!(!file.has_sidecars());
		assert!(!file.has_tags());
		assert_eq!(file.content_kind, ContentKind::Unknown);
	}

	#[test]
	fn test_file_with_data() {
		let device_id = Uuid::new_v4();
		let entry = Entry {
			id: Uuid::new_v4(),
			sd_path: crate::domain::entry::SdPathSerialized {
				device_id,
				path: "/test/image.jpg".to_string(),
			},
			name: "image.jpg".to_string(),
			kind: crate::domain::entry::EntryKind::File {
				extension: Some("jpg".to_string()),
			},
			size: Some(2048),
			created_at: Some(Utc::now()),
			modified_at: Some(Utc::now()),
			accessed_at: None,
			inode: None,
			file_id: None,
			parent_id: None,
			location_id: None,
			metadata_id: Uuid::new_v4(),
			content_id: Some(Uuid::new_v4()),
			first_seen_at: Utc::now(),
			last_indexed_at: None,
		};

		let content_identity = ContentIdentity {
			uuid: Uuid::new_v4(),
			kind: ContentKind::Image,
			hash: "abc123".to_string(),
			created_at: Utc::now(),
		};

		let sidecar = Sidecar::from_entity(
			1,
			content_identity.uuid,
			SidecarKind::Thumb,
			SidecarVariant::new("grid@2x"),
			SidecarFormat::Webp,
			SidecarStatus::Ready,
			1024,
			Utc::now(),
			Utc::now(),
		);

		let data = FileConstructionData {
			entry,
			content_identity: Some(content_identity.clone()),
			tags: Vec::new(),
			sidecars: vec![sidecar.clone()],
			alternate_paths: Vec::new(),
		};

		let file = File::from_data(data);

		assert_eq!(file.name, "image.jpg");
		assert_eq!(file.size, 2048);
		assert!(file.has_content_identity());
		assert!(file.has_sidecars());
		assert_eq!(file.content_kind, ContentKind::Image);
		assert!(file.is_media());
		assert!(!file.is_document());
		assert!(!file.is_archive());

		let thumbs = file.sidecars_by_kind(SidecarKind::Thumb.as_str());
		assert_eq!(thumbs.len(), 1);
		assert!(thumbs[0].is_ready());
	}
}
