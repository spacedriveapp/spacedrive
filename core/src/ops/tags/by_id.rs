//! One tag by uuid.

use crate::{
	context::CoreContext,
	domain::Tag,
	infra::query::{LibraryQuery, QueryError, QueryResult},
	ops::tags::definitions,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetTagByIdQuery {
	input: GetTagByIdInput,
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
		let cache = context.volume_index();

		let tag = definitions::find_one(&library, &cache, self.input.tag_id)
			.await
			.map(|definition| Tag::from_definition(&definition));

		Ok(GetTagByIdOutput { tag })
	}
}

crate::register_library_query!(GetTagByIdQuery, "tags.by_id");
