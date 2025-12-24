//! File domain model - the forward-facing file representation
//!
//! This module provides the File domain type that aggregates data from Entry,
//! ContentIdentity, Tags, and Sidecars into a developer-friendly interface.
//! The File struct is computed from pre-fetched data rather than fetching
//! individual pieces on demand.

use crate::domain::{
	addressing::SdPath,
	content_identity::{ContentIdentity, ContentKind},
	media_data::{AudioMediaData, ImageMediaData, VideoMediaData},
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
	pub content_kind: ContentKind, // Populated by the ephemeral indexer, for when a File does not have a ContentIdentity
	pub is_local: bool, // this is redundant with SdPath

	/// Video duration (for grid display optimization)
	pub duration_seconds: Option<f64>,
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

impl crate::domain::resource::Identifiable for File {
	fn id(&self) -> Uuid {
		self.id
	}

	fn resource_type() -> &'static str {
		"file"
	}

	fn sync_dependencies() -> &'static [&'static str] {
		&[
			"entry",
			"content_identity",
			"sidecar",
			"image_media_data",
			"video_media_data",
			"audio_media_data",
			"user_metadata",
			"user_metadata_tag",
			"tag",
		]
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

	async fn route_from_dependency(
		db: &sea_orm::DatabaseConnection,
		dependency_type: &str,
		dependency_id: Uuid,
	) -> crate::common::errors::Result<Vec<Uuid>> {
		use crate::infra::db::entities::{content_identity, entry, sidecar};
		use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};

		match dependency_type {
			// Pattern 1: Direct mapping - File ID = Entry UUID
			"entry" => Ok(vec![dependency_id]),

			// Pattern 2: Fan-out via content identity
			"content_identity" => {
				let ci = content_identity::Entity::find()
					.filter(content_identity::Column::Uuid.eq(dependency_id))
					.one(db)
					.await?
					.ok_or_else(|| {
						crate::common::errors::CoreError::NotFound(format!(
							"ContentIdentity {} not found",
							dependency_id
						))
					})?;

				let entries = entry::Entity::find()
					.filter(entry::Column::ContentId.eq(ci.id))
					.all(db)
					.await?;

				Ok(entries.into_iter().filter_map(|e| e.uuid).collect())
			}

			// Pattern 2: Fan-out via sidecar
			"sidecar" => {
				let sc = sidecar::Entity::find()
					.filter(sidecar::Column::Uuid.eq(dependency_id))
					.one(db)
					.await?
					.ok_or_else(|| {
						crate::common::errors::CoreError::NotFound(format!(
							"Sidecar {} not found",
							dependency_id
						))
					})?;

				// Find entries with matching content_identity UUID
				// Note: content_identity.uuid is Option<Uuid>, must wrap in Some()
				let ci_opt = content_identity::Entity::find()
					.filter(content_identity::Column::Uuid.eq(Some(sc.content_uuid)))
					.one(db)
					.await?;

				if let Some(ci) = ci_opt {
					let entries = entry::Entity::find()
						.filter(entry::Column::ContentId.eq(ci.id))
						.all(db)
						.await?;

					Ok(entries.into_iter().filter_map(|e| e.uuid).collect())
				} else {
					Ok(vec![])
				}
			}

			// Media data types - for now return empty, can be implemented later
			"image_media_data" | "video_media_data" | "audio_media_data" => Ok(vec![]),

			// Pattern 3: Route from user_metadata_tag to affected files
			"user_metadata_tag" => {
				use crate::infra::db::entities::user_metadata;

				// Get the user_metadata_tag to find its parent user_metadata
				let umt = crate::infra::db::entities::user_metadata_tag::Entity::find()
					.filter(
						crate::infra::db::entities::user_metadata_tag::Column::Uuid
							.eq(dependency_id),
					)
					.one(db)
					.await?
					.ok_or_else(|| {
						crate::common::errors::CoreError::NotFound(format!(
							"UserMetadataTag {} not found",
							dependency_id
						))
					})?;

				// Get the parent user_metadata
				let metadata = user_metadata::Entity::find_by_id(umt.user_metadata_id)
					.one(db)
					.await?
					.ok_or_else(|| {
						crate::common::errors::CoreError::NotFound(format!(
							"UserMetadata {} not found",
							umt.user_metadata_id
						))
					})?;

				// Route based on metadata scope (entry-scoped or content-scoped)
				if let Some(entry_uuid) = metadata.entry_uuid {
					// Entry-scoped: affects only this entry
					Ok(vec![entry_uuid])
				} else if let Some(content_uuid) = metadata.content_identity_uuid {
					// Content-scoped: affects all entries with this content
					let ci = content_identity::Entity::find()
						.filter(content_identity::Column::Uuid.eq(content_uuid))
						.one(db)
						.await?
						.ok_or_else(|| {
							crate::common::errors::CoreError::NotFound(format!(
								"ContentIdentity {} not found",
								content_uuid
							))
						})?;

					let entries = entry::Entity::find()
						.filter(entry::Column::ContentId.eq(ci.id))
						.all(db)
						.await?;

					Ok(entries.into_iter().filter_map(|e| e.uuid).collect())
				} else {
					Ok(vec![])
				}
			}

			// Pattern 3: Route from user_metadata to affected files
			"user_metadata" => {
				use crate::infra::db::entities::user_metadata;

				let metadata = user_metadata::Entity::find()
					.filter(user_metadata::Column::Uuid.eq(dependency_id))
					.one(db)
					.await?
					.ok_or_else(|| {
						crate::common::errors::CoreError::NotFound(format!(
							"UserMetadata {} not found",
							dependency_id
						))
					})?;

				// Route based on metadata scope
				if let Some(entry_uuid) = metadata.entry_uuid {
					Ok(vec![entry_uuid])
				} else if let Some(content_uuid) = metadata.content_identity_uuid {
					let ci = content_identity::Entity::find()
						.filter(content_identity::Column::Uuid.eq(content_uuid))
						.one(db)
						.await?
						.ok_or_else(|| {
							crate::common::errors::CoreError::NotFound(format!(
								"ContentIdentity {} not found",
								content_uuid
							))
						})?;

					let entries = entry::Entity::find()
						.filter(entry::Column::ContentId.eq(ci.id))
						.all(db)
						.await?;

					Ok(entries.into_iter().filter_map(|e| e.uuid).collect())
				} else {
					Ok(vec![])
				}
			}

			// Pattern 4: Route from tag changes (affects all files with this tag)
			"tag" => {
				use crate::infra::db::entities::{user_metadata, user_metadata_tag};

				// Find all user_metadata_tag records with this tag
				let tag_model = crate::infra::db::entities::tag::Entity::find()
					.filter(crate::infra::db::entities::tag::Column::Uuid.eq(dependency_id))
					.one(db)
					.await?
					.ok_or_else(|| {
						crate::common::errors::CoreError::NotFound(format!(
							"Tag {} not found",
							dependency_id
						))
					})?;

				let metadata_tags = user_metadata_tag::Entity::find()
					.filter(user_metadata_tag::Column::TagId.eq(tag_model.id))
					.all(db)
					.await?;

				let mut affected_entries = Vec::new();

				for umt in metadata_tags {
					// Get the parent user_metadata
					if let Some(metadata) = user_metadata::Entity::find_by_id(umt.user_metadata_id)
						.one(db)
						.await?
					{
						// Route based on metadata scope
						if let Some(entry_uuid) = metadata.entry_uuid {
							affected_entries.push(entry_uuid);
						} else if let Some(content_uuid) = metadata.content_identity_uuid {
							// Content-scoped: get all entries with this content
							if let Some(ci) = content_identity::Entity::find()
								.filter(content_identity::Column::Uuid.eq(content_uuid))
								.one(db)
								.await?
							{
								let entries = entry::Entity::find()
									.filter(entry::Column::ContentId.eq(ci.id))
									.all(db)
									.await?;

								affected_entries.extend(entries.into_iter().filter_map(|e| e.uuid));
							}
						}
					}
				}

				Ok(affected_entries)
			}

			_ => Ok(vec![]),
		}
	}

	async fn from_ids(
		db: &sea_orm::DatabaseConnection,
		ids: &[Uuid],
	) -> crate::common::errors::Result<Vec<Self>> {
		File::from_entry_uuids(db, ids).await
	}
}

impl File {
	/// Build a File from an entry model and item type (for space item resolution)
	///
	/// This is used by SpaceItem resolution where we know the ItemType
	/// and need to construct a File with the appropriate SdPath.
	pub async fn from_entry_model_with_item_type(
		entry_model: crate::infra::db::entities::entry::Model,
		item_type: &crate::domain::ItemType,
		db: &sea_orm::DatabaseConnection,
	) -> Option<Self> {
		use crate::domain::{ContentIdentity, ContentKind, ItemType, SdPath, Sidecar};
		use crate::infra::db::entities::{content_identity, sidecar};
		use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};

		let sd_path = match item_type {
			ItemType::Path { sd_path } => sd_path.clone(),
			_ => return None,
		};

		let content_identity = if let Some(content_id) = entry_model.content_id {
			content_identity::Entity::find_by_id(content_id)
				.one(db)
				.await
				.ok()
				.flatten()
				.map(|ci| ContentIdentity {
					uuid: ci.uuid.unwrap_or_else(Uuid::new_v4),
					kind: ContentKind::from_id(ci.kind_id),
					content_hash: ci.content_hash,
					integrity_hash: ci.integrity_hash,
					mime_type_id: ci.mime_type_id,
					text_content: ci.text_content,
					total_size: ci.total_size,
					entry_count: ci.entry_count,
					first_seen_at: ci.first_seen_at,
					last_verified_at: ci.last_verified_at,
				})
		} else {
			None
		};

		let sidecars = if let Some(ref ci) = content_identity {
			sidecar::Entity::find()
				.filter(sidecar::Column::ContentUuid.eq(ci.uuid))
				.all(db)
				.await
				.ok()
				.unwrap_or_default()
				.into_iter()
				.map(|s| Sidecar {
					id: s.id,
					content_uuid: s.content_uuid,
					kind: s.kind,
					variant: s.variant,
					format: s.format,
					status: s.status,
					size: s.size,
					created_at: s.created_at,
					updated_at: s.updated_at,
				})
				.collect()
		} else {
			Vec::new()
		};

		let mut file = File::from_entity_model(entry_model, sd_path);
		file.content_identity = content_identity;
		file.sidecars = sidecars;
		if let Some(ref ci) = file.content_identity {
			file.content_kind = ci.kind;
		}

		Some(file)
	}

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
			0 => EntryKind::File,
			1 => EntryKind::Directory,
			2 => EntryKind::Symlink,
			_ => EntryKind::File,
		};

		let extension = model.extension.clone();

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
			size: model.aggregate_size.max(model.size) as u64,
			content_identity: None,
			alternate_paths: Vec::new(),
			tags: Vec::new(),
			sidecars: Vec::new(),
			image_media_data: None,
			video_media_data: None,
			audio_media_data: None,
			created_at: model.created_at,
			modified_at: model.modified_at,
			accessed_at: model.accessed_at,
			content_kind: ContentKind::Unknown,
			extension,
			kind,
			is_local,
			duration_seconds: None,
		}
	}

	/// Construct a File from ephemeral indexing data (no database)
	///
	/// This is used for ephemeral indexing where files are discovered but not persisted to the database.
	pub fn from_ephemeral(
		id: Uuid,
		metadata: &crate::ops::indexing::database_storage::EntryMetadata,
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

	/// Batch construct File instances from entry UUIDs
	///
	/// This is used by the ResourceManager to emit File events when
	/// dependencies (Entry, ContentIdentity, Sidecar) change.
	///
	/// Efficiently loads all necessary data in batch queries and constructs
	/// fully-populated File instances.
	pub async fn from_entry_uuids(
		db: &sea_orm::DatabaseConnection,
		entry_uuids: &[Uuid],
	) -> crate::common::errors::Result<Vec<File>> {
		use crate::infra::db::entities::{content_identity, entry, location, sidecar};
		use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
		use std::collections::HashMap;

		if entry_uuids.is_empty() {
			return Ok(Vec::new());
		}

		// Batch load all entries
		let entries = entry::Entity::find()
			.filter(entry::Column::Uuid.is_in(entry_uuids.iter().copied()))
			.all(db)
			.await?;

		if entries.is_empty() {
			return Ok(Vec::new());
		}

		// Collect content_ids and location_ids for batch loading
		let content_ids: Vec<i32> = entries.iter().filter_map(|e| e.content_id).collect();

		// Load locations to build SdPaths
		// For now, we need to build a path from the entry. The challenge is that entries
		// don't store full paths - we need to traverse up to the location root.
		// This is a simplified version that creates Content-based paths when content_id exists

		// Batch load content identities
		let content_identities = if !content_ids.is_empty() {
			content_identity::Entity::find()
				.filter(content_identity::Column::Id.is_in(content_ids.clone()))
				.all(db)
				.await?
		} else {
			Vec::new()
		};

		let content_by_id: HashMap<i32, content_identity::Model> = content_identities
			.into_iter()
			.map(|ci| (ci.id, ci))
			.collect();

		// Batch load alternate paths (all entries with same content_id)
		// This populates the alternate_paths field so frontend filters can check physical locations
		let all_entries_with_content = if !content_ids.is_empty() {
			entry::Entity::find()
				.filter(entry::Column::ContentId.is_in(content_ids.clone()))
				.all(db)
				.await?
		} else {
			Vec::new()
		};

		// Group entries by content_id for alternate paths lookup
		let mut entries_by_content_id: HashMap<i32, Vec<entry::Model>> = HashMap::new();
		for e in all_entries_with_content {
			if let Some(cid) = e.content_id {
				entries_by_content_id.entry(cid).or_default().push(e);
			}
		}

		// Batch load content kinds for proper icon display
		use crate::infra::db::entities::content_kind;
		let kind_ids: Vec<i32> = content_by_id.values().map(|ci| ci.kind_id).collect();

		let content_kinds = if !kind_ids.is_empty() {
			content_kind::Entity::find()
				.filter(content_kind::Column::Id.is_in(kind_ids))
				.all(db)
				.await?
		} else {
			Vec::new()
		};

		let kind_by_id: HashMap<i32, ContentKind> = content_kinds
			.into_iter()
			.map(|ck| (ck.id, ContentKind::from_id(ck.id)))
			.collect();

		// Batch load sidecars
		let content_uuids: Vec<Uuid> = content_by_id.values().filter_map(|ci| ci.uuid).collect();

		let sidecars = if !content_uuids.is_empty() {
			sidecar::Entity::find()
				.filter(sidecar::Column::ContentUuid.is_in(content_uuids.clone()))
				.all(db)
				.await?
		} else {
			Vec::new()
		};

		let mut sidecars_by_content_uuid: HashMap<Uuid, Vec<Sidecar>> = HashMap::new();
		for s in sidecars {
			sidecars_by_content_uuid
				.entry(s.content_uuid)
				.or_default()
				.push(Sidecar {
					id: s.id,
					content_uuid: s.content_uuid,
					kind: s.kind,
					variant: s.variant,
					format: s.format,
					status: s.status,
					size: s.size,
					created_at: s.created_at,
					updated_at: s.updated_at,
				});
		}

		// Batch load tags (both entry-scoped and content-scoped)
		use crate::infra::db::entities::{tag, user_metadata, user_metadata_tag};
		let mut tags_by_entry: HashMap<Uuid, Vec<Tag>> = HashMap::new();

		// Load user_metadata for entries and content
		let metadata_records = user_metadata::Entity::find()
			.filter(
				user_metadata::Column::EntryUuid
					.is_in(entry_uuids.iter().copied())
					.or(user_metadata::Column::ContentIdentityUuid.is_in(content_uuids.clone())),
			)
			.all(db)
			.await?;

		if !metadata_records.is_empty() {
			let metadata_ids: Vec<i32> = metadata_records.iter().map(|m| m.id).collect();

			// Load user_metadata_tag records
			let metadata_tags = user_metadata_tag::Entity::find()
				.filter(user_metadata_tag::Column::UserMetadataId.is_in(metadata_ids))
				.all(db)
				.await?;

			if !metadata_tags.is_empty() {
				let tag_ids: Vec<i32> = metadata_tags.iter().map(|mt| mt.tag_id).collect();

				// Load tag entities
				let tag_models = tag::Entity::find()
					.filter(tag::Column::Id.is_in(tag_ids))
					.all(db)
					.await?;

				// Build tag_id -> Tag mapping using the tags manager converter
				let tag_map: HashMap<i32, Tag> = tag_models
					.into_iter()
					.filter_map(|t| {
						let db_id = t.id;
						crate::ops::tags::manager::model_to_domain(t)
							.ok()
							.map(|tag| (db_id, tag))
					})
					.collect();

				// Build metadata_id -> Vec<Tag> mapping
				let mut tags_by_metadata: HashMap<i32, Vec<Tag>> = HashMap::new();
				for mt in metadata_tags {
					if let Some(tag) = tag_map.get(&mt.tag_id) {
						tags_by_metadata
							.entry(mt.user_metadata_id)
							.or_default()
							.push(tag.clone());
					}
				}

				// Map tags to entries (handle both entry-scoped and content-scoped)
				for metadata in &metadata_records {
					if let Some(tags) = tags_by_metadata.get(&metadata.id) {
						// Entry-scoped metadata
						if let Some(entry_uuid) = metadata.entry_uuid {
							tags_by_entry
								.entry(entry_uuid)
								.or_default()
								.extend(tags.clone());
						}
						// Content-scoped metadata: apply to all entries with this content
						else if let Some(content_uuid) = metadata.content_identity_uuid {
							// Find all entries with this content_id
							if let Some(ci) = content_by_id
								.values()
								.find(|ci| ci.uuid == Some(content_uuid))
							{
								if let Some(alt_entries) = entries_by_content_id.get(&ci.id) {
									for alt_entry in alt_entries {
										if let Some(entry_uuid) = alt_entry.uuid {
											tags_by_entry
												.entry(entry_uuid)
												.or_default()
												.extend(tags.clone());
										}
									}
								}
							}
						}
					}
				}
			}
		}

		// Build File instances
		let mut files = Vec::new();
		for entry_model in entries {
			let entry_uuid = entry_model.uuid.ok_or_else(|| {
				crate::common::errors::CoreError::InvalidOperation(format!(
					"Entry {} missing UUID",
					entry_model.id
				))
			})?;

			// Build SdPath - use Content path if content_id exists, otherwise use Physical path
			// Physical paths are needed for newly created files that don't have content_id yet
			let sd_path = if let Some(content_id) = entry_model.content_id {
				if let Some(ci) = content_by_id.get(&content_id) {
					if let Some(ci_uuid) = ci.uuid {
						SdPath::Content {
							content_id: ci_uuid,
						}
					} else {
						tracing::warn!("Entry {} has ContentIdentity without UUID", entry_model.id);
						continue;
					}
				} else {
					// Fallback: use entry UUID as synthetic path
					// This shouldn't normally happen but provides a fallback
					tracing::warn!(
						"Entry {} has content_id but ContentIdentity not found",
						entry_model.id
					);
					continue;
				}
			} else {
				// No content identity yet - build Physical path from filesystem
				// This is common for newly created files before content hash runs
				match crate::ops::indexing::PathResolver::get_full_path(db, entry_model.id).await {
					Ok(physical_path) => {
						let device_slug = crate::device::get_current_device_slug();
						tracing::debug!(
							"Using Physical path for entry {} without content_id: {}",
							entry_model.id,
							physical_path.display()
						);
						SdPath::Physical {
							device_slug,
							path: physical_path,
						}
					}
					Err(e) => {
						tracing::warn!(
							"Failed to resolve physical path for entry {}: {}",
							entry_model.id,
							e
						);
						continue;
					}
				}
			};

			// Start with basic File from entity
			let mut file = File::from_entity_model(entry_model.clone(), sd_path.clone());

			// ALWAYS populate alternate_paths with at least the current file's physical path
			// This ensures server-side filtering works even for files without content_id
			if let SdPath::Physical { device_slug, path } = &sd_path {
				file.alternate_paths.push(SdPath::Physical {
					device_slug: device_slug.clone(),
					path: path.clone(),
				});
			} else if let SdPath::Content { .. } = &sd_path {
				// For Content paths, we'll populate from entries_by_content_id below
				// But we should still try to add the current entry's physical path
				if let Ok(physical_path) =
					crate::ops::indexing::PathResolver::get_full_path(db, entry_model.id).await
				{
					let device_slug = crate::device::get_current_device_slug();
					file.alternate_paths.push(SdPath::Physical {
						device_slug,
						path: physical_path,
					});
				}
			}

			// Enrich with content identity and alternate paths from duplicates
			if let Some(content_id) = entry_model.content_id {
				if let Some(ci) = content_by_id.get(&content_id) {
					if let Some(ci_uuid) = ci.uuid {
						file.content_identity = Some(ContentIdentity {
							uuid: ci_uuid,
							content_hash: ci.content_hash.clone(),
							integrity_hash: ci.integrity_hash.clone(),
							mime_type_id: ci.mime_type_id,
							kind: kind_by_id
								.get(&ci.kind_id)
								.copied()
								.unwrap_or(ContentKind::Unknown),
							total_size: ci.total_size,
							entry_count: ci.entry_count,
							first_seen_at: ci.first_seen_at,
							last_verified_at: ci.last_verified_at,
							text_content: ci.text_content.clone(),
						});

						// Add sidecars
						if let Some(sidecars) = sidecars_by_content_uuid.get(&ci_uuid) {
							file.sidecars = sidecars.clone();
						}

						// Add physical paths from OTHER entries with same content (duplicates)
						// Current entry's path was already added above
						if let Some(alt_entries) = entries_by_content_id.get(&content_id) {
							for alt_entry in alt_entries {
								// Skip current entry to avoid duplicate
								if alt_entry.id == entry_model.id {
									continue;
								}

								// Build physical path for each OTHER entry with this content
								if let Ok(physical_path) =
									crate::ops::indexing::PathResolver::get_full_path(
										db,
										alt_entry.id,
									)
									.await
								{
									let device_slug = crate::device::get_current_device_slug();

									file.alternate_paths.push(SdPath::Physical {
										device_slug,
										path: physical_path,
									});
								}
							}
						}
					}
				}
			}

			// Add tags from batch lookup
			if let Some(tags) = tags_by_entry.get(&entry_uuid) {
				file.tags = tags.clone();
			}

			files.push(file);
		}

		Ok(files)
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

// Register File as a virtual resource (has dependencies on entry, content_identity, etc.)
crate::register_resource!(File, virtual);
