//! Get tag ancestors query

use crate::{
	context::CoreContext,
	domain::tag::Tag,
	infra::query::{LibraryQuery, QueryError, QueryResult},
	ops::tags::manager::TagManager,
};
use serde::{Deserialize, Serialize};
use specta::Type;
use std::sync::Arc;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct GetTagAncestorsInput {
	pub tag_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct GetTagAncestorsOutput {
	pub ancestors: Vec<Tag>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct GetTagAncestorsQuery {
	pub input: GetTagAncestorsInput,
}

impl LibraryQuery for GetTagAncestorsQuery {
	type Input = GetTagAncestorsInput;
	type Output = GetTagAncestorsOutput;

	fn from_input(input: Self::Input) -> QueryResult<Self> {
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

		let db = library.db();
		let manager = TagManager::new(Arc::new(db.conn().clone()));

		let ancestors = manager
			.get_ancestors(self.input.tag_id)
			.await
			.map_err(|e| QueryError::Internal(format!("Ancestor lookup failed: {}", e)))?;

		Ok(GetTagAncestorsOutput { ancestors })
	}
}

crate::register_library_query!(GetTagAncestorsQuery, "tags.ancestors");
