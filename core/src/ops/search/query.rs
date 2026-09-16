//! File search query implementation

use super::{
	input::{FileSearchInput, SearchScope},
	output::FileSearchOutput,
};
use crate::infra::query::{QueryError, QueryResult};
use crate::{context::CoreContext, infra::query::LibraryQuery};
use serde::{Deserialize, Serialize};
use specta::Type;
use std::sync::Arc;
use uuid::Uuid;

/// File search query
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct FileSearchQuery {
	pub input: FileSearchInput,
}

impl FileSearchQuery {
	pub fn new(input: FileSearchInput) -> Self {
		Self { input }
	}
}

impl LibraryQuery for FileSearchQuery {
	type Input = FileSearchInput;
	type Output = FileSearchOutput;

	fn from_input(input: Self::Input) -> QueryResult<Self> {
		Ok(Self { input })
	}

	async fn execute(
		self,
		context: Arc<CoreContext>,
		_session: crate::infra::api::SessionContext,
	) -> QueryResult<Self::Output> {
		let start_time = std::time::Instant::now();

		self.input
			.validate()
			.map_err(|e| QueryError::Internal(format!("Invalid search input: {e}")))?;

		let search_id = Uuid::new_v4();

		tracing::info!(
			"Search query: '{}', scope: {:?}, mode: {:?}, limit: {}, offset: {}",
			self.input.query,
			self.input.scope,
			self.input.mode,
			self.input.pagination.limit,
			self.input.pagination.offset
		);

		self.search_index(context, search_id, start_time).await
	}
}

impl FileSearchQuery {
	/// A search reads the volume index: one partition when the scope names a
	/// path, every partition when it does not.
	async fn search_index(
		&self,
		context: Arc<CoreContext>,
		search_id: Uuid,
		start_time: std::time::Instant,
	) -> QueryResult<FileSearchOutput> {
		use crate::ops::search::ephemeral_search::{search_ephemeral_index, search_every_index};

		let cache = context.ephemeral_cache();
		let registry = context.file_type_registry();

		let page = match &self.input.scope {
			SearchScope::Path { path } => {
				search_ephemeral_index(
					&self.input.query,
					path,
					&self.input.filters,
					&self.input.sort,
					&self.input.pagination,
					&context,
					cache,
					registry,
				)
				.await?
			}
			SearchScope::Library => {
				search_every_index(
					&self.input.query,
					&self.input.filters,
					&self.input.sort,
					&self.input.pagination,
					&context,
					cache,
					registry,
				)
				.await?
			}
		};

		let execution_time = start_time.elapsed().as_millis() as u64;

		Ok(FileSearchOutput::new_ephemeral(
			page,
			search_id,
			execution_time,
			&self.input.pagination,
		))
	}
}

crate::register_library_query!(FileSearchQuery, "search.files");
