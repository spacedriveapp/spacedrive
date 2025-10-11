//! Search semantic tags query

use super::{input::SearchTagsInput, output::SearchTagsOutput};
use crate::infra::query::{QueryError, QueryResult};
use crate::{context::CoreContext, infra::query::LibraryQuery, ops::tags::manager::TagManager};
use serde::{Deserialize, Serialize};
use specta::Type;
use std::sync::Arc;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct SearchTagsQuery {
	pub input: SearchTagsInput,
}

impl SearchTagsQuery {
	pub fn new(input: SearchTagsInput) -> Self {
		Self { input }
	}
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

		let db = library.db();
		let semantic_tag_manager = TagManager::new(Arc::new(db.conn().clone()));

		let include_archived = self.input.include_archived.unwrap_or(false);

		// Perform the search
		let mut search_results = semantic_tag_manager
			.search_tags(
				&self.input.query,
				self.input.namespace.as_deref(),
				self.input.tag_type.clone(),
				include_archived,
			)
			.await
			.map_err(|e| QueryError::Internal(format!("Tag search failed: {}", e.to_string())))?;

		let mut disambiguated = false;

		// Apply context resolution if requested and context tags provided
		if self.input.resolve_ambiguous.unwrap_or(false) {
			if let Some(context_tag_ids) = &self.input.context_tag_ids {
				if !context_tag_ids.is_empty() {
					// Get context tags
					let context_tags = semantic_tag_manager
						.get_tags_by_ids(context_tag_ids)
						.await
						.map_err(|e| {
							QueryError::Internal(format!(
								"Failed to get context tags: {}",
								e.to_string()
							))
						})?;

					// Resolve ambiguous results
					search_results = semantic_tag_manager
						.resolve_ambiguous_tag(&self.input.query, &context_tags)
						.await
						.map_err(|e| {
							QueryError::Internal(format!(
								"Context resolution failed: {}",
								e.to_string()
							))
						})?;

					disambiguated = true;
				}
			}
		}

		// Apply limit if specified
		if let Some(limit) = self.input.limit {
			search_results.truncate(limit);
		}

		// Create output
		let output = SearchTagsOutput::success(
			search_results,
			self.input.query.clone(),
			self.input.namespace.clone(),
			self.input.tag_type.as_ref().map(|t| t.as_str().to_string()),
			include_archived,
			self.input.limit,
			disambiguated,
		);

		Ok(output)
	}
}

crate::register_library_query!(SearchTagsQuery, "tags.search");
