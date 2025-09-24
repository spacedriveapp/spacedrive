//! Output for apply semantic tags action

use serde::{Deserialize, Serialize};
use specta::Type;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct ApplyTagsOutput {
    /// Number of entries that had tags applied
    pub entries_affected: usize,

    /// Number of tags that were applied
    pub tags_applied: usize,

    /// Tag IDs that were successfully applied
    pub applied_tag_ids: Vec<Uuid>,

    /// Entry IDs that were successfully tagged
    pub tagged_entry_ids: Vec<i32>,

    /// Any warnings or notes about the operation
    pub warnings: Vec<String>,

    /// Success message
    pub message: String,
}

impl ApplyTagsOutput {
    /// Create a successful output
    pub fn success(
        entries_affected: usize,
        tags_applied: usize,
        applied_tag_ids: Vec<Uuid>,
        tagged_entry_ids: Vec<i32>,
    ) -> Self {
        let message = format!(
            "Successfully applied {} tag(s) to {} entry/entries",
            tags_applied,
            entries_affected
        );

        Self {
            entries_affected,
            tags_applied,
            applied_tag_ids,
            tagged_entry_ids,
            warnings: Vec::new(),
            message,
        }
    }

    /// Add a warning to the output
    pub fn with_warning(mut self, warning: String) -> Self {
        self.warnings.push(warning);
        self
    }

    /// Add multiple warnings to the output
    pub fn with_warnings(mut self, warnings: Vec<String>) -> Self {
        self.warnings.extend(warnings);
        self
    }
}