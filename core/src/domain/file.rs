//! File domain model - the forward-facing file representation
//!
//! This module provides the File domain type that aggregates data from Entry,
//! ContentIdentity, Tags, and Sidecars into a developer-friendly interface.
//! The File struct is computed from pre-fetched data rather than fetching
//! individual pieces on demand.

use crate::domain::{
	addressing::SdPath,
	content_identity::{ContentIdentity, ContentKind},
	tag::Tag,
};
use crate::ops::sidecar::types::{SidecarFormat, SidecarKind, SidecarStatus, SidecarVariant};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use specta::Type;
use uuid::Uuid;

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
	pub kind: EntryKind,
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

impl File {
	/// Construct a File directly from entity model and SdPath
	///
	/// This is the preferred method for converting database entities to File objects,
	/// bypassing the Entry domain model entirely.
	pub fn from_entity_model(
		model: crate::infra::db::entities::entry::Model,
		sd_path: SdPath,
	) -> Self {
		let is_local = sd_path.is_local();

		// Convert entity kind to domain EntryKind
		let kind = match model.kind {
			0 => EntryKind::File {
				extension: model.extension.clone(),
			},
			1 => EntryKind::Directory,
			2 => EntryKind::Symlink {
				target: String::new(),
			},
			_ => EntryKind::File {
				extension: model.extension.clone(),
			},
		};

		let extension = match &kind {
			EntryKind::File { extension } => extension.clone(),
			_ => None,
		};

		// Generate UUID from id if uuid is None
		let id = model.uuid.unwrap_or_else(|| {
			Uuid::parse_str(&format!(
				"{:08x}-0000-0000-0000-{:012x}",
				model.id, model.id
			))
			.unwrap_or_else(|_| Uuid::new_v4())
		});

		Self {
			id,
			sd_path,
			name: model.name,
			size: model.size as u64,
			content_identity: None,
			alternate_paths: Vec::new(),
			tags: Vec::new(),
			sidecars: Vec::new(),
			created_at: model.created_at,
			modified_at: model.modified_at,
			accessed_at: model.accessed_at,
			content_kind: ContentKind::Unknown,
			extension,
			kind,
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

