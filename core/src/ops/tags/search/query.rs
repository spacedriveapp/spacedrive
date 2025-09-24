//! Search semantic tags query

use super::{input::SearchTagsInput, output::SearchTagsOutput};
use crate::{context::CoreContext, cqrs::Query, ops::tags::manager::TagManager};
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchTagsQuery {
	pub input: SearchTagsInput,
}

impl SearchTagsQuery {
	pub fn new(input: SearchTagsInput) -> Self {
		Self { input }
	}
}

impl Query for SearchTagsQuery {
	type Input = SearchTagsInput;
	type Output = SearchTagsOutput;

	async fn execute(self, context: Arc<CoreContext>) -> Result<Self::Output> {
		// Resolve current library from session
		let session_state = context.session.get().await;
		let library_id = session_state
			.current_library_id
			.ok_or_else(|| anyhow::anyhow!("No active library selected"))?;
		let library = context
			.libraries()
			.await
			.get_library(library_id)
			.await
			.ok_or_else(|| anyhow::anyhow!("Library not found"))?;

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
			.map_err(|e| anyhow::anyhow!("Tag search failed: {}", e))?;

		let mut disambiguated = false;

		// Apply context resolution if requested and context tags provided
		if self.input.resolve_ambiguous.unwrap_or(false) {
			if let Some(context_tag_ids) = &self.input.context_tag_ids {
				if !context_tag_ids.is_empty() {
					// Get context tags
					let context_tags = semantic_tag_manager
						.get_tags_by_ids(context_tag_ids)
						.await
						.map_err(|e| anyhow::anyhow!("Failed to get context tags: {}", e))?;

					// Resolve ambiguous results
					search_results = semantic_tag_manager
						.resolve_ambiguous_tag(&self.input.query, &context_tags)
						.await
						.map_err(|e| anyhow::anyhow!("Context resolution failed: {}", e))?;

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
