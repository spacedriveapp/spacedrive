//! Query to get a single file by local path with all related data

use crate::infra::query::QueryResult;
use crate::{
	context::CoreContext,
	domain::{addressing::SdPath, File},
	infra::query::LibraryQuery,
};
use serde::{Deserialize, Serialize};
use specta::Type;
use std::{path::PathBuf, sync::Arc};

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

		// The index holds the volume's spelling, so a path reached through a
		// firmlink such as /Users/me is rewritten before the lookup. The
		// answer goes back in the caller's spelling once the decorators,
		// which find the store by path, are done with it.
		let path = context.volume_manager.index_spelling(&self.path).await;

		// The arena is the index for every drive; the store is its durable
		// half rather than a second answer.
		let volume_index = context.volume_index();
		volume_index.ensure_restored(&path).await;
		let index = volume_index.resolve_index(&path);
		let index_read = index.read().await;

		if let Some(entry_uuid) = index_read.get_entry_uuid(&path) {
			if let Some(metadata) = index_read.get_entry_ref(&path) {
				let content_kind = index_read.get_content_kind(&path);
				let sd_path = SdPath::local(path.clone());

				let mut file = File::from_arena(entry_uuid, &metadata, sd_path);
				file.content_kind = content_kind;
				drop(index_read);

				let mut files = [file];
				crate::ops::tags::decorate::decorate_files(&volume_index, &mut files).await;
				crate::ops::indexing::kinds::decorate_kinds(&volume_index, &mut files).await;
				let [mut file] = files;
				file.sd_path = SdPath::local(self.path);

				return Ok(Some(file));
			}
		}

		Ok(None)
	}
}

crate::register_library_query!(FileByPathQuery, "files.by_path");
