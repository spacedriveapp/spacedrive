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

		// A uuid gives no path to route by, so every partition is checked.
		let ephemeral_cache = context.ephemeral_cache();
		for index in ephemeral_cache.all_indexes() {
			let index_read = index.read().await;

			if let Some(path) = index_read.get_path_by_uuid(self.file_id) {
				if let Some(metadata) = index_read.get_entry_ref(&path) {
					let content_kind = index_read.get_content_kind(&path);
					let sd_path = SdPath::local(path.clone());

					let mut file = File::from_ephemeral(self.file_id, &metadata, sd_path);
					file.content_kind = content_kind;

					return Ok(Some(file));
				}
			}
		}

		Ok(None)
	}
}

// Register the query
crate::register_library_query!(FileByIdQuery, "files.by_id");
