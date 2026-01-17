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
		// Check database first (primary source of truth)
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
		let entry_result = self.find_entry_by_sd_path(&sd_path, db.conn()).await;

		// If found in database, process and return it
		if let Ok(entry_model) = entry_result {
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
							kind: crate::domain::ContentKind::from_id(
								content_identity_model.kind_id,
							),
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
			let mut file = File::from_entity_model(entry_model.clone(), sd_path);
			file.sidecars = sidecars;
			file.content_identity = content_identity_domain;
			file.image_media_data = image_media;
			file.video_media_data = video_media.clone();
			file.audio_media_data = audio_media;
			file.duration_seconds = video_media.as_ref().and_then(|v| v.duration_seconds);
			if let Some(ref ci) = file.content_identity {
				file.content_kind = ci.kind;
			}

			// Populate alternate paths (other instances of same content)
			if let Some(content_id) = entry_model.content_id {
				file.alternate_paths = self
					.get_alternate_paths(content_id, entry_model.id, db.conn())
					.await?;
			}

			return Ok(Some(file));
		}

		// Fall back to ephemeral index if not found in database
		let ephemeral_cache = context.ephemeral_cache();
		let index = ephemeral_cache.get_global_index();
		let index_read = index.read().await;

		if let Some(entry_uuid) = index_read.get_entry_uuid(&self.path) {
			if let Some(metadata) = index_read.get_entry_ref(&self.path) {
				let content_kind = index_read.get_content_kind(&self.path);
				let sd_path = SdPath::local(self.path.clone());

				let mut file = File::from_ephemeral(entry_uuid, &metadata, sd_path);
				file.content_kind = content_kind;

				return Ok(Some(file));
			}
		}

		Ok(None)
	}
}

impl FileByPathQuery {
	/// Get alternate paths for all other entries with the same content_id
	async fn get_alternate_paths(
		&self,
		content_id: i32,
		current_entry_id: i32,
		db: &DatabaseConnection,
	) -> QueryResult<Vec<SdPath>> {
		use crate::infra::db::entities::{device, directory_paths, location};

		// Find all entries with the same content_id (excluding current entry)
		let alternate_entries = entry::Entity::find()
			.filter(entry::Column::ContentId.eq(content_id))
			.filter(entry::Column::Id.ne(current_entry_id))
			.all(db)
			.await?;

		let mut alternate_paths = Vec::new();

		// Resolve path for each alternate entry
		for alt_entry in alternate_entries {
			// Build the full path for this entry
			let mut path_components = Vec::new();

			// Add the file name with extension
			let file_name = if let Some(ext) = &alt_entry.extension {
				format!("{}.{}", alt_entry.name, ext)
			} else {
				alt_entry.name.clone()
			};
			path_components.push(file_name);

			// Walk up parent chain
			let mut current_parent_id = alt_entry.parent_id;
			let mut location_entry_id = None;

			while let Some(parent_id) = current_parent_id {
				if let Some(parent) = entry::Entity::find_by_id(parent_id).one(db).await? {
					if parent.parent_id.is_none() {
						location_entry_id = Some(parent.id);
						break;
					}
					path_components.push(parent.name.clone());
					current_parent_id = parent.parent_id;
				} else {
					break;
				}
			}

			if let Some(location_entry_id) = location_entry_id {
				path_components.reverse();

				// Get location and device info
				if let Some(location_model) = location::Entity::find()
					.filter(location::Column::EntryId.eq(location_entry_id))
					.one(db)
					.await?
				{
					if let Some(device_model) = device::Entity::find_by_id(location_model.device_id)
						.one(db)
						.await?
					{
						if let Some(location_root_path) = directory_paths::Entity::find()
							.filter(directory_paths::Column::EntryId.eq(location_entry_id))
							.one(db)
							.await?
						{
							// Build absolute path
							let mut absolute_path = PathBuf::from(&location_root_path.path);
							for component in path_components {
								absolute_path.push(component);
							}

							alternate_paths.push(SdPath::Physical {
								device_slug: device_model.slug,
								path: absolute_path.into(),
							});
						}
					}
				}
			}
		}

		Ok(alternate_paths)
	}

	/// Find entry by SdPath using canonical PathResolver
	async fn find_entry_by_sd_path(
		&self,
		sd_path: &SdPath,
		db: &DatabaseConnection,
	) -> QueryResult<entry::Model> {
		use crate::ops::indexing::PathResolver;

		PathResolver::resolve_to_entry(db, sd_path)
			.await
			.map_err(|e| QueryError::Internal(format!("Database error: {}", e)))?
			.ok_or_else(|| {
				QueryError::Internal(format!("Entry not found for path: {}", sd_path.display()))
			})
	}
}

crate::register_library_query!(FileByPathQuery, "files.by_path");
