//! Query to get a single file by local path with all related data

use crate::infra::query::{QueryError, QueryResult};
use crate::{
	context::CoreContext,
	domain::{addressing::SdPath, File},
	infra::db::entities::{content_identity, entry, sidecar, tag, user_metadata_tag},
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

		// Convert to File using from_entity_model
		let file = File::from_entity_model(entry_model, sd_path);

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
