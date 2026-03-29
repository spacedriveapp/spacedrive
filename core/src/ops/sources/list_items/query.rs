//! Source items listing query implementation

use crate::{
	context::CoreContext,
	infra::query::{LibraryQuery, QueryError, QueryResult},
};
use serde::{Deserialize, Serialize};
use specta::Type;
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct ListSourceItemsInput {
	pub source_id: String,
	pub limit: u32,
	pub offset: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct SourceItem {
	pub id: String,
	pub external_id: String,
	pub title: String,
	pub preview: Option<String>,
	pub subtitle: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListSourceItemsQuery {
	pub input: ListSourceItemsInput,
}

impl LibraryQuery for ListSourceItemsQuery {
	type Input = ListSourceItemsInput;
	type Output = Vec<SourceItem>;

	fn from_input(input: Self::Input) -> QueryResult<Self> {
		if input.source_id.trim().is_empty() {
			return Err(QueryError::Validation {
				field: "source_id".to_string(),
				message: "source_id cannot be empty".to_string(),
			});
		}
		Ok(Self { input })
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

		if library.source_manager().is_none() {
			library
				.init_source_manager()
				.await
				.map_err(|e| QueryError::Internal(format!("Failed to init source manager: {e}")))?;
		}

		let source_manager = library
			.source_manager()
			.ok_or_else(|| QueryError::Internal("Source manager not available".to_string()))?;

		let items = source_manager
			.list_items(
				&self.input.source_id,
				self.input.limit as usize,
				self.input.offset as usize,
			)
			.await
			.map_err(|e| QueryError::Internal(e))?;

		Ok(items
			.into_iter()
			.map(|item| SourceItem {
				id: item.id,
				external_id: item.external_id,
				title: item.title,
				preview: item.preview,
				subtitle: item.subtitle,
			})
			.collect())
	}
}

crate::register_library_query!(ListSourceItemsQuery, "sources.list_items");
