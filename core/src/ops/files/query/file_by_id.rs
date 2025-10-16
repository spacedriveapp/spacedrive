//! Query to get a single file by ID with all related data

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

		// Get the entry
		let entry_model = entry::Entity::find()
			.filter(entry::Column::Uuid.eq(self.file_id))
			.one(db.conn())
			.await?
			.ok_or_else(|| QueryError::Internal("File not found".to_string()))?;

		// Only proceed if this is actually a file (not a directory)
		if entry_model.kind == 1 {
			return Ok(None);
		}

		// Create placeholder SdPath
		// TODO: Resolve actual path from database
		let sd_path = SdPath::Physical {
			device_slug: format!("placeholder-{}", Uuid::new_v4()),
			path: format!("/{}", entry_model.name).into(),
		};

		// Convert to File using from_entity_model
		let file = File::from_entity_model(entry_model, sd_path);

		Ok(Some(file))
	}
}

// Register the query
crate::register_library_query!(FileByIdQuery, "files.by_id");
