//! Query to get a single file by ID with all related data

use crate::infra::query::{QueryError, QueryResult};
use crate::{
	context::CoreContext,
	domain::{addressing::SdPath, File},
	infra::db::entities::{
		audio_media_data, content_identity, device, directory_paths, entry, image_media_data,
		location, sidecar, tag, user_metadata, user_metadata_tag, video_media_data,
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
		let _ = &session;

		let cache = context.ephemeral_cache();
		let Some(path) = cache.path_of_record(self.file_id).await else {
			return Ok(None);
		};

		let index = cache.resolve_index(&path);
		let mut index = index.write().await;
		let Some(metadata) = index.get_entry_ref(&path) else {
			return Ok(None);
		};
		let content_kind = index.get_content_kind(&path);
		drop(index);

		let mut file = File::from_ephemeral(self.file_id, &metadata, SdPath::local(path));
		file.content_kind = content_kind;

		Ok(Some(file))
	}
}

// Register the query
crate::register_library_query!(FileByIdQuery, "files.by_id");
