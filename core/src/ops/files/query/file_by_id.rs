//! Query to get a single file by ID with all related data

use crate::infra::query::{QueryError, QueryResult};
use crate::{
	context::CoreContext,
	domain::{file::FileConstructionData, File},
	infra::db::entities::{content_identity, entry, sidecar, tag, user_metadata_tag},
	infra::query::LibraryQuery,
};
use sea_orm::{
	ColumnTrait, ConnectionTrait, DatabaseConnection, EntityTrait, JoinType, QueryFilter,
	QuerySelect, RelationTrait,
};
use serde::{Deserialize, Serialize};
use specta::Type;
use std::sync::Arc;
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

		// Get the entry first
		// TODO: Fix UUID to i32 conversion - using a simplified approach for now
		let entry_model = entry::Entity::find()
			.filter(entry::Column::Uuid.eq(self.file_id))
			.one(db.conn())
			.await?
			.ok_or_else(|| QueryError::Internal("File not found".to_string()))?;

		// Create a minimal Entry from the database model
		let entry = crate::domain::Entry {
			id: entry_model.uuid.unwrap_or_else(Uuid::new_v4),
			sd_path: crate::domain::entry::SdPathSerialized {
				device_id: Uuid::new_v4(),         // Placeholder
				path: "/unknown/path".to_string(), // Placeholder
			},
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
			parent_id: entry_model.parent_id.map(|id| Uuid::new_v4()), // Placeholder
			location_id: None,
			metadata_id: entry_model
				.metadata_id
				.map(|id| Uuid::new_v4())
				.unwrap_or_else(Uuid::new_v4),
			content_id: entry_model.content_id.map(|id| Uuid::new_v4()), // Placeholder
			first_seen_at: entry_model.created_at,
			last_indexed_at: None,
		};

		// Only proceed if this is actually a file (not a directory)
		if !entry.is_file() {
			return Ok(None);
		}

		// For now, return minimal data to avoid complex UUID conversions
		// TODO: Implement proper data loading with correct ID mappings
		let content_identity = None;
		let tags = Vec::new();
		let sidecars = Vec::new();
		let alternate_paths = Vec::new();

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

// Register the query
crate::register_library_query!(FileByIdQuery, "files.by_id");
