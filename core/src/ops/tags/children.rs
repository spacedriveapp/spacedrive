//! A tag's direct children, derived from paths.

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
pub struct GetTagChildrenInput {
	/// The parent tag. `None` lists root tags: definitions whose path has a
	/// single segment.
	pub tag_id: Option<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct GetTagChildrenOutput {
	pub tags: Vec<Tag>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetTagChildrenQuery {
	input: GetTagChildrenInput,
}

impl LibraryQuery for GetTagChildrenQuery {
	type Input = GetTagChildrenInput;
	type Output = GetTagChildrenOutput;

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
		let cache = context.ephemeral_cache();

		let all = definitions::all(&library, &cache).await;

		let tags = match self.input.tag_id {
			None => all
				.iter()
				.filter(|d| !d.path.contains('/'))
				.map(Tag::from_definition)
				.collect(),
			Some(parent_id) => {
				let Some(parent) = all.iter().find(|d| d.uuid == parent_id) else {
					return Ok(GetTagChildrenOutput { tags: Vec::new() });
				};
				let prefix = format!("{}/", parent.path);
				all.iter()
					.filter(|d| {
						d.path
							.strip_prefix(&prefix)
							.is_some_and(|rest| !rest.is_empty() && !rest.contains('/'))
					})
					.map(Tag::from_definition)
					.collect()
			}
		};

		Ok(GetTagChildrenOutput { tags })
	}
}

crate::register_library_query!(GetTagChildrenQuery, "tags.children");
