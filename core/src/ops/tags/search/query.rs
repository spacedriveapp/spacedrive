//! Search tag definitions across staging and every local store.

use super::{input::SearchTagsInput, output::SearchTagsOutput};
use crate::{
	context::CoreContext,
	domain::Tag,
	infra::query::{LibraryQuery, QueryError, QueryResult},
	ops::tags::definitions,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchTagsQuery {
	input: SearchTagsInput,
}

impl LibraryQuery for SearchTagsQuery {
	type Input = SearchTagsInput;
	type Output = SearchTagsOutput;

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

		let needle = self.input.query.trim().to_lowercase();
		let mut tags: Vec<Tag> = definitions::all(&library, &cache)
			.await
			.iter()
			.filter(|definition| {
				needle.is_empty() || definition.path.to_lowercase().contains(&needle)
			})
			.map(Tag::from_definition)
			.collect();

		if let Some(limit) = self.input.limit {
			tags.truncate(limit as usize);
		}

		Ok(SearchTagsOutput { tags })
	}
}

crate::register_library_query!(SearchTagsQuery, "tags.search");
