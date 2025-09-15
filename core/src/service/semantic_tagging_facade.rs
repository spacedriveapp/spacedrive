//! Semantic Tagging Facade
//!
//! High-level convenience API for semantic tagging operations.
//! This facade simplifies common tagging workflows and provides a clean
//! interface for UI and CLI integration.

use crate::{
    domain::semantic_tag::{SemanticTag, TagApplication, TagType, PrivacyLevel, RelationshipType, TagSource, TagError},
    service::{
        semantic_tag_service::SemanticTagService,
        user_metadata_service::UserMetadataService,
    },
    infra::db::Database,
};
use std::collections::HashMap;
use std::sync::Arc;
use uuid::Uuid;

/// High-level facade for semantic tagging operations
#[derive(Clone)]
pub struct SemanticTaggingFacade {
    tag_service: Arc<SemanticTagService>,
    metadata_service: Arc<UserMetadataService>,
}

impl SemanticTaggingFacade {
    pub fn new(db: Arc<Database>) -> Self {
        let db_conn = Arc::new(db.conn().clone());
        let tag_service = Arc::new(SemanticTagService::new(db_conn.clone()));
        let metadata_service = Arc::new(UserMetadataService::new(db_conn));

        Self {
            tag_service,
            metadata_service,
        }
    }

    /// Create a simple tag (most common use case)
    pub async fn create_simple_tag(
        &self,
        name: String,
        color: Option<String>,
        device_id: Uuid,
    ) -> Result<SemanticTag, TagError> {
        self.tag_service.create_tag(name, None, device_id).await
    }

    /// Create a tag with namespace (for disambiguation)
    pub async fn create_namespaced_tag(
        &self,
        name: String,
        namespace: String,
        color: Option<String>,
        device_id: Uuid,
    ) -> Result<SemanticTag, TagError> {
        let mut tag = self.tag_service.create_tag(name, Some(namespace), device_id).await?;
        if let Some(color) = color {
            tag.color = Some(color);
            // TODO: Update tag in database with color
        }
        Ok(tag)
    }

    /// Create an organizational tag (creates visual hierarchies)
    pub async fn create_organizational_tag(
        &self,
        name: String,
        color: Option<String>,
        device_id: Uuid,
    ) -> Result<SemanticTag, TagError> {
        let mut tag = self.tag_service.create_tag(name, None, device_id).await?;
        tag.tag_type = TagType::Organizational;
        tag.is_organizational_anchor = true;
        if let Some(color) = color {
            tag.color = Some(color);
        }
        // TODO: Update tag in database with type and anchor status
        Ok(tag)
    }

    /// Create a tag with semantic variants (JavaScript/JS/ECMAScript)
    pub async fn create_tag_with_variants(
        &self,
        canonical_name: String,
        abbreviation: Option<String>,
        aliases: Vec<String>,
        namespace: Option<String>,
        device_id: Uuid,
    ) -> Result<SemanticTag, TagError> {
        let mut tag = self.tag_service.create_tag(canonical_name, namespace, device_id).await?;

        if let Some(abbrev) = abbreviation {
            tag.abbreviation = Some(abbrev);
        }

        for alias in aliases {
            tag.add_alias(alias);
        }

        // TODO: Update tag in database with variants
        Ok(tag)
    }

    /// Build a tag hierarchy (Technology → Programming → Web Development)
    pub async fn create_tag_hierarchy(
        &self,
        hierarchy: Vec<(String, Option<String>)>, // (name, namespace) pairs
        device_id: Uuid,
    ) -> Result<Vec<SemanticTag>, TagError> {
        let mut created_tags = Vec::new();

        // Create all tags first
        for (name, namespace) in hierarchy {
            let tag = self.tag_service.create_tag(name, namespace, device_id).await?;
            created_tags.push(tag);
        }

        // Create parent-child relationships
        for i in 0..created_tags.len().saturating_sub(1) {
            self.tag_service.create_relationship(
                created_tags[i].id,
                created_tags[i + 1].id,
                RelationshipType::ParentChild,
                None,
            ).await?;
        }

        Ok(created_tags)
    }

    /// Tag a file with user-applied tags (most common use case)
    pub async fn tag_entry(
        &self,
        entry_id: i32,
        tag_names: Vec<String>,
        device_id: Uuid,
    ) -> Result<Vec<Uuid>, TagError> {
        let mut applied_tag_ids = Vec::new();

        // Find or create tags by name
        for tag_name in tag_names {
            let existing_tags = self.tag_service.find_tags_by_name(&tag_name).await?;

            let tag_id = if existing_tags.is_empty() {
                // Create new tag if it doesn't exist
                let new_tag = self.tag_service.create_tag(tag_name, None, device_id).await?;
                new_tag.id
            } else if existing_tags.len() == 1 {
                // Use existing tag if unambiguous
                existing_tags[0].id
            } else {
                // Multiple tags found - use context resolution
                // For now, just use the first one (TODO: implement smarter resolution)
                existing_tags[0].id
            };

            applied_tag_ids.push(tag_id);
        }

        // Apply all tags to the entry
        self.metadata_service.apply_user_semantic_tags(
            entry_id,
            &applied_tag_ids,
            device_id,
        ).await?;

        Ok(applied_tag_ids)
    }

    /// Tag a file with AI suggestions (with confidence scores)
    pub async fn apply_ai_tags(
        &self,
        entry_id: i32,
        ai_suggestions: Vec<(String, f32, String)>, // (tag_name, confidence, context)
        device_id: Uuid,
    ) -> Result<Vec<Uuid>, TagError> {
        let mut tag_suggestions = Vec::new();

        // Find or create tags for AI suggestions
        for (tag_name, confidence, context) in ai_suggestions {
            let existing_tags = self.tag_service.find_tags_by_name(&tag_name).await?;

            let tag_id = if existing_tags.is_empty() {
                // Create new system tag for AI-discovered content
                let mut new_tag = self.tag_service.create_tag(tag_name, None, device_id).await?;
                new_tag.tag_type = TagType::System;
                // TODO: Update tag type in database
                new_tag.id
            } else {
                existing_tags[0].id
            };

            tag_suggestions.push((tag_id, confidence, context));
        }

        // Apply AI tags with confidence scores
        self.metadata_service.apply_ai_semantic_tags(
            entry_id,
            tag_suggestions.clone(),
            device_id,
        ).await?;

        Ok(tag_suggestions.into_iter().map(|(id, _, _)| id).collect())
    }

    /// Smart tag suggestion based on existing patterns
    pub async fn suggest_tags_for_entry(
        &self,
        entry_id: i32,
        max_suggestions: usize,
    ) -> Result<Vec<(SemanticTag, f32)>, TagError> {
        // Get existing tags for this entry
        let existing_applications = self.metadata_service.get_semantic_tags_for_entry(entry_id).await?;
        let existing_tag_ids: Vec<Uuid> = existing_applications.iter().map(|app| app.tag_id).collect();

        if existing_tag_ids.is_empty() {
            return Ok(Vec::new());
        }

        let existing_tags = self.tag_service.get_tags_by_ids(&existing_tag_ids).await?;

        // Find patterns from existing tags
        let patterns = self.tag_service.discover_organizational_patterns().await?;

        let mut suggestions = Vec::new();

        // Simple suggestion logic based on co-occurrence
        for existing_tag in &existing_tags {
            // TODO: Access usage analyzer through public method
            let co_occurrences: Vec<(Uuid, Uuid, i32)> = Vec::new(); // Placeholder

            for (tag1_id, tag2_id, count) in co_occurrences {
                if tag1_id == existing_tag.id && !existing_tag_ids.contains(&tag2_id) {
                    if let Ok(suggested_tags) = self.tag_service.get_tags_by_ids(&[tag2_id]).await {
                        if let Some(suggested_tag) = suggested_tags.first() {
                            let confidence = (count as f32 / 20.0).min(1.0); // Normalize
                            suggestions.push((suggested_tag.clone(), confidence));
                        }
                    }
                }
            }
        }

        // Sort by confidence and limit results
        suggestions.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        suggestions.truncate(max_suggestions);

        Ok(suggestions)
    }

    /// Find files by semantic tags (supports hierarchy)
    pub async fn find_files_by_tags(
        &self,
        tag_names: Vec<String>,
        include_descendants: bool,
    ) -> Result<Vec<i32>, TagError> {
        let mut tag_ids = Vec::new();

        // Resolve tag names to IDs
        for tag_name in tag_names {
            let tags = self.tag_service.find_tags_by_name(&tag_name).await?;
            if let Some(tag) = tags.first() {
                tag_ids.push(tag.id);
            }
        }

        if tag_ids.is_empty() {
            return Ok(Vec::new());
        }

        self.metadata_service.find_entries_by_semantic_tags(&tag_ids, include_descendants).await
    }

    /// Get tag hierarchy for display (organizational anchors first)
    pub async fn get_tag_hierarchy(&self) -> Result<Vec<TagHierarchyNode>, TagError> {
        let all_tags = self.tag_service.search_tags("", None, None, true).await?;

        // Find root tags (organizational anchors without parents)
        let mut hierarchy = Vec::new();

        for tag in &all_tags {
            if tag.is_organizational_anchor {
                let ancestors = self.tag_service.get_ancestors(tag.id).await?;
                if ancestors.is_empty() {
                    // This is a root organizational tag
                    let node = self.build_hierarchy_node(tag, &all_tags).await?;
                    hierarchy.push(node);
                }
            }
        }

        Ok(hierarchy)
    }

    async fn build_hierarchy_node(
        &self,
        tag: &SemanticTag,
        all_tags: &[SemanticTag],
    ) -> Result<TagHierarchyNode, TagError> {
        let descendant_ids = self.tag_service.get_descendants(tag.id).await?;
        let descendant_uuid_ids: Vec<Uuid> = descendant_ids.into_iter().map(|tag| tag.id).collect();
        let descendants = self.tag_service.get_tags_by_ids(&descendant_uuid_ids).await?;

        let children = descendants
            .into_iter()
            .map(|child_tag| TagHierarchyNode {
                tag: child_tag,
                children: Vec::new(), // TODO: Recursive building if needed
            })
            .collect();

        Ok(TagHierarchyNode {
            tag: tag.clone(),
            children,
        })
    }
}

/// Hierarchical representation of tags for UI display
#[derive(Debug, Clone)]
pub struct TagHierarchyNode {
    pub tag: SemanticTag,
    pub children: Vec<TagHierarchyNode>,
}

impl TagHierarchyNode {
    /// Get the depth of this node in the hierarchy
    pub fn depth(&self) -> usize {
        if self.children.is_empty() {
            0
        } else {
            1 + self.children.iter().map(|child| child.depth()).max().unwrap_or(0)
        }
    }

    /// Get all tags in this subtree (flattened)
    pub fn flatten(&self) -> Vec<&SemanticTag> {
        let mut result = vec![&self.tag];
        for child in &self.children {
            result.extend(child.flatten());
        }
        result
    }

    /// Count total tags in this subtree
    pub fn count_tags(&self) -> usize {
        1 + self.children.iter().map(|child| child.count_tags()).sum::<usize>()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hierarchy_node() {
        let device_id = Uuid::new_v4();
        let root_tag = SemanticTag::new("Technology".to_string(), device_id);
        let child_tag = SemanticTag::new("Programming".to_string(), device_id);

        let child_node = TagHierarchyNode {
            tag: child_tag,
            children: Vec::new(),
        };

        let root_node = TagHierarchyNode {
            tag: root_tag,
            children: vec![child_node],
        };

        assert_eq!(root_node.count_tags(), 2);
        assert_eq!(root_node.depth(), 1);
        assert_eq!(root_node.flatten().len(), 2);
    }
}