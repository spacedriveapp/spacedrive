//! Search semantic tags action

use super::{input::SearchTagsInput, output::SearchTagsOutput};
use crate::{
    context::CoreContext,
    domain::tag::{Tag, TagType},
    infra::action::{error::ActionError, LibraryAction},
    library::Library,
    ops::tags::manager::TagManager,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchTagsAction {
    input: SearchTagsInput,
}

impl SearchTagsAction {
    pub fn new(input: SearchTagsInput) -> Self {
        Self { input }
    }
}

impl LibraryAction for SearchTagsAction {
    type Input = SearchTagsInput;
    type Output = SearchTagsOutput;

    fn from_input(input: SearchTagsInput) -> Result<Self, String> {
        input.validate()?;
        Ok(SearchTagsAction::new(input))
    }

    async fn execute(
        self,
        library: Arc<Library>,
        _context: Arc<CoreContext>,
    ) -> Result<Self::Output, ActionError> {
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
            .map_err(|e| ActionError::Internal(format!("Tag search failed: {}", e)))?;

        let mut disambiguated = false;

        // Apply context resolution if requested and context tags provided
        if self.input.resolve_ambiguous.unwrap_or(false) {
            if let Some(context_tag_ids) = &self.input.context_tag_ids {
                if !context_tag_ids.is_empty() {
                    // Get context tags
                    let context_tags = semantic_tag_manager
                        .get_tags_by_ids(context_tag_ids)
                        .await
                        .map_err(|e| ActionError::Internal(format!("Failed to get context tags: {}", e)))?;

                    // Resolve ambiguous results
                    search_results = semantic_tag_manager
                        .resolve_ambiguous_tag(&self.input.query, &context_tags)
                        .await
                        .map_err(|e| ActionError::Internal(format!("Context resolution failed: {}", e)))?;

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

    fn action_kind(&self) -> &'static str {
        "tags.search"
    }

    async fn validate(&self, _library: &Arc<Library>, _context: Arc<CoreContext>) -> Result<(), ActionError> {
        self.input.validate().map_err(|msg| ActionError::Validation {
            field: "input".to_string(),
            message: msg,
        })?;

        Ok(())
    }
}

// Register library action
crate::register_library_action!(SearchTagsAction, "tags.search");