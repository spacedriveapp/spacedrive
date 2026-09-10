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
		let _ = &session;

		// The arena is the index for every drive; the store is its durable
		// half rather than a second answer.
		let ephemeral_cache = context.ephemeral_cache();
		ephemeral_cache.ensure_restored(&self.path).await;
		let index = ephemeral_cache.resolve_index(&self.path);
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

crate::register_library_query!(FileByPathQuery, "files.by_path");
