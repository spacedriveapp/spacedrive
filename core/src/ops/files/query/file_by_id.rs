//! Query to get a single file by ID with all related data

use crate::infra::query::{QueryError, QueryResult};
use crate::{
	context::CoreContext,
	domain::{addressing::SdPath, File},
	infra::db::entities::{audio_media_data, content_identity, device, directory_paths, entry, image_media_data, location, sidecar, tag, user_metadata_tag, video_media_data},
	infra::query::LibraryQuery,
};
use sea_orm::{
	ColumnTrait, ConnectionTrait, DatabaseConnection, EntityTrait, JoinType, QueryFilter,
	QuerySelect, RelationTrait,
};
use serde::{Deserialize, Serialize};
use specta::Type;
use std::{path::PathBuf, sync::Arc};
use uuid::Uuid;

/// Query to get a file by its ID with all related data
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct FileByIdQuery {
	pub file_id: Uuid,
}

impl FileByIdQuery {
	pub fn new(file_id: Uuid) -> Self {
		Self { file_id }
	}
}

impl LibraryQuery for FileByIdQuery {
	type Input = FileByIdQuery;
	type Output = Option<File>;

	fn from_input(input: Self::Input) -> QueryResult<Self> {
		Ok(input)
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

		// Get the entry with all related data in one query using JOINs
		let entry_with_relations = entry::Entity::find()
			.filter(entry::Column::Uuid.eq(self.file_id))
			.find_also_related(content_identity::Entity)
			.one(db.conn())
			.await?
			.ok_or_else(|| QueryError::Internal("File not found".to_string()))?;

		let (entry_model, content_identity_model_opt) = entry_with_relations;

		// Only proceed if this is actually a file (not a directory)
		if entry_model.kind == 1 {
			return Ok(None);
		}

		// Resolve the full absolute path for this file
		let sd_path = self.resolve_file_path(&entry_model, db.conn()).await?;

		// Process content identity and load media data
		let (content_identity_domain, sidecars, image_media, video_media, audio_media) = if let Some(content_identity_model) = content_identity_model_opt {
			let content_uuid = content_identity_model.uuid;

			// Load media data in parallel using JOINs
			let (image_media_opt, video_media_opt, audio_media_opt) = tokio::join!(
				async {
					if let Some(image_id) = content_identity_model.image_media_data_id {
						image_media_data::Entity::find_by_id(image_id)
							.one(db.conn())
							.await
							.ok()
							.flatten()
							.map(Into::into)
					} else {
						None
					}
				},
				async {
					if let Some(video_id) = content_identity_model.video_media_data_id {
						video_media_data::Entity::find_by_id(video_id)
							.one(db.conn())
							.await
							.ok()
							.flatten()
							.map(Into::into)
					} else {
						None
					}
				},
				async {
					if let Some(audio_id) = content_identity_model.audio_media_data_id {
						audio_media_data::Entity::find_by_id(audio_id)
							.one(db.conn())
							.await
							.ok()
							.flatten()
							.map(Into::into)
					} else {
						None
					}
				}
			);

			// Fetch sidecars for this content UUID
			let sidecars = if let Some(uuid) = content_uuid {
				sidecar::Entity::find()
					.filter(sidecar::Column::ContentUuid.eq(uuid))
					.all(db.conn())
					.await?
					.into_iter()
					.map(|s| crate::domain::Sidecar {
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

			// Convert content_identity to domain type
			let content_identity = crate::domain::ContentIdentity {
				uuid: content_identity_model.uuid.unwrap_or_else(|| Uuid::new_v4()),
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

			(Some(content_identity), sidecars, image_media_opt, video_media_opt, audio_media_opt)
		} else {
			(None, Vec::new(), None, None, None)
		};

		// Convert to File using from_entity_model
		let mut file = File::from_entity_model(entry_model, sd_path);
		file.sidecars = sidecars;
		file.content_identity = content_identity_domain;
		file.image_media_data = image_media;
		file.video_media_data = video_media.clone();
		file.audio_media_data = audio_media;
		file.duration_seconds = video_media.as_ref().and_then(|v| v.duration_seconds);
		if let Some(ref ci) = file.content_identity {
			file.content_kind = ci.kind;
		}

		Ok(Some(file))
	}
}

impl FileByIdQuery {
	/// Resolve the full absolute SdPath for a file entry
	async fn resolve_file_path(
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
crate::register_library_query!(FileByIdQuery, "files.by_id");
