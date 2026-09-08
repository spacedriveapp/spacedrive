//! Full-record listing for a source: every facet field, as JSON rows.
//!
//! Presentation surfaces that need more than the search projection consume
//! this — a photo grid reads original_path/thumb_path/captured_at from the
//! same rows an inspector reads camera fields from. The shape follows the
//! adapter's declared model, so the op stays adapter-agnostic.

use crate::{
	context::CoreContext,
	infra::query::{LibraryQuery, QueryError, QueryResult},
	ops::sources::registry,
};
use serde::{Deserialize, Serialize};
use specta::Type;
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct ListSourceRecordsInput {
	pub source_id: String,
	pub limit: u32,
	pub offset: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListSourceRecordsQuery {
	pub input: ListSourceRecordsInput,
}

impl LibraryQuery for ListSourceRecordsQuery {
	type Input = ListSourceRecordsInput;
	type Output = Vec<serde_json::Value>;

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

		let store_id = registry::parse_store_id(&self.input.source_id)
			.map_err(|e| QueryError::Internal(format!("{e}")))?;

		source_manager
			.list_records_full(
				&store_id,
				(self.input.limit as usize).min(2000),
				self.input.offset as usize,
			)
			.await
			.map_err(QueryError::Internal)
	}
}

crate::register_library_query!(ListSourceRecordsQuery, "sources.list_records");
