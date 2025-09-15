//! Input for search semantic tags action

use crate::domain::semantic_tag::TagType;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchTagsInput {
    /// Search query (searches across all name variants)
    pub query: String,
    
    /// Optional namespace filter
    pub namespace: Option<String>,
    
    /// Optional tag type filter
    pub tag_type: Option<TagType>,
    
    /// Whether to include archived/hidden tags
    pub include_archived: Option<bool>,
    
    /// Maximum number of results to return
    pub limit: Option<usize>,
    
    /// Whether to resolve ambiguous results using context
    pub resolve_ambiguous: Option<bool>,
    
    /// Context tags for disambiguation (UUIDs)
    pub context_tag_ids: Option<Vec<uuid::Uuid>>,
}

impl SearchTagsInput {
    /// Create a simple search input
    pub fn simple(query: String) -> Self {
        Self {
            query,
            namespace: None,
            tag_type: None,
            include_archived: Some(false),
            limit: Some(50),
            resolve_ambiguous: Some(false),
            context_tag_ids: None,
        }
    }
    
    /// Create a search with namespace filter
    pub fn in_namespace(query: String, namespace: String) -> Self {
        Self {
            query,
            namespace: Some(namespace),
            tag_type: None,
            include_archived: Some(false),
            limit: Some(50),
            resolve_ambiguous: Some(false),
            context_tag_ids: None,
        }
    }
    
    /// Create a context-aware search for disambiguation
    pub fn with_context(query: String, context_tag_ids: Vec<uuid::Uuid>) -> Self {
        Self {
            query,
            namespace: None,
            tag_type: None,
            include_archived: Some(false),
            limit: Some(10),
            resolve_ambiguous: Some(true),
            context_tag_ids: Some(context_tag_ids),
        }
    }
    
    /// Validate the input
    pub fn validate(&self) -> Result<(), String> {
        if self.query.trim().is_empty() {
            return Err("query cannot be empty".to_string());
        }
        
        if self.query.len() > 1000 {
            return Err("query cannot exceed 1000 characters".to_string());
        }
        
        if let Some(limit) = self.limit {
            if limit == 0 {
                return Err("limit must be greater than 0".to_string());
            }
            if limit > 1000 {
                return Err("limit cannot exceed 1000".to_string());
            }
        }
        
        if let Some(namespace) = &self.namespace {
            if namespace.trim().is_empty() {
                return Err("namespace cannot be empty if provided".to_string());
            }
        }
        
        Ok(())
    }
}