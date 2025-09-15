# Semantic Tagging Architecture Implementation

## Overview

This document outlines the implementation of the advanced semantic tagging system described in the Spacedrive whitepaper. The system transforms tags from simple labels into a semantic fabric that captures nuanced relationships in personal data organization.

## Key Features to Implement

### 1. Graph-Based DAG Structure
- Directed Acyclic Graph (DAG) for tag relationships
- Closure table for efficient hierarchy traversal
- Support for multiple inheritance paths

### 2. Contextual Tag Design
- **Polymorphic Naming**: Multiple "Project" tags differentiated by semantic context
- **Unicode-Native**: Full international character support
- **Semantic Variants**: Formal names, abbreviations, contextual aliases

### 3. Advanced Tag Capabilities
- **Organizational Roles**: Tags marked as organizational anchors
- **Privacy Controls**: Archive-style tags for search filtering
- **Visual Semantics**: Customizable appearance properties
- **Compositional Attributes**: Complex attribute composition

### 4. Context Resolution
- Intelligent disambiguation through relationship analysis
- Automatic contextual display based on semantic graph position
- Emergent pattern recognition

## Database Schema Enhancement

### Current Schema Issues
The current implementation stores tags as JSON in `user_metadata.tags` and has a basic `tags` table without relationships. This needs to be completely restructured.

### Proposed Schema

```sql
-- Enhanced tags table with semantic features
CREATE TABLE semantic_tags (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    uuid BLOB UNIQUE NOT NULL,
    
    -- Core identity
    canonical_name TEXT NOT NULL,              -- Primary name for this tag
    display_name TEXT,                         -- How it appears in UI (can be context-dependent)
    
    -- Semantic variants
    formal_name TEXT,                          -- Official/formal name
    abbreviation TEXT,                         -- Short form (e.g., "JS" for "JavaScript")
    aliases JSON,                              -- Array of alternative names
    
    -- Context and categorization
    namespace TEXT,                            -- Context namespace (e.g., "Geography", "Technology")
    tag_type TEXT NOT NULL DEFAULT 'standard', -- standard, organizational, privacy, system
    
    -- Visual and behavioral properties
    color TEXT,                                -- Hex color
    icon TEXT,                                 -- Icon identifier
    description TEXT,                          -- Optional description
    
    -- Advanced capabilities
    is_organizational_anchor BOOLEAN DEFAULT FALSE,   -- Creates visual hierarchies
    privacy_level TEXT DEFAULT 'normal',             -- normal, archive, hidden
    search_weight INTEGER DEFAULT 100,               -- Influence in search results
    
    -- Compositional attributes
    attributes JSON,                           -- Key-value pairs for complex attributes
    composition_rules JSON,                    -- Rules for attribute composition
    
    -- Metadata
    created_at TIMESTAMP NOT NULL,
    updated_at TIMESTAMP NOT NULL,
    created_by_device UUID,
    
    -- Constraints
    UNIQUE(canonical_name, namespace)          -- Allow same name in different contexts
);

-- Tag hierarchy using adjacency list + closure table
CREATE TABLE tag_relationships (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    parent_tag_id INTEGER NOT NULL,
    child_tag_id INTEGER NOT NULL,
    relationship_type TEXT NOT NULL DEFAULT 'parent_child', -- parent_child, synonym, related
    strength REAL DEFAULT 1.0,                -- Relationship strength (0.0-1.0)
    created_at TIMESTAMP NOT NULL,
    
    FOREIGN KEY (parent_tag_id) REFERENCES semantic_tags(id) ON DELETE CASCADE,
    FOREIGN KEY (child_tag_id) REFERENCES semantic_tags(id) ON DELETE CASCADE,
    
    -- Prevent cycles and duplicate relationships
    UNIQUE(parent_tag_id, child_tag_id, relationship_type),
    CHECK(parent_tag_id != child_tag_id)
);

-- Closure table for efficient hierarchy traversal
CREATE TABLE tag_closure (
    ancestor_id INTEGER NOT NULL,
    descendant_id INTEGER NOT NULL,
    depth INTEGER NOT NULL,
    path_strength REAL DEFAULT 1.0,           -- Aggregate strength of path
    
    PRIMARY KEY (ancestor_id, descendant_id),
    FOREIGN KEY (ancestor_id) REFERENCES semantic_tags(id) ON DELETE CASCADE,
    FOREIGN KEY (descendant_id) REFERENCES semantic_tags(id) ON DELETE CASCADE
);

-- Enhanced user metadata tagging
CREATE TABLE user_metadata_semantic_tags (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    user_metadata_id INTEGER NOT NULL,
    tag_id INTEGER NOT NULL,
    
    -- Context for this specific tagging instance
    applied_context TEXT,                     -- Context when tag was applied
    applied_variant TEXT,                     -- Which variant name was used
    confidence REAL DEFAULT 1.0,             -- Confidence level (for AI-applied tags)
    source TEXT DEFAULT 'user',              -- user, ai, import, sync
    
    -- Compositional attributes for this specific application
    instance_attributes JSON,                 -- Attributes specific to this tagging
    
    -- Sync and audit
    created_at TIMESTAMP NOT NULL,
    updated_at TIMESTAMP NOT NULL,
    device_uuid UUID NOT NULL,
    
    FOREIGN KEY (user_metadata_id) REFERENCES user_metadata(id) ON DELETE CASCADE,
    FOREIGN KEY (tag_id) REFERENCES semantic_tags(id) ON DELETE CASCADE,
    
    UNIQUE(user_metadata_id, tag_id)
);

-- Tag usage analytics for context resolution
CREATE TABLE tag_usage_patterns (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    tag_id INTEGER NOT NULL,
    co_occurrence_tag_id INTEGER NOT NULL,
    occurrence_count INTEGER DEFAULT 1,
    last_used_together TIMESTAMP NOT NULL,
    
    FOREIGN KEY (tag_id) REFERENCES semantic_tags(id) ON DELETE CASCADE,
    FOREIGN KEY (co_occurrence_tag_id) REFERENCES semantic_tags(id) ON DELETE CASCADE,
    
    UNIQUE(tag_id, co_occurrence_tag_id)
);

-- Indexes for performance
CREATE INDEX idx_semantic_tags_namespace ON semantic_tags(namespace);
CREATE INDEX idx_semantic_tags_canonical_name ON semantic_tags(canonical_name);
CREATE INDEX idx_semantic_tags_type ON semantic_tags(tag_type);

CREATE INDEX idx_tag_closure_ancestor ON tag_closure(ancestor_id);
CREATE INDEX idx_tag_closure_descendant ON tag_closure(descendant_id);
CREATE INDEX idx_tag_closure_depth ON tag_closure(depth);

CREATE INDEX idx_user_metadata_tags_metadata ON user_metadata_semantic_tags(user_metadata_id);
CREATE INDEX idx_user_metadata_tags_tag ON user_metadata_semantic_tags(tag_id);
CREATE INDEX idx_user_metadata_tags_source ON user_metadata_semantic_tags(source);

-- Full-text search support for tag discovery
CREATE VIRTUAL TABLE tag_search_fts USING fts5(
    tag_id,
    canonical_name,
    display_name,
    formal_name,
    abbreviation,
    aliases,
    description,
    namespace,
    content='semantic_tags',
    content_rowid='id'
);
```

## Rust Domain Models

```rust
use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use uuid::Uuid;
use std::collections::HashMap;

/// A semantic tag with advanced capabilities
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SemanticTag {
    pub id: Uuid,
    
    // Core identity
    pub canonical_name: String,
    pub display_name: Option<String>,
    
    // Semantic variants
    pub formal_name: Option<String>,
    pub abbreviation: Option<String>,
    pub aliases: Vec<String>,
    
    // Context
    pub namespace: Option<String>,
    pub tag_type: TagType,
    
    // Visual properties
    pub color: Option<String>,
    pub icon: Option<String>,
    pub description: Option<String>,
    
    // Advanced capabilities
    pub is_organizational_anchor: bool,
    pub privacy_level: PrivacyLevel,
    pub search_weight: i32,
    
    // Compositional attributes
    pub attributes: HashMap<String, serde_json::Value>,
    pub composition_rules: Vec<CompositionRule>,
    
    // Relationships
    pub parents: Vec<TagRelationship>,
    pub children: Vec<TagRelationship>,
    
    // Metadata
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub created_by_device: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TagType {
    Standard,
    Organizational,    // Creates visual hierarchies
    Privacy,          // Controls visibility
    System,           // System-generated
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PrivacyLevel {
    Normal,           // Standard visibility
    Archive,          // Hidden from normal searches but accessible
    Hidden,           // Completely hidden from UI
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TagRelationship {
    pub tag_id: Uuid,
    pub relationship_type: RelationshipType,
    pub strength: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RelationshipType {
    ParentChild,
    Synonym,
    Related,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompositionRule {
    pub operator: CompositionOperator,
    pub operands: Vec<String>,
    pub result_attribute: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CompositionOperator {
    And,
    Or,
    With,
    Without,
}

/// Context-aware tag application
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TagApplication {
    pub tag_id: Uuid,
    pub applied_context: Option<String>,
    pub applied_variant: Option<String>,
    pub confidence: f32,
    pub source: TagSource,
    pub instance_attributes: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TagSource {
    User,
    AI,
    Import,
    Sync,
}
```

## Core Implementation Components

### 1. Tag Context Resolution Engine

```rust
/// Resolves tag ambiguity through context analysis
pub struct TagContextResolver {
    tag_service: Arc<TagService>,
    usage_analyzer: Arc<TagUsageAnalyzer>,
}

impl TagContextResolver {
    /// Resolve which "Phoenix" tag is meant based on context
    pub async fn resolve_ambiguous_tag(
        &self,
        tag_name: &str,
        context_tags: &[SemanticTag],
        user_metadata: &UserMetadata,
    ) -> Result<Vec<SemanticTag>, TagError> {
        // 1. Find all tags with this name
        let candidates = self.tag_service.find_tags_by_name(tag_name).await?;
        
        if candidates.len() <= 1 {
            return Ok(candidates);
        }
        
        // 2. Analyze context
        let mut scored_candidates = Vec::new();
        
        for candidate in candidates {
            let mut score = 0.0;
            
            // Check namespace compatibility with existing tags
            if let Some(namespace) = &candidate.namespace {
                for context_tag in context_tags {
                    if context_tag.namespace.as_ref() == Some(namespace) {
                        score += 0.5;
                    }
                }
            }
            
            // Check usage patterns
            let usage_score = self.usage_analyzer
                .calculate_co_occurrence_score(&candidate, context_tags)
                .await?;
            score += usage_score;
            
            // Check hierarchical relationships
            let hierarchy_score = self.calculate_hierarchy_compatibility(
                &candidate,
                context_tags
            ).await?;
            score += hierarchy_score;
            
            scored_candidates.push((candidate, score));
        }
        
        // Sort by score and return best matches
        scored_candidates.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
        Ok(scored_candidates.into_iter().map(|(tag, _)| tag).collect())
    }
}
```

### 2. Semantic Discovery Engine

```rust
/// Enables semantic queries across the tag graph
pub struct SemanticDiscoveryEngine {
    tag_service: Arc<TagService>,
    closure_service: Arc<TagClosureService>,
}

impl SemanticDiscoveryEngine {
    /// Find all content tagged with descendants of "Corporate Materials"
    pub async fn find_descendant_tagged_entries(
        &self,
        ancestor_tag: &str,
        entry_service: &EntryService,
    ) -> Result<Vec<Entry>, TagError> {
        // 1. Find the ancestor tag
        let ancestor = self.tag_service
            .find_tag_by_name(ancestor_tag)
            .await?
            .ok_or(TagError::TagNotFound)?;
        
        // 2. Get all descendant tags using closure table
        let descendants = self.closure_service
            .get_all_descendants(ancestor.id)
            .await?;
        
        // 3. Include the ancestor itself
        let mut all_tags = descendants;
        all_tags.push(ancestor);
        
        // 4. Find all entries tagged with any of these tags
        let tagged_entries = entry_service
            .find_entries_by_tags(&all_tags)
            .await?;
        
        Ok(tagged_entries)
    }
    
    /// Discover emergent organizational patterns
    pub async fn discover_patterns(
        &self,
        user_metadata_service: &UserMetadataService,
    ) -> Result<Vec<OrganizationalPattern>, TagError> {
        let usage_patterns = self.tag_service
            .get_tag_usage_patterns()
            .await?;
        
        let mut discovered_patterns = Vec::new();
        
        // Analyze frequently co-occurring tags
        for pattern in usage_patterns {
            if pattern.occurrence_count > 10 {
                let relationship_suggestion = self.suggest_relationship(
                    &pattern.tag_id,
                    &pattern.co_occurrence_tag_id
                ).await?;
                
                if let Some(suggestion) = relationship_suggestion {
                    discovered_patterns.push(suggestion);
                }
            }
        }
        
        Ok(discovered_patterns)
    }
}
```

### 3. Union Merge Conflict Resolution

```rust
/// Handles tag conflict resolution during sync
pub struct TagConflictResolver;

impl TagConflictResolver {
    /// Merge tags using union strategy
    pub fn merge_tag_applications(
        &self,
        local_tags: Vec<TagApplication>,
        remote_tags: Vec<TagApplication>,
    ) -> Result<TagMergeResult, TagError> {
        let mut merged_tags = HashMap::new();
        let mut conflicts = Vec::new();
        
        // Add all local tags
        for tag_app in local_tags {
            merged_tags.insert(tag_app.tag_id, tag_app);
        }
        
        // Union merge with remote tags
        for remote_tag in remote_tags {
            match merged_tags.get(&remote_tag.tag_id) {
                Some(local_tag) => {
                    // Tag exists locally - check for attribute conflicts
                    if local_tag.instance_attributes != remote_tag.instance_attributes {
                        // Merge attributes intelligently
                        let merged_attributes = self.merge_attributes(
                            &local_tag.instance_attributes,
                            &remote_tag.instance_attributes,
                        )?;
                        
                        let mut merged_tag = local_tag.clone();
                        merged_tag.instance_attributes = merged_attributes;
                        merged_tags.insert(remote_tag.tag_id, merged_tag);
                    }
                }
                None => {
                    // New remote tag - add it
                    merged_tags.insert(remote_tag.tag_id, remote_tag);
                }
            }
        }
        
        Ok(TagMergeResult {
            merged_tags: merged_tags.into_values().collect(),
            conflicts,
            merge_summary: self.generate_merge_summary(&merged_tags),
        })
    }
    
    fn merge_attributes(
        &self,
        local: &HashMap<String, serde_json::Value>,
        remote: &HashMap<String, serde_json::Value>,
    ) -> Result<HashMap<String, serde_json::Value>, TagError> {
        let mut merged = local.clone();
        
        for (key, remote_value) in remote {
            match merged.get(key) {
                Some(local_value) if local_value != remote_value => {
                    // Conflict - use conflict resolution strategy
                    merged.insert(
                        key.clone(),
                        self.resolve_attribute_conflict(local_value, remote_value)?
                    );
                }
                None => {
                    // New attribute from remote
                    merged.insert(key.clone(), remote_value.clone());
                }
                _ => {
                    // Same value, no conflict
                }
            }
        }
        
        Ok(merged)
    }
}
```

## Implementation Phases

### Phase 1: Database Migration and Core Models
- [ ] Create migration to transform current tag schema
- [ ] Implement enhanced SemanticTag domain model
- [ ] Build TagService with CRUD operations
- [ ] Create closure table maintenance system

### Phase 2: Context Resolution System
- [ ] Implement TagContextResolver
- [ ] Build usage pattern tracking
- [ ] Create semantic disambiguation logic
- [ ] Add namespace-based context grouping

### Phase 3: Advanced Features
- [ ] Organizational anchor functionality
- [ ] Privacy level controls
- [ ] Visual semantic properties
- [ ] Compositional attribute system

### Phase 4: Discovery and Intelligence
- [ ] Semantic discovery engine
- [ ] Pattern recognition system
- [ ] Emergent relationship suggestions
- [ ] Full-text search integration

### Phase 5: Sync Integration
- [ ] Union merge conflict resolution
- [ ] Tag-specific sync domain handling
- [ ] Cross-device context preservation
- [ ] Audit trail for tag operations

## Implementation Strategy

This is a clean implementation of the semantic tagging architecture that creates an entirely new system:

1. **Fresh Start**: Creates new semantic tagging tables alongside existing simple tags
2. **No Migration**: No data migration from the old system is required
3. **Progressive Adoption**: Users can start using semantic tags immediately
4. **Gradual Feature Rollout**: Advanced features can be enabled as they're implemented
5. **Performance Optimized**: Built with proper indexing and closure table from day one

This implementation transforms Spacedrive's tagging from a basic labeling system into a sophisticated semantic fabric that truly captures the nuanced relationships in personal data organization.