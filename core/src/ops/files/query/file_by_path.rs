//! Query to get a single file by local path with all related data

use crate::infra::query::{QueryError, QueryResult};
use crate::{
	context::CoreContext,
	domain::{addressing::SdPath, file::FileConstructionData, File},
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

		// Create a minimal Entry from the database model
		// Use the SdPath that was created internally
		let entry = crate::domain::Entry {
			id: entry_model.uuid.unwrap_or_else(Uuid::new_v4),
			sd_path: crate::domain::entry::SdPathSerialized::from_sdpath(&sd_path).unwrap_or_else(
				|| crate::domain::entry::SdPathSerialized {
					device_id: Uuid::new_v4(),
					path: "/unknown/path".to_string(),
				},
			),
			name: entry_model.name,
			kind: match entry_model.kind {
				0 => crate::domain::entry::EntryKind::File {
					extension: entry_model.extension,
				},
				1 => crate::domain::entry::EntryKind::Directory,
				2 => crate::domain::entry::EntryKind::Symlink {
					target: String::new(),
				},
				_ => crate::domain::entry::EntryKind::File {
					extension: entry_model.extension,
				},
			},
			size: Some(entry_model.size as u64),
			created_at: Some(entry_model.created_at),
			modified_at: Some(entry_model.modified_at),
			accessed_at: entry_model.accessed_at,
			inode: entry_model.inode.map(|i| i as u64),
			file_id: None,
			parent_id: entry_model.parent_id.map(|id| Uuid::new_v4()), // This would need proper conversion
			location_id: None,
			metadata_id: entry_model
				.metadata_id
				.map(|id| Uuid::new_v4())
				.unwrap_or_else(Uuid::new_v4),
			content_id: entry_model.content_id.map(|id| Uuid::new_v4()), // This would need proper conversion
			first_seen_at: entry_model.created_at,
			last_indexed_at: None,
		};

		// Only proceed if this is actually a file (not a directory)
		if !entry.is_file() {
			return Ok(None);
		}

		// For now, return minimal data to avoid complex UUID conversions
		// TODO: Implement proper data loading with correct ID mappings
		let content_identity = self.load_content_identity(&entry, db.conn()).await?;
		let tags = self.load_tags(&entry, db.conn()).await?;
		let sidecars = self.load_sidecars(&entry, db.conn()).await?;
		let alternate_paths = self.load_alternate_paths(&entry, db.conn()).await?;

		// Construct the file
		let construction_data = FileConstructionData {
			entry,
			content_identity,
			tags,
			sidecars,
			alternate_paths,
		};

		Ok(Some(File::from_data(construction_data)))
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
		}
	}

	/// Load content identity for the entry
	async fn load_content_identity(
		&self,
		_entry: &crate::domain::Entry,
		_db: &DatabaseConnection,
	) -> QueryResult<Option<crate::domain::ContentIdentity>> {
		// TODO: Implement proper content identity loading
		Ok(None)
	}

	/// Load tags for the entry
	async fn load_tags(
		&self,
		_entry: &crate::domain::Entry,
		_db: &DatabaseConnection,
	) -> QueryResult<Vec<crate::domain::Tag>> {
		// TODO: Implement proper tag loading
		Ok(Vec::new())
	}

	/// Load sidecars for the entry
	async fn load_sidecars(
		&self,
		_entry: &crate::domain::Entry,
		_db: &DatabaseConnection,
	) -> QueryResult<Vec<crate::domain::Sidecar>> {
		// TODO: Implement proper sidecar loading
		Ok(Vec::new())
	}

	/// Load alternate paths for the entry
	async fn load_alternate_paths(
		&self,
		_entry: &crate::domain::Entry,
		_db: &DatabaseConnection,
	) -> QueryResult<Vec<crate::domain::addressing::SdPath>> {
		// TODO: Implement proper alternate paths loading
		Ok(Vec::new())
	}
}

crate::register_library_query!(FileByPathQuery, "files.by_path");
