//! Get tag by ID query

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
pub struct GetTagByIdInput {
	pub tag_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct GetTagByIdOutput {
	pub tag: Option<Tag>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct GetTagByIdQuery {
	pub input: GetTagByIdInput,
}

impl LibraryQuery for GetTagByIdQuery {
	type Input = GetTagByIdInput;
	type Output = GetTagByIdOutput;

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

		let tags = manager
			.get_tags_by_ids(&[self.input.tag_id])
			.await
			.map_err(|e| QueryError::Internal(format!("Tag lookup failed: {}", e)))?;

		Ok(GetTagByIdOutput {
			tag: tags.into_iter().next(),
		})
	}
}

crate::register_library_query!(GetTagByIdQuery, "tags.by_id");
