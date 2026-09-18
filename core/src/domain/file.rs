//! File domain model - the forward-facing file representation
//!
//! This module provides the File domain type that aggregates data from Entry,
//! ContentIdentity, Tags, and Sidecars into a developer-friendly interface.
//! The File struct is computed from pre-fetched data rather than fetching
//! individual pieces on demand.

use crate::domain::sidecar::{SidecarFormat, SidecarKind, SidecarStatus, SidecarVariant};
use crate::domain::{
	addressing::SdPath,
	content_identity::{ContentIdentity, ContentKind},
	media_data::{AudioMediaData, ImageMediaData, VideoMediaData},
	tag::Tag,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use specta::Type;
use uuid::Uuid;

/// Type of filesystem entry
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Type)]
pub enum EntryKind {
	/// Regular file
	File,

	/// Directory
	Directory,

	/// Symbolic link
	Symlink,
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

	/// The file kind (file, directory, symlink)
	pub kind: EntryKind,

	/// The name of the file, including the extension
	pub name: String,

	/// The file extension (without dot)
	pub extension: Option<String>,

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

	/// Media-specific metadata (extracted from EXIF/FFmpeg)
	pub image_media_data: Option<ImageMediaData>,
	pub video_media_data: Option<VideoMediaData>,
	pub audio_media_data: Option<AudioMediaData>,

	/// Timestamps for creation, modification, and access
	pub created_at: DateTime<Utc>,
	pub modified_at: DateTime<Utc>,
	pub accessed_at: Option<DateTime<Utc>>,

	/// Additional computed fields
	pub content_kind: ContentKind, // Populated by the indexer, for when a File does not have a ContentIdentity
	pub is_local: bool, // this is redundant with SdPath

	/// Video duration (for grid display optimization)
	pub duration_seconds: Option<f64>,

	/// A locally resolvable preview image for files that have no sidecar
	/// thumbnails yet — e.g. a photo library's own derivative, or the file
	/// itself when it is a small local image. UI-facing only.
	#[serde(default)]
	pub thumbnail_path: Option<std::path::PathBuf>,
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
	/// Bumped on every regeneration; sidecar URLs carry it as the cache
	/// buster, since the served files are otherwise immutable to browsers.
	pub version: i32,
	pub created_at: DateTime<Utc>,
	pub updated_at: DateTime<Utc>,
}

impl crate::domain::resource::Identifiable for File {
	fn id(&self) -> Uuid {
		self.id
	}

	fn resource_type() -> &'static str {
		"file"
	}

	fn sync_dependencies() -> &'static [&'static str] {
		// File payloads are announced directly from the volume index and the
		// stores; nothing routes them out of library rows anymore.
		&[]
	}

	fn alternate_ids(&self) -> Vec<Uuid> {
		// Files can be matched by content UUID
		if let Some(content) = &self.content_identity {
			vec![content.uuid]
		} else {
			vec![]
		}
	}

	fn no_merge_fields() -> &'static [&'static str] {
		&["sd_path"]
	}
}

impl File {
	/// The file at this path, as the volume index has it.
	///
	/// The one place a `File` comes from now: resolve the partition, read the
	/// entry, take the content kind. Everything that used to build one from an
	/// entry row goes through here.
	pub async fn at_path(
		cache: &crate::ops::indexing::VolumeIndex,
		path: &std::path::Path,
		uuid: Uuid,
	) -> Option<Self> {
		let owned = path.to_path_buf();
		let index = cache.resolve_index(path);
		let mut index = index.write().await;
		let metadata = index.get_entry_ref(&owned)?;
		let content_kind = index.get_content_kind(&owned);
		drop(index);

		let mut file = Self::from_arena(uuid, &metadata, SdPath::local(path.to_path_buf()));
		file.content_kind = content_kind;
		Some(file)
	}

	/// The file a record uuid names, wherever it lives.
	pub async fn for_record(cache: &crate::ops::indexing::VolumeIndex, uuid: Uuid) -> Option<Self> {
		let path = cache.path_of_record(uuid).await?;
		Self::at_path(cache, &path, uuid).await
	}

	/// Tell clients these files changed.
	///
	/// Content-scoped changes reach every copy of the bytes, so this takes a
	/// list rather than one uuid and the caller does not have to know how many
	/// copies there were.
	pub async fn announce(
		context: &std::sync::Arc<crate::context::CoreContext>,
		records: Vec<Uuid>,
	) {
		use crate::infra::event::{Event, ResourceMetadata};

		let cache = context.volume_index();
		let mut files = Vec::with_capacity(records.len());
		for record in records {
			if let Some(file) = Self::for_record(cache, record).await {
				files.push(file);
			}
		}
		// Clients merge this payload into what they render, so it has to
		// carry the tag state the change was usually about.
		crate::ops::tags::decorate::decorate_files(cache, &mut files).await;

		for file in files {
			let Ok(resource) = serde_json::to_value(&file) else {
				continue;
			};

			context.events.emit(Event::ResourceChanged {
				resource_type: "file".to_string(),
				resource,
				metadata: Some(ResourceMetadata {
					no_merge_fields:
						<Self as crate::domain::resource::Identifiable>::no_merge_fields()
							.iter()
							.map(|s| s.to_string())
							.collect(),
					alternate_ids: vec![],
					affected_paths: vec![file.sd_path.clone()],
				}),
			});
		}
	}

	/// Build a `File` from a store row: the cold twin of
	/// [`Self::from_arena`], with the same name/extension split and kind
	/// mapping, timestamps from the store's millisecond fields, and the
	/// content kind hashing recorded. A directory reports its own row's size,
	/// since a store keeps no subtree rollups.
	pub fn from_store_entry(entry: &sd_store::FsEntry, sd_path: SdPath) -> Self {
		let is_local = sd_path.is_local();

		let (name, extension) = if entry.kind == sd_store::FileKind::File {
			let name = std::path::Path::new(&entry.name)
				.file_stem()
				.and_then(|s| s.to_str())
				.unwrap_or(&entry.name)
				.to_string();
			(name, entry.extension.as_ref().map(|e| e.to_lowercase()))
		} else {
			(entry.name.clone(), None)
		};

		let kind = match entry.kind {
			sd_store::FileKind::File => EntryKind::File,
			sd_store::FileKind::Directory => EntryKind::Directory,
			sd_store::FileKind::Symlink => EntryKind::Symlink,
		};

		let from_ms = |ms: Option<i64>| ms.and_then(DateTime::from_timestamp_millis);
		let content_kind = entry
			.content_kind
			.and_then(|kind| i32::try_from(kind).ok())
			.and_then(|kind| ContentKind::try_from(kind).ok())
			.unwrap_or(ContentKind::Unknown);

		Self {
			id: entry.uuid,
			sd_path,
			name,
			size: entry.size.unwrap_or(0).max(0) as u64,
			content_identity: None,
			alternate_paths: Vec::new(),
			tags: Vec::new(),
			sidecars: Vec::new(),
			image_media_data: None,
			video_media_data: None,
			audio_media_data: None,
			created_at: from_ms(entry.created_ms).unwrap_or_else(Utc::now),
			modified_at: from_ms(entry.mtime_ms).unwrap_or_else(Utc::now),
			accessed_at: from_ms(entry.atime_ms),
			content_kind,
			extension,
			kind,
			is_local,
			duration_seconds: None,
			thumbnail_path: None,
		}
	}

	pub fn from_arena(
		id: Uuid,
		metadata: &crate::ops::indexing::metadata::EntryMetadata,
		sd_path: SdPath,
	) -> Self {
		let is_local = sd_path.is_local();

		// Extract name and extension from path
		let file_name = metadata
			.path
			.file_name()
			.and_then(|n| n.to_str())
			.unwrap_or("unknown");

		let (name, extension) = if metadata.kind == crate::ops::indexing::state::EntryKind::File {
			let extension = metadata
				.path
				.extension()
				.and_then(|e| e.to_str())
				.map(|s| s.to_lowercase());

			let name = metadata
				.path
				.file_stem()
				.and_then(|s| s.to_str())
				.unwrap_or(file_name)
				.to_string();

			(name, extension)
		} else {
			(file_name.to_string(), None)
		};

		// Convert indexing EntryKind to domain EntryKind
		let kind = match metadata.kind {
			crate::ops::indexing::state::EntryKind::File => EntryKind::File,
			crate::ops::indexing::state::EntryKind::Directory => EntryKind::Directory,
			crate::ops::indexing::state::EntryKind::Symlink => EntryKind::Symlink,
		};

		// Convert SystemTime to chrono::DateTime
		let created_at = metadata
			.created
			.and_then(|t| {
				chrono::DateTime::from_timestamp(
					t.duration_since(std::time::UNIX_EPOCH).ok()?.as_secs() as i64,
					0,
				)
			})
			.unwrap_or_else(chrono::Utc::now);

		let modified_at = metadata
			.modified
			.and_then(|t| {
				chrono::DateTime::from_timestamp(
					t.duration_since(std::time::UNIX_EPOCH).ok()?.as_secs() as i64,
					0,
				)
			})
			.unwrap_or_else(chrono::Utc::now);

		let accessed_at = metadata.accessed.and_then(|t| {
			chrono::DateTime::from_timestamp(
				t.duration_since(std::time::UNIX_EPOCH).ok()?.as_secs() as i64,
				0,
			)
		});

		Self {
			id,
			sd_path,
			name,
			size: metadata.size,
			content_identity: None,
			alternate_paths: Vec::new(),
			tags: Vec::new(),
			sidecars: Vec::new(),
			image_media_data: None,
			video_media_data: None,
			audio_media_data: None,
			created_at,
			modified_at,
			accessed_at,
			content_kind: ContentKind::Unknown,
			extension,
			kind,
			is_local,
			duration_seconds: None,
			thumbnail_path: None,
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

// Register File as a virtual resource (has dependencies on entry, content_identity, etc.)
crate::register_resource!(File, virtual);
