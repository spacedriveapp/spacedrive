//! Semantic Tag Service
//!
//! Core service for managing the semantic tagging architecture.
//! Provides high-level operations for tag creation, hierarchy management,
//! context resolution, and conflict resolution during sync.

use crate::domain::semantic_tag::{
    SemanticTag, TagApplication, TagRelationship, RelationshipType, TagError,
    TagMergeResult, OrganizationalPattern, PatternType, TagType, PrivacyLevel,
};
use crate::infra::db::DbPool;
use anyhow::Result;
use chrono::{DateTime, Utc};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use uuid::Uuid;

/// Service for managing semantic tags and their relationships
#[derive(Clone)]
pub struct SemanticTagService {
    db: Arc<DbPool>,
    context_resolver: Arc<TagContextResolver>,
    usage_analyzer: Arc<TagUsageAnalyzer>,
    closure_service: Arc<TagClosureService>,
}

impl SemanticTagService {
    pub fn new(db: Arc<DbPool>) -> Self {
        let context_resolver = Arc::new(TagContextResolver::new(db.clone()));
        let usage_analyzer = Arc::new(TagUsageAnalyzer::new(db.clone()));
        let closure_service = Arc::new(TagClosureService::new(db.clone()));
        
        Self {
            db,
            context_resolver,
            usage_analyzer,
            closure_service,
        }
    }
    
    /// Create a new semantic tag
    pub async fn create_tag(
        &self,
        canonical_name: String,
        namespace: Option<String>,
        created_by_device: Uuid,
    ) -> Result<SemanticTag, TagError> {
        // Check for name conflicts in the same namespace
        if let Some(existing) = self.find_tag_by_name_and_namespace(&canonical_name, namespace.as_deref()).await? {
            return Err(TagError::NameConflict(format!(
                "Tag '{}' already exists in namespace '{:?}'",
                canonical_name, namespace
            )));
        }
        
        let mut tag = SemanticTag::new(canonical_name, created_by_device);
        tag.namespace = namespace;
        
        // TODO: Insert into database
        // self.db.insert_semantic_tag(&tag).await?;
        
        Ok(tag)
    }
    
    /// Find a tag by its canonical name and namespace
    pub async fn find_tag_by_name_and_namespace(
        &self,
        name: &str,
        namespace: Option<&str>,
    ) -> Result<Option<SemanticTag>, TagError> {
        // TODO: Implement database query
        // self.db.find_semantic_tag_by_name_and_namespace(name, namespace).await
        Ok(None)
    }
    
    /// Find all tags matching a name (across all namespaces)
    pub async fn find_tags_by_name(&self, name: &str) -> Result<Vec<SemanticTag>, TagError> {
        // TODO: Implement database query including aliases
        // This should search canonical_name, formal_name, abbreviation, and aliases
        Ok(Vec::new())
    }
    
    /// Resolve ambiguous tag names using context
    pub async fn resolve_ambiguous_tag(
        &self,
        tag_name: &str,
        context_tags: &[SemanticTag],
    ) -> Result<Vec<SemanticTag>, TagError> {
        self.context_resolver.resolve_ambiguous_tag(tag_name, context_tags).await
    }
    
    /// Create a relationship between two tags
    pub async fn create_relationship(
        &self,
        parent_id: Uuid,
        child_id: Uuid,
        relationship_type: RelationshipType,
        strength: Option<f32>,
    ) -> Result<(), TagError> {
        // Check for circular references
        if self.would_create_cycle(parent_id, child_id).await? {
            return Err(TagError::CircularReference);
        }
        
        let strength = strength.unwrap_or(1.0);
        
        // TODO: Insert relationship into database
        // self.db.create_tag_relationship(parent_id, child_id, relationship_type, strength).await?;
        
        // Update closure table if this is a parent-child relationship
        if relationship_type == RelationshipType::ParentChild {
            self.closure_service.add_relationship(parent_id, child_id).await?;
        }
        
        Ok(())
    }
    
    /// Check if adding a relationship would create a cycle
    async fn would_create_cycle(&self, parent_id: Uuid, child_id: Uuid) -> Result<bool, TagError> {
        // If child_id is an ancestor of parent_id, adding this relationship would create a cycle
        let ancestors = self.closure_service.get_all_ancestors(parent_id).await?;
        Ok(ancestors.contains(&child_id))
    }
    
    /// Get all tags that are descendants of the given tag
    pub async fn get_descendants(&self, tag_id: Uuid) -> Result<Vec<SemanticTag>, TagError> {
        let descendant_ids = self.closure_service.get_all_descendants(tag_id).await?;
        self.get_tags_by_ids(&descendant_ids).await
    }
    
    /// Get all tags that are ancestors of the given tag
    pub async fn get_ancestors(&self, tag_id: Uuid) -> Result<Vec<SemanticTag>, TagError> {
        let ancestor_ids = self.closure_service.get_all_ancestors(tag_id).await?;
        self.get_tags_by_ids(&ancestor_ids).await
    }
    
    /// Get tags by their IDs
    async fn get_tags_by_ids(&self, tag_ids: &[Uuid]) -> Result<Vec<SemanticTag>, TagError> {
        // TODO: Implement batch lookup
        Ok(Vec::new())
    }
    
    /// Apply semantic discovery to find organizational patterns
    pub async fn discover_organizational_patterns(&self) -> Result<Vec<OrganizationalPattern>, TagError> {
        let mut patterns = Vec::new();
        
        // Analyze tag co-occurrence patterns
        let usage_patterns = self.usage_analyzer.get_frequent_co_occurrences(10).await?;
        
        for (tag1_id, tag2_id, count) in usage_patterns {
            // Check if these tags should be related
            if count > 5 && !self.are_tags_related(tag1_id, tag2_id).await? {
                patterns.push(OrganizationalPattern {
                    pattern_type: PatternType::FrequentCoOccurrence,
                    tags_involved: vec![tag1_id, tag2_id],
                    confidence: (count as f32) / 100.0,
                    suggestion: format!("Consider creating a relationship between tags that frequently appear together"),
                    discovered_at: Utc::now(),
                });
            }
        }
        
        // TODO: Add more pattern discovery algorithms
        // - Hierarchical relationship detection
        // - Semantic similarity analysis
        // - Contextual grouping analysis
        
        Ok(patterns)
    }
    
    /// Check if two tags are already related
    async fn are_tags_related(&self, tag1_id: Uuid, tag2_id: Uuid) -> Result<bool, TagError> {
        // TODO: Check if tags have any relationship
        Ok(false)
    }
    
    /// Merge tag applications during sync (union merge strategy)
    pub async fn merge_tag_applications(
        &self,
        local_applications: Vec<TagApplication>,
        remote_applications: Vec<TagApplication>,
    ) -> Result<TagMergeResult, TagError> {
        let resolver = TagConflictResolver::new();
        resolver.merge_tag_applications(local_applications, remote_applications).await
    }
    
    /// Search for tags using various criteria
    pub async fn search_tags(
        &self,
        query: &str,
        namespace_filter: Option<&str>,
        tag_type_filter: Option<TagType>,
        include_archived: bool,
    ) -> Result<Vec<SemanticTag>, TagError> {
        // TODO: Implement full-text search across all tag fields
        // Use the FTS5 virtual table for efficient text search
        Ok(Vec::new())
    }
    
    /// Update tag usage statistics
    pub async fn record_tag_usage(
        &self,
        tag_applications: &[TagApplication],
    ) -> Result<(), TagError> {
        self.usage_analyzer.record_usage_patterns(tag_applications).await
    }
}

/// Resolves tag context and disambiguation
pub struct TagContextResolver {
    db: Arc<DbPool>,
}

impl TagContextResolver {
    pub fn new(db: Arc<DbPool>) -> Self {
        Self { db }
    }
    
    /// Resolve which version of an ambiguous tag name is intended
    pub async fn resolve_ambiguous_tag(
        &self,
        tag_name: &str,
        context_tags: &[SemanticTag],
    ) -> Result<Vec<SemanticTag>, TagError> {
        // Find all possible tags with this name
        let candidates = self.find_all_name_matches(tag_name).await?;
        
        if candidates.len() <= 1 {
            return Ok(candidates);
        }
        
        // Score candidates based on context compatibility
        let mut scored_candidates = Vec::new();
        
        for candidate in candidates {
            let mut score = 0.0;
            
            // 1. Namespace compatibility
            score += self.calculate_namespace_compatibility(&candidate, context_tags).await?;
            
            // 2. Usage pattern compatibility
            score += self.calculate_usage_compatibility(&candidate, context_tags).await?;
            
            // 3. Hierarchical relationship compatibility
            score += self.calculate_hierarchy_compatibility(&candidate, context_tags).await?;
            
            scored_candidates.push((candidate, score));
        }
        
        // Sort by score and return ranked results
        scored_candidates.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        
        Ok(scored_candidates.into_iter().map(|(tag, _)| tag).collect())
    }
    
    async fn find_all_name_matches(&self, name: &str) -> Result<Vec<SemanticTag>, TagError> {
        // TODO: Search canonical_name, formal_name, abbreviation, and aliases
        Ok(Vec::new())
    }
    
    async fn calculate_namespace_compatibility(
        &self,
        candidate: &SemanticTag,
        context_tags: &[SemanticTag],
    ) -> Result<f32, TagError> {
        let mut score = 0.0;
        
        if let Some(candidate_namespace) = &candidate.namespace {
            let matching_namespaces = context_tags
                .iter()
                .filter_map(|t| t.namespace.as_ref())
                .filter(|ns| *ns == candidate_namespace)
                .count();
            
            score = (matching_namespaces as f32) / (context_tags.len() as f32);
        }
        
        Ok(score * 0.5) // Weight namespace compatibility
    }
    
    async fn calculate_usage_compatibility(
        &self,
        candidate: &SemanticTag,
        context_tags: &[SemanticTag],
    ) -> Result<f32, TagError> {
        // TODO: Calculate based on historical co-occurrence patterns
        Ok(0.0)
    }
    
    async fn calculate_hierarchy_compatibility(
        &self,
        candidate: &SemanticTag,
        context_tags: &[SemanticTag],
    ) -> Result<f32, TagError> {
        // TODO: Calculate based on shared ancestors/descendants
        Ok(0.0)
    }
}

/// Analyzes tag usage patterns for intelligent suggestions
pub struct TagUsageAnalyzer {
    db: Arc<DbPool>,
}

impl TagUsageAnalyzer {
    pub fn new(db: Arc<DbPool>) -> Self {
        Self { db }
    }
    
    /// Record co-occurrence patterns when tags are applied together
    pub async fn record_usage_patterns(
        &self,
        tag_applications: &[TagApplication],
    ) -> Result<(), TagError> {
        // Record co-occurrence between all pairs of tags in this application set
        for (i, app1) in tag_applications.iter().enumerate() {
            for app2 in tag_applications.iter().skip(i + 1) {
                // TODO: Increment co-occurrence count in tag_usage_patterns table
                // self.db.increment_co_occurrence(app1.tag_id, app2.tag_id).await?;
            }
        }
        
        Ok(())
    }
    
    /// Get frequently co-occurring tag pairs
    pub async fn get_frequent_co_occurrences(
        &self,
        min_count: i32,
    ) -> Result<Vec<(Uuid, Uuid, i32)>, TagError> {
        // TODO: Query tag_usage_patterns table for frequent co-occurrences
        Ok(Vec::new())
    }
    
    /// Calculate co-occurrence score between a tag and a set of context tags
    pub async fn calculate_co_occurrence_score(
        &self,
        candidate: &SemanticTag,
        context_tags: &[SemanticTag],
    ) -> Result<f32, TagError> {
        let mut total_score = 0.0;
        let mut count = 0;
        
        for context_tag in context_tags {
            if let Some(co_occurrence_count) = self.get_co_occurrence_count(candidate.id, context_tag.id).await? {
                total_score += co_occurrence_count as f32;
                count += 1;
            }
        }
        
        if count > 0 {
            Ok((total_score / count as f32) / 100.0) // Normalize to 0-1 range
        } else {
            Ok(0.0)
        }
    }
    
    async fn get_co_occurrence_count(
        &self,
        tag1_id: Uuid,
        tag2_id: Uuid,
    ) -> Result<Option<i32>, TagError> {
        // TODO: Query tag_usage_patterns table
        Ok(None)
    }
}

/// Manages the closure table for efficient hierarchy queries
pub struct TagClosureService {
    db: Arc<DbPool>,
}

impl TagClosureService {
    pub fn new(db: Arc<DbPool>) -> Self {
        Self { db }
    }
    
    /// Add a new parent-child relationship and update closure table
    pub async fn add_relationship(
        &self,
        parent_id: Uuid,
        child_id: Uuid,
    ) -> Result<(), TagError> {
        // TODO: Update closure table with new relationship
        // This involves:
        // 1. Adding direct relationship (depth = 1)
        // 2. Adding transitive relationships through existing ancestors/descendants
        Ok(())
    }
    
    /// Remove a relationship and update closure table
    pub async fn remove_relationship(
        &self,
        parent_id: Uuid,
        child_id: Uuid,
    ) -> Result<(), TagError> {
        // TODO: Remove relationship and recalculate affected closure paths
        Ok(())
    }
    
    /// Get all descendant tag IDs
    pub async fn get_all_descendants(&self, ancestor_id: Uuid) -> Result<Vec<Uuid>, TagError> {
        // TODO: Query closure table for all descendants
        Ok(Vec::new())
    }
    
    /// Get all ancestor tag IDs
    pub async fn get_all_ancestors(&self, descendant_id: Uuid) -> Result<Vec<Uuid>, TagError> {
        // TODO: Query closure table for all ancestors
        Ok(Vec::new())
    }
    
    /// Get direct children only
    pub async fn get_direct_children(&self, parent_id: Uuid) -> Result<Vec<Uuid>, TagError> {
        // TODO: Query closure table with depth = 1
        Ok(Vec::new())
    }
    
    /// Get path between two tags
    pub async fn get_path_between(
        &self,
        from_tag_id: Uuid,
        to_tag_id: Uuid,
    ) -> Result<Option<Vec<Uuid>>, TagError> {
        // TODO: Find shortest path between tags in the hierarchy
        Ok(None)
    }
}

/// Handles conflict resolution during tag synchronization
pub struct TagConflictResolver;

impl TagConflictResolver {
    pub fn new() -> Self {
        Self
    }
    
    /// Merge tag applications using union merge strategy
    pub async fn merge_tag_applications(
        &self,
        local_applications: Vec<TagApplication>,
        remote_applications: Vec<TagApplication>,
    ) -> Result<TagMergeResult, TagError> {
        let mut merged_tags = HashMap::new();
        let mut conflicts = Vec::new();
        
        // Add all local applications
        for app in local_applications {
            merged_tags.insert(app.tag_id, app);
        }
        
        // Union merge with remote applications
        for remote_app in remote_applications {
            match merged_tags.get(&remote_app.tag_id) {
                Some(local_app) => {
                    // Tag exists locally - merge intelligently
                    let merged_app = self.merge_single_application(local_app, &remote_app)?;
                    merged_tags.insert(remote_app.tag_id, merged_app);
                }
                None => {
                    // New remote tag - add it
                    merged_tags.insert(remote_app.tag_id, remote_app);
                }
            }
        }
        
        let merge_summary = format!(
            "Merged {} tag applications with {} conflicts",
            merged_tags.len(),
            conflicts.len()
        );
        
        Ok(TagMergeResult {
            merged_applications: merged_tags.into_values().collect(),
            conflicts,
            merge_summary,
        })
    }
    
    fn merge_single_application(
        &self,
        local: &TagApplication,
        remote: &TagApplication,
    ) -> Result<TagApplication, TagError> {
        let mut merged = local.clone();
        
        // Use higher confidence value
        if remote.confidence > local.confidence {
            merged.confidence = remote.confidence;
        }
        
        // Merge instance attributes (union merge)
        for (key, value) in &remote.instance_attributes {
            if !merged.instance_attributes.contains_key(key) {
                merged.instance_attributes.insert(key.clone(), value.clone());
            }
        }
        
        // Prefer remote context if local doesn't have one
        if merged.applied_context.is_none() && remote.applied_context.is_some() {
            merged.applied_context = remote.applied_context.clone();
        }
        
        Ok(merged)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_semantic_tag_creation() {
        let device_id = Uuid::new_v4();
        let tag = SemanticTag::new("test-tag".to_string(), device_id);
        
        assert_eq!(tag.canonical_name, "test-tag");
        assert_eq!(tag.created_by_device, device_id);
        assert_eq!(tag.tag_type, TagType::Standard);
        assert_eq!(tag.privacy_level, PrivacyLevel::Normal);
    }
    
    #[test]
    fn test_tag_name_matching() {
        let device_id = Uuid::new_v4();
        let mut tag = SemanticTag::new("JavaScript".to_string(), device_id);
        tag.formal_name = Some("JavaScript Programming Language".to_string());
        tag.abbreviation = Some("JS".to_string());
        tag.add_alias("ECMAScript".to_string());
        
        assert!(tag.matches_name("JavaScript"));
        assert!(tag.matches_name("js")); // Case insensitive
        assert!(tag.matches_name("ECMAScript"));
        assert!(tag.matches_name("JavaScript Programming Language"));
        assert!(!tag.matches_name("Python"));
    }
    
    #[test]
    fn test_tag_application_creation() {
        let tag_id = Uuid::new_v4();
        let device_id = Uuid::new_v4();
        
        let user_app = TagApplication::user_applied(tag_id, device_id);
        assert_eq!(user_app.source, TagSource::User);
        assert_eq!(user_app.confidence, 1.0);
        
        let ai_app = TagApplication::ai_applied(tag_id, 0.85, device_id);
        assert_eq!(ai_app.source, TagSource::AI);
        assert_eq!(ai_app.confidence, 0.85);
        assert!(ai_app.is_high_confidence());
    }
}