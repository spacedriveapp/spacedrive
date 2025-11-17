//! Query to get a single file by local path with all related data

use crate::infra::query::{QueryError, QueryResult};
use crate::{
	context::CoreContext,
	domain::{addressing::SdPath, File},
	infra::db::entities::{
		audio_media_data, content_identity, entry, image_media_data, sidecar, tag,
		user_metadata_tag, video_media_data,
	},
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

/// Query to get a file by its local path with all related data
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct FileByPathQuery {
	pub path: PathBuf,
}

impl FileByPathQuery {
	pub fn new(path: PathBuf) -> Self {
		Self { path }
	}
}

impl LibraryQuery for FileByPathQuery {
	type Input = FileByPathQuery;
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

		// Convert the local path to SdPath internally
		let sd_path = SdPath::local(self.path.clone());

		// Find the entry by SdPath
		let entry_model = self.find_entry_by_sd_path(&sd_path, db.conn()).await?;

		// Only proceed if this is actually a file (not a directory)
		// if entry_model.kind == 1 {
		// 	return Ok(None);
		// }

		// Fetch content identity, sidecars, and media data if file has content_id
		let (content_identity_domain, sidecars, image_media, video_media, audio_media) =
			if let Some(content_id) = entry_model.content_id {
				// Get content identity
				if let Some(content_identity_model) =
					content_identity::Entity::find_by_id(content_id)
						.one(db.conn())
						.await?
				{
					let content_uuid = content_identity_model.uuid;

					// Load media data in parallel
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
						uuid: content_identity_model
							.uuid
							.unwrap_or_else(|| Uuid::new_v4()),
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

					(
						Some(content_identity),
						sidecars,
						image_media_opt,
						video_media_opt,
						audio_media_opt,
					)
				} else {
					(None, Vec::new(), None, None, None)
				}
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

impl FileByPathQuery {
	/// Find entry by SdPath
	async fn find_entry_by_sd_path(
		&self,
		sd_path: &SdPath,
		db: &DatabaseConnection,
	) -> QueryResult<entry::Model> {
		match sd_path {
			SdPath::Physical { .. } | SdPath::Cloud { .. } => {
				// Use SdPath API for consistent path handling
				let file_name = sd_path
					.file_name()
					.ok_or_else(|| QueryError::Internal("Invalid file name in path".to_string()))?;

				let parent_sd_path = sd_path
					.parent()
					.ok_or_else(|| QueryError::Internal("No parent directory".to_string()))?;

				// Parse extension from filename
				let (name, extension) = if let Some(dot_idx) = file_name.rfind('.') {
					let name_without_ext = &file_name[..dot_idx];
					let ext = &file_name[dot_idx + 1..];
					(name_without_ext.to_string(), Some(ext.to_string()))
				} else {
					(file_name.to_string(), None)
				};

				// Find entries with matching name and extension
				let mut query = entry::Entity::find()
					.filter(entry::Column::Name.eq(&name))
					.filter(entry::Column::Kind.eq(0)); // Only files, not directories

				if let Some(ext) = &extension {
					query = query.filter(entry::Column::Extension.eq(ext));
				}

				let entries = query.all(db).await?;

				// Get parent path string for comparison
				let parent_path_str = match &parent_sd_path {
					SdPath::Physical { path, .. } => path.to_string_lossy().to_string(),
					SdPath::Cloud { path, .. } => path.clone(),
					_ => return Err(QueryError::Internal("Invalid parent path".to_string())),
				};

				// For each matching entry, check if its parent directory path matches
				for entry_model in entries {
					if let Some(parent_id) = entry_model.parent_id {
						// Get parent directory path
						if let Ok(parent_path_model) =
							crate::infra::db::entities::directory_paths::Entity::find_by_id(
								parent_id,
							)
							.one(db)
							.await
						{
							if let Some(parent_path_model) = parent_path_model {
								// Check if the parent directory path matches
								if parent_path_model.path == parent_path_str {
									return Ok(entry_model);
								}
							}
						}
					}
				}

				Err(QueryError::Internal(format!(
					"File not found at path: {}",
					sd_path.display()
				)))
			}
			SdPath::Content { content_id } => {
				// For content-addressed paths, find any entry with this content_id
				// First we need to find the content_identity with this UUID
				let content_identity = crate::infra::db::entities::content_identity::Entity::find()
					.filter(
						crate::infra::db::entities::content_identity::Column::Uuid.eq(*content_id),
					)
					.one(db)
					.await?
					.ok_or_else(|| {
						QueryError::Internal("Content identity not found".to_string())
					})?;

				// Find any entry with this content_id
				entry::Entity::find()
					.filter(entry::Column::ContentId.eq(content_identity.id))
					.one(db)
					.await?
					.ok_or_else(|| QueryError::Internal("Entry not found for content".to_string()))
			}
			SdPath::Sidecar { .. } => {
				return Err(QueryError::Internal(
					"Sidecar paths not yet implemented for file queries".to_string(),
				));
			}
		}
	}
}

crate::register_library_query!(FileByPathQuery, "files.by_path");
