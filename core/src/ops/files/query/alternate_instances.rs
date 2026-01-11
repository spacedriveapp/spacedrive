//! Query to get all alternate instances of a file by entry ID
//!
//! This query finds all other entries that share the same content_id and returns
//! them as complete File objects with all related data (tags, sidecars, media data).

use crate::infra::query::{QueryError, QueryResult};
use crate::{
	context::CoreContext,
	domain::{addressing::SdPath, content_identity::ContentIdentity, file::File},
	infra::db::entities::{
		content_identity, device, directory_paths, entry, location, sidecar, tag, user_metadata,
		user_metadata_tag, video_media_data,
	},
	infra::query::LibraryQuery,
};
use sea_orm::{ColumnTrait, ConnectionTrait, DatabaseConnection, EntityTrait, QueryFilter};
use serde::{Deserialize, Serialize};
use specta::Type;
use std::{collections::HashMap, path::PathBuf, sync::Arc};
use uuid::Uuid;

/// Input for alternate instances query
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct AlternateInstancesInput {
	/// The entry UUID to find alternates for
	pub entry_uuid: Uuid,
}

/// Output containing alternate instances
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct AlternateInstancesOutput {
	/// All instances of this file (including the original)
	pub instances: Vec<File>,
	/// Total number of instances found
	pub total_count: u32,
}

/// Query to get alternate instances of a file
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct AlternateInstancesQuery {
	pub input: AlternateInstancesInput,
}

impl AlternateInstancesQuery {
	pub fn new(entry_uuid: Uuid) -> Self {
		Self {
			input: AlternateInstancesInput { entry_uuid },
		}
	}
}

impl LibraryQuery for AlternateInstancesQuery {
	type Input = AlternateInstancesInput;
	type Output = AlternateInstancesOutput;

	fn from_input(input: Self::Input) -> QueryResult<Self> {
		Ok(Self { input })
	}

	async fn execute(
		self,
		context: Arc<CoreContext>,
		session: crate::infra::api::SessionContext,
	) -> QueryResult<Self::Output> {
		let library_id = session
			.current_library_id
			.ok_or_else(|| QueryError::Internal("No library in session".to_string()))?;

		let library = context
			.libraries()
			.await
			.get_library(library_id)
			.await
			.ok_or_else(|| QueryError::Internal("Library not found".to_string()))?;

		let db = library.db();

		// Find the original entry
		let original_entry = entry::Entity::find()
			.filter(entry::Column::Uuid.eq(self.input.entry_uuid))
			.one(db.conn())
			.await?
			.ok_or_else(|| QueryError::Internal("Entry not found".to_string()))?;

		// Get the content_id
		let content_id = original_entry.content_id.ok_or_else(|| {
			QueryError::Internal(
				"Entry has no content identity, cannot find alternates".to_string(),
			)
		})?;

		// Find all entries with the same content_id
		let alternate_entries = entry::Entity::find()
			.filter(entry::Column::ContentId.eq(content_id))
			.all(db.conn())
			.await?;

		if alternate_entries.is_empty() {
			return Ok(AlternateInstancesOutput {
				instances: Vec::new(),
				total_count: 0,
			});
		}

		// Batch load content identity
		let content_identity_model = content_identity::Entity::find_by_id(content_id)
			.one(db.conn())
			.await?
			.ok_or_else(|| QueryError::Internal("Content identity not found".to_string()))?;

		let content_uuid = content_identity_model.uuid;

		// Batch load sidecars
		let sidecars = if let Some(ci_uuid) = content_uuid {
			sidecar::Entity::find()
				.filter(sidecar::Column::ContentUuid.eq(ci_uuid))
				.all(db.conn())
				.await?
				.into_iter()
				.map(|s| crate::domain::file::Sidecar {
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

		// Batch load tags for all entries
		let entry_uuids: Vec<Uuid> = alternate_entries.iter().filter_map(|e| e.uuid).collect();

		let mut tags_by_entry: HashMap<Uuid, Vec<crate::domain::tag::Tag>> = HashMap::new();

		if !entry_uuids.is_empty() || content_uuid.is_some() {
			// Load user_metadata for entries and content
			let mut filter = user_metadata::Column::EntryUuid.is_in(entry_uuids.clone());
			if let Some(ci_uuid) = content_uuid {
				filter = filter.or(user_metadata::Column::ContentIdentityUuid.eq(ci_uuid));
			}

			let metadata_records = user_metadata::Entity::find()
				.filter(filter)
				.all(db.conn())
				.await?;

			if !metadata_records.is_empty() {
				let metadata_ids: Vec<i32> = metadata_records.iter().map(|m| m.id).collect();

				// Load user_metadata_tag records
				let metadata_tags = user_metadata_tag::Entity::find()
					.filter(user_metadata_tag::Column::UserMetadataId.is_in(metadata_ids))
					.all(db.conn())
					.await?;

				if !metadata_tags.is_empty() {
					let tag_ids: Vec<i32> = metadata_tags.iter().map(|mt| mt.tag_id).collect();

					// Load tag entities
					let tag_models = tag::Entity::find()
						.filter(tag::Column::Id.is_in(tag_ids))
						.all(db.conn())
						.await?;

					// Build tag_id -> Tag mapping
					let tag_map: HashMap<i32, crate::domain::tag::Tag> = tag_models
						.into_iter()
						.filter_map(|t| {
							let db_id = t.id;
							crate::ops::tags::manager::model_to_domain(t)
								.ok()
								.map(|tag| (db_id, tag))
						})
						.collect();

					// Build metadata_id -> Vec<Tag> mapping
					let mut tags_by_metadata: HashMap<i32, Vec<crate::domain::tag::Tag>> =
						HashMap::new();
					for mt in metadata_tags {
						if let Some(tag) = tag_map.get(&mt.tag_id) {
							tags_by_metadata
								.entry(mt.user_metadata_id)
								.or_insert_with(Vec::new)
								.push(tag.clone());
						}
					}

					// Map tags to entries (prioritize entry-scoped, fall back to content-scoped)
					for metadata in &metadata_records {
						if let Some(tags) = tags_by_metadata.get(&metadata.id) {
							// Entry-scoped metadata (higher priority)
							if let Some(entry_uuid) = metadata.entry_uuid {
								tags_by_entry.insert(entry_uuid, tags.clone());
							}
							// Content-scoped metadata (applies to all entries with this content)
							else if let Some(_content_uuid) = metadata.content_identity_uuid {
								// Apply to all entries
								for entry_uuid in &entry_uuids {
									tags_by_entry
										.entry(*entry_uuid)
										.or_insert_with(|| tags.clone());
								}
							}
						}
					}
				}
			}
		}

		// Build content identity domain object
		let content_identity_domain = ContentIdentity {
			uuid: content_uuid.unwrap_or_else(Uuid::new_v4),
			kind: crate::domain::ContentKind::from_id(content_identity_model.kind_id),
			content_hash: content_identity_model.content_hash,
			integrity_hash: content_identity_model.integrity_hash,
			mime_type_id: content_identity_model.mime_type_id,
			text_content: content_identity_model.text_content,
			total_size: content_identity_model.total_size,
			entry_count: content_identity_model.entry_count,
			first_seen_at: content_identity_model.first_seen_at,
			last_verified_at: content_identity_model.last_verified_at,
		};

		// Load media data if available
		let video_media_data = if let Some(video_id) = content_identity_model.video_media_data_id {
			video_media_data::Entity::find_by_id(video_id)
				.one(db.conn())
				.await?
				.map(Into::into)
		} else {
			None
		};

		// Convert each entry to a complete File object
		let mut instances = Vec::new();
		for entry_model in alternate_entries {
			// Resolve full path for this entry
			let sd_path = match self.resolve_entry_path(&entry_model, db.conn()).await {
				Ok(path) => path,
				Err(e) => {
					tracing::warn!("Failed to resolve path for entry {}: {}", entry_model.id, e);
					continue;
				}
			};

			// Create File from entry model
			let mut file = File::from_entity_model(entry_model.clone(), sd_path);

			// Add content identity, sidecars, and media data
			file.content_identity = Some(content_identity_domain.clone());
			file.sidecars = sidecars.clone();
			file.video_media_data = video_media_data.clone();
			file.content_kind = content_identity_domain.kind;
			file.duration_seconds = video_media_data.as_ref().and_then(|v| v.duration_seconds);

			// Add tags for this specific entry
			if let Some(entry_uuid) = entry_model.uuid {
				if let Some(tags) = tags_by_entry.get(&entry_uuid) {
					file.tags = tags.clone();
				}
			}

			instances.push(file);
		}

		let total_count = instances.len() as u32;

		Ok(AlternateInstancesOutput {
			instances,
			total_count,
		})
	}
}

impl AlternateInstancesQuery {
	/// Resolve the full absolute SdPath for an entry
	async fn resolve_entry_path(
		&self,
		entry: &entry::Model,
		db: &DatabaseConnection,
	) -> QueryResult<SdPath> {
		// Walk up the entry hierarchy to build the full path
		let mut path_components = Vec::new();

		// Add the file name with extension
		let file_name = if let Some(ext) = &entry.extension {
			format!("{}.{}", entry.name, ext)
		} else {
			entry.name.clone()
		};
		path_components.push(file_name);

		// Walk up parent chain
		let mut current_parent_id = entry.parent_id;
		let mut location_entry_id = None;

		while let Some(parent_id) = current_parent_id {
			let parent = entry::Entity::find_by_id(parent_id)
				.one(db)
				.await?
				.ok_or_else(|| QueryError::Internal("Parent entry not found".to_string()))?;

			// Check if this is the location root (no parent)
			if parent.parent_id.is_none() {
				location_entry_id = Some(parent.id);
				break;
			}

			// Add parent directory name to path
			path_components.push(parent.name.clone());
			current_parent_id = parent.parent_id;
		}

		// Reverse to get correct order (root -> file)
		path_components.reverse();

		// Get location info
		let location_entry_id = location_entry_id
			.ok_or_else(|| QueryError::Internal("Could not find location root".to_string()))?;

		let location_model = location::Entity::find()
			.filter(location::Column::EntryId.eq(location_entry_id))
			.one(db)
			.await?
			.ok_or_else(|| QueryError::Internal("Location not found for entry".to_string()))?;

		// Get device slug
		let device_model = device::Entity::find_by_id(location_model.device_id)
			.one(db)
			.await?
			.ok_or_else(|| QueryError::Internal("Device not found".to_string()))?;

		// Get location root absolute path
		let location_root_path = directory_paths::Entity::find()
			.filter(directory_paths::Column::EntryId.eq(location_entry_id))
			.one(db)
			.await?
			.ok_or_else(|| QueryError::Internal("Location root path not found".to_string()))?;

		// Build absolute path: location_root + relative components
		let mut absolute_path = PathBuf::from(&location_root_path.path);
		for component in path_components {
			absolute_path.push(component);
		}

		Ok(SdPath::Physical {
			device_slug: device_model.slug,
			path: absolute_path.into(),
		})
	}
}

// Register the query
crate::register_library_query!(AlternateInstancesQuery, "files.alternate_instances");
