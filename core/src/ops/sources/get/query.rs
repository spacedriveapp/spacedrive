//! Single source query implementation

use crate::ops::sources::list::SourceInfo;
use crate::{
	context::CoreContext,
	infra::query::{LibraryQuery, QueryError, QueryResult},
	ops::sources::registry,
};
use serde::{Deserialize, Serialize};
use specta::Type;
use std::sync::Arc;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct GetSourceInput {
	pub source_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetSourceQuery {
	pub input: GetSourceInput,
}

impl LibraryQuery for GetSourceQuery {
	type Input = GetSourceInput;
	type Output = SourceInfo;

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

		let source_id = Uuid::parse_str(&self.input.source_id)
			.map_err(|e| QueryError::Internal(format!("Invalid source ID: {e}")))?;

		let row = registry::get(library.db().conn(), source_id)
			.await
			.map_err(|e| QueryError::Internal(format!("{e}")))?;

		Ok(SourceInfo::from_row(row, None))
	}
}

crate::register_library_query!(GetSourceQuery, "sources.get");
