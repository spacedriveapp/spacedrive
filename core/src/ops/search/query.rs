//! File search query implementation

use super::{input::FileSearchInput, output::FileSearchOutput};
use crate::{
    context::CoreContext,
    cqrs::Query,
    domain::Entry,
    infra::db::entities::entry,
    filetype::FileTypeRegistry,
};
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;
use sea_orm::{DatabaseConnection, EntityTrait, QueryFilter, QuerySelect, QueryOrder, Condition, ColumnTrait};
use chrono::{DateTime, Utc};

/// File search query
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileSearchQuery {
    pub input: FileSearchInput,
}

impl FileSearchQuery {
    pub fn new(input: FileSearchInput) -> Self {
        Self { input }
    }
}

impl Query for FileSearchQuery {
    type Output = FileSearchOutput;

    async fn execute(self, context: Arc<CoreContext>) -> Result<Self::Output> {
        let start_time = std::time::Instant::now();
        
        // Validate input
        self.input.validate().map_err(|e| anyhow::anyhow!("Invalid search input: {}", e))?;
        
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
        let search_id = Uuid::new_v4();
        
        // Perform the search based on mode
        let results = match self.input.mode {
            crate::ops::search::input::SearchMode::Fast => {
                self.execute_fast_search(db.conn()).await?
            }
            crate::ops::search::input::SearchMode::Normal => {
                self.execute_normal_search(db.conn()).await?
            }
            crate::ops::search::input::SearchMode::Full => {
                self.execute_full_search(db.conn()).await?
            }
        };

        let execution_time = start_time.elapsed().as_millis() as u64;
        
        // Create output
        let total_count = results.len() as u64;
        let output = FileSearchOutput::success(
            results,
            total_count, // TODO: Get actual total count
            search_id,
            execution_time,
        );

        Ok(output)
    }
}

impl FileSearchQuery {
    /// Execute fast search using FTS5
    async fn execute_fast_search(&self, db: &DatabaseConnection) -> Result<Vec<crate::ops::search::output::FileSearchResult>> {
        // For now, implement basic SQL LIKE search
        // TODO: Implement FTS5 integration
        let query = format!("%{}%", self.input.query);
        
        let mut condition = Condition::any()
            .add(entry::Column::Name.contains(&self.input.query))
            .add(entry::Column::Extension.contains(&self.input.query));
        
        // Apply scope filters
        condition = self.apply_scope_filter(condition);
        
        // Get file type registry for content type filtering
        let registry = FileTypeRegistry::new();
        
        // Apply additional filters
        condition = self.apply_filters(condition, &registry);
        
        let entries = entry::Entity::find()
            .filter(condition)
            .filter(entry::Column::Kind.eq(0)) // Only files
            .limit(self.input.pagination.limit as u64)
            .offset(self.input.pagination.offset as u64)
            .all(db)
            .await?;
        
        // Convert to search results
        let results = entries.into_iter().map(|entry_model| {
            // Convert database model to domain Entry
            let entry = Entry {
                id: entry_model.uuid.unwrap_or_else(|| Uuid::new_v4()),
                sd_path: crate::domain::SdPathSerialized {
                    device_id: Uuid::new_v4(), // TODO: Get from device table
                    path: format!("/path/to/{}", entry_model.name), // TODO: Proper path construction
                },
                name: entry_model.name,
                kind: match entry_model.kind {
                    0 => crate::domain::entry::EntryKind::File {
                        extension: entry_model.extension,
                    },
                    1 => crate::domain::entry::EntryKind::Directory,
                    2 => crate::domain::entry::EntryKind::Symlink {
                        target: "".to_string(), // TODO: Get from database
                    },
                    _ => crate::domain::entry::EntryKind::File {
                        extension: entry_model.extension,
                    },
                },
                size: Some(entry_model.size as u64),
                created_at: Some(entry_model.created_at),
                modified_at: Some(entry_model.modified_at),
                accessed_at: entry_model.accessed_at,
                inode: entry_model.inode.map(|i| i as u64),
                file_id: None,
                parent_id: entry_model.parent_id.map(|_| Uuid::new_v4()), // TODO: Proper conversion
                location_id: None, // TODO: Get from location table
                metadata_id: Uuid::new_v4(), // TODO: Proper conversion
                content_id: entry_model.content_id.map(|_| Uuid::new_v4()), // TODO: Proper conversion
                first_seen_at: entry_model.created_at,
                last_indexed_at: Some(entry_model.created_at),
            };
            
            crate::ops::search::output::FileSearchResult {
                entry,
                score: 1.0, // TODO: Calculate actual relevance score
                score_breakdown: crate::ops::search::output::ScoreBreakdown::new(
                    1.0, // temporal_score
                    None, // semantic_score
                    0.0, // metadata_score
                    0.0, // recency_boost
                    0.0, // user_preference_boost
                ),
                highlights: Vec::new(),
                matched_content: None,
            }
        }).collect();
        
        Ok(results)
    }
    
    /// Execute normal search with basic ranking
    async fn execute_normal_search(&self, db: &DatabaseConnection) -> Result<Vec<crate::ops::search::output::FileSearchResult>> {
        // For now, same as fast search
        // TODO: Add semantic ranking
        self.execute_fast_search(db).await
    }
    
    /// Execute full search with content analysis
    async fn execute_full_search(&self, db: &DatabaseConnection) -> Result<Vec<crate::ops::search::output::FileSearchResult>> {
        // For now, same as normal search
        // TODO: Add content extraction and analysis
        self.execute_normal_search(db).await
    }
    
    /// Apply scope filters to the query condition
    fn apply_scope_filter(&self, mut condition: Condition) -> Condition {
        match &self.input.scope {
            crate::ops::search::input::SearchScope::Library => {
                // No additional filtering needed for library-wide search
                condition
            }
            crate::ops::search::input::SearchScope::Location { location_id } => {
                // TODO: Add location filtering when location_id is available in entry table
                condition
            }
            crate::ops::search::input::SearchScope::Path { path } => {
                // TODO: Implement path-based filtering using directory closure table
                condition
            }
        }
    }
    
    /// Apply additional filters to the query condition
    fn apply_filters(&self, mut condition: Condition, registry: &FileTypeRegistry) -> Condition {
        // File type filter
        if let Some(file_types) = &self.input.filters.file_types {
            if !file_types.is_empty() {
                let mut file_type_condition = Condition::any();
                for file_type in file_types {
                    file_type_condition = file_type_condition.add(entry::Column::Extension.eq(file_type));
                }
                condition = condition.add(file_type_condition);
            }
        }
        
        // Date range filter
        if let Some(date_range) = &self.input.filters.date_range {
            let date_column = match date_range.field {
                crate::ops::search::input::DateField::CreatedAt => entry::Column::CreatedAt,
                crate::ops::search::input::DateField::ModifiedAt => entry::Column::ModifiedAt,
                crate::ops::search::input::DateField::AccessedAt => entry::Column::AccessedAt,
            };
            
            if let Some(start) = date_range.start {
                condition = condition.add(date_column.gte(start));
            }
            if let Some(end) = date_range.end {
                condition = condition.add(date_column.lte(end));
            }
        }
        
        // Size range filter
        if let Some(size_range) = &self.input.filters.size_range {
            if let Some(min) = size_range.min {
                condition = condition.add(entry::Column::Size.gte(min as i64));
            }
            if let Some(max) = size_range.max {
                condition = condition.add(entry::Column::Size.lte(max as i64));
            }
        }
        
        // Content type filter using file type registry
        if let Some(content_types) = &self.input.filters.content_types {
            if !content_types.is_empty() {
                let mut content_condition = Condition::any();
                for content_type in content_types {
                    let extensions = registry.get_extensions_for_category(*content_type);
                    for extension in extensions {
                        content_condition = content_condition.add(entry::Column::Extension.eq(extension));
                    }
                }
                condition = condition.add(content_condition);
            }
        }
        
        // Location filter
        if let Some(locations) = &self.input.filters.locations {
            if !locations.is_empty() {
                // TODO: Add location filtering when location_id is available in entry table
                // let mut location_condition = Condition::any();
                // for location_id in locations {
                //     location_condition = location_condition.add(entry::Column::LocationId.eq(*location_id));
                // }
                // condition = condition.add(location_condition);
            }
        }
        
        // Include hidden filter
        if let Some(include_hidden) = self.input.filters.include_hidden {
            if !include_hidden {
                // TODO: Add hidden field to entry table
                // condition = condition.add(entry::Column::Hidden.eq(false));
            }
        }
        
        condition
    }
}

crate::register_query!(FileSearchQuery, "search.files");