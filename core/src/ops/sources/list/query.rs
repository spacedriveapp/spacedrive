//! Source listing query implementation

use super::output::SourceInfo;
use crate::{
	context::CoreContext,
	infra::query::{LibraryQuery, QueryError, QueryResult},
};
use serde::{Deserialize, Serialize};
use specta::Type;
use std::sync::Arc;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct ListSourcesInput {
	/// Filter by data type
	pub data_type: Option<String>,
}

/// Query to list all sources in the active library
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct ListSourcesQuery {
	pub input: ListSourcesInput,
}

impl ListSourcesQuery {
	pub fn all() -> Self {
		Self {
			input: ListSourcesInput { data_type: None },
		}
	}
}

impl LibraryQuery for ListSourcesQuery {
	type Input = ListSourcesInput;
	type Output = Vec<SourceInfo>;

	fn from_input(input: Self::Input) -> QueryResult<Self> {
		Ok(Self { input })
	}

	async fn execute(
		self,
		context: Arc<CoreContext>,
		session: crate::infra::api::SessionContext,
	) -> QueryResult<Self::Output> {
		// Get the active library from session
		let library_id = session
			.current_library_id
			.ok_or_else(|| QueryError::Internal("No library in session".to_string()))?;
		let library = context
			.libraries()
			.await
			.get_library(library_id)
			.await
			.ok_or_else(|| QueryError::Internal("Library not found".to_string()))?;

		// Get or initialize the source manager
		if library.source_manager().is_none() {
			library
				.init_source_manager()
				.await
				.map_err(|e| QueryError::Internal(format!("Failed to init source manager: {e}")))?;
		}

		let source_manager = library
			.source_manager()
			.ok_or_else(|| QueryError::Internal("Source manager not available".to_string()))?;

		// List sources via sd-archive
		let sources = source_manager
			.list_sources()
			.await
			.map_err(|e| QueryError::Internal(format!("Failed to list sources: {e}")))?;

		let mut result = Vec::new();

		for source in sources {
			// Apply data type filter if specified
			if let Some(ref filter) = self.input.data_type {
				if &source.data_type != filter {
					continue;
				}
			}

			let id = Uuid::parse_str(&source.id)
				.map_err(|e| QueryError::Internal(format!("Invalid source ID: {e}")))?;

			result.push(SourceInfo::new(
				id,
				source.name,
				source.data_type,
				source.adapter_id,
				source.item_count,
				source.last_synced,
				source.status,
			));
		}

		Ok(result)
	}
}

// Register library-scoped query
crate::register_library_query!(ListSourcesQuery, "sources.list");
