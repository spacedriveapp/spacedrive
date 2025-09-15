//! Semantic Tag domain model
//!
//! Implementation of the advanced semantic tagging architecture described in the whitepaper.
//! This replaces the simple tag model with a sophisticated graph-based system that supports
//! polymorphic naming, contextual resolution, and compositional attributes.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// A tag with advanced capabilities for contextual organization
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Tag {
    /// Unique identifier
    pub id: Uuid,

    /// Core identity
    pub canonical_name: String,
    pub display_name: Option<String>,

    /// Semantic variants for flexible access
    pub formal_name: Option<String>,
    pub abbreviation: Option<String>,
    pub aliases: Vec<String>,

    /// Context and categorization
    pub namespace: Option<String>,
    pub tag_type: TagType,

    /// Visual and behavioral properties
    pub color: Option<String>,
    pub icon: Option<String>,
    pub description: Option<String>,

    /// Advanced capabilities
    pub is_organizational_anchor: bool,
    pub privacy_level: PrivacyLevel,
    pub search_weight: i32,

    /// Compositional attributes
    pub attributes: HashMap<String, serde_json::Value>,
    pub composition_rules: Vec<CompositionRule>,

    /// Metadata
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub created_by_device: Uuid,
}

/// Types of semantic tags with different behaviors
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum TagType {
    /// Standard user-created tag
    Standard,
    /// Creates visual hierarchies in the interface
    Organizational,
    /// Controls search and display visibility
    Privacy,
    /// System-generated tag (AI, import, etc.)
    System,
}

impl TagType {
    pub fn as_str(&self) -> &'static str {
        match self {
            TagType::Standard => "standard",
            TagType::Organizational => "organizational",
            TagType::Privacy => "privacy",
            TagType::System => "system",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "standard" => Some(TagType::Standard),
            "organizational" => Some(TagType::Organizational),
            "privacy" => Some(TagType::Privacy),
            "system" => Some(TagType::System),
            _ => None,
        }
    }
}

/// Privacy levels for tag visibility control
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum PrivacyLevel {
    /// Standard visibility in all contexts
    Normal,
    /// Hidden from normal searches but accessible via direct query
    Archive,
    /// Completely hidden from standard UI
    Hidden,
}

impl PrivacyLevel {
    pub fn as_str(&self) -> &'static str {
        match self {
            PrivacyLevel::Normal => "normal",
            PrivacyLevel::Archive => "archive",
            PrivacyLevel::Hidden => "hidden",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "normal" => Some(PrivacyLevel::Normal),
            "archive" => Some(PrivacyLevel::Archive),
            "hidden" => Some(PrivacyLevel::Hidden),
            _ => None,
        }
    }
}

/// Relationship between two tags in the semantic graph
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TagRelationship {
    pub related_tag_id: Uuid,
    pub relationship_type: RelationshipType,
    pub strength: f32,
    pub created_at: DateTime<Utc>,
}

/// Types of relationships between tags
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum RelationshipType {
    /// Hierarchical parent-child relationship
    ParentChild,
    /// Synonym or alias relationship
    Synonym,
    /// General semantic relatedness
    Related,
}

impl RelationshipType {
    pub fn as_str(&self) -> &'static str {
        match self {
            RelationshipType::ParentChild => "parent_child",
            RelationshipType::Synonym => "synonym",
            RelationshipType::Related => "related",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "parent_child" => Some(RelationshipType::ParentChild),
            "synonym" => Some(RelationshipType::Synonym),
            "related" => Some(RelationshipType::Related),
            _ => None,
        }
    }
}

/// Rules for composing attributes from multiple tags
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CompositionRule {
    pub operator: CompositionOperator,
    pub operands: Vec<String>,
    pub result_attribute: String,
}

/// Operators for combining tag attributes
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum CompositionOperator {
    /// All conditions must be true
    And,
    /// Any condition must be true
    Or,
    /// Must have this property
    With,
    /// Must not have this property
    Without,
}

/// Context-aware application of a tag to content
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TagApplication {
    pub tag_id: Uuid,
    /// Context when the tag was applied (e.g., "geography", "technology")
    pub applied_context: Option<String>,
    /// Which variant name was used when applying
    pub applied_variant: Option<String>,
    /// Confidence level (0.0-1.0, useful for AI-applied tags)
    pub confidence: f32,
    /// Source of the tag application
    pub source: TagSource,
    /// Attributes specific to this particular application
    pub instance_attributes: HashMap<String, serde_json::Value>,
    /// When this application was created
    pub created_at: DateTime<Utc>,
    /// Which device applied this tag
    pub device_uuid: Uuid,
}

/// Source of tag application
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum TagSource {
    /// Manually applied by user
    User,
    /// Applied by AI analysis
    AI,
    /// Imported from external source
    Import,
    /// Synchronized from another device
    Sync,
}

/// Result of merging tag applications during sync
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TagMergeResult {
    pub merged_applications: Vec<TagApplication>,
    pub conflicts: Vec<TagConflict>,
    pub merge_summary: String,
}

/// Conflict that occurred during tag merging
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TagConflict {
    pub tag_id: Uuid,
    pub conflict_type: ConflictType,
    pub local_value: serde_json::Value,
    pub remote_value: serde_json::Value,
    pub resolution: ConflictResolution,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ConflictType {
    AttributeValue,
    Context,
    Confidence,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ConflictResolution {
    UseLocal,
    UseRemote,
    Merge,
    RequiresUserInput,
}

/// Pattern discovered through usage analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrganizationalPattern {
    pub pattern_type: PatternType,
    pub tags_involved: Vec<Uuid>,
    pub confidence: f32,
    pub suggestion: String,
    pub discovered_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PatternType {
    FrequentCoOccurrence,
    HierarchicalRelationship,
    SemanticSimilarity,
    ContextualGrouping,
}

impl Tag {
    /// Create a new semantic tag with default values
    pub fn new(canonical_name: String, created_by_device: Uuid) -> Self {
        let now = Utc::now();

        Self {
            id: Uuid::new_v4(),
            canonical_name: canonical_name.clone(),
            display_name: None,
            formal_name: None,
            abbreviation: None,
            aliases: Vec::new(),
            namespace: None,
            tag_type: TagType::Standard,
            color: None,
            icon: None,
            description: None,
            is_organizational_anchor: false,
            privacy_level: PrivacyLevel::Normal,
            search_weight: 100,
            attributes: HashMap::new(),
            composition_rules: Vec::new(),
            created_at: now,
            updated_at: now,
            created_by_device,
        }
    }

    /// Get the best display name for this tag in the given context
    pub fn get_display_name(&self, context: Option<&str>) -> &str {
        // If we have a context-specific display name, use it
        if let Some(display) = &self.display_name {
            return display;
        }

        // Otherwise use canonical name
        &self.canonical_name
    }

    /// Get all possible names this tag can be accessed by
    pub fn get_all_names(&self) -> Vec<&str> {
        let mut names = vec![self.canonical_name.as_str()];

        if let Some(formal) = &self.formal_name {
            names.push(formal);
        }

        if let Some(abbrev) = &self.abbreviation {
            names.push(abbrev);
        }

        for alias in &self.aliases {
            names.push(alias);
        }

        names
    }

    /// Check if this tag matches the given name in any variant
    pub fn matches_name(&self, name: &str) -> bool {
        self.get_all_names().iter().any(|&n| n.eq_ignore_ascii_case(name))
    }

    /// Add an alias to this tag
    pub fn add_alias(&mut self, alias: String) {
        if !self.aliases.contains(&alias) {
            self.aliases.push(alias);
            self.updated_at = Utc::now();
        }
    }

    /// Set an attribute value
    pub fn set_attribute<T: Serialize>(&mut self, key: String, value: T) -> Result<(), serde_json::Error> {
        let json_value = serde_json::to_value(value)?;
        self.attributes.insert(key, json_value);
        self.updated_at = Utc::now();
        Ok(())
    }

    /// Get an attribute value
    pub fn get_attribute<T: for<'de> Deserialize<'de>>(&self, key: &str) -> Result<Option<T>, serde_json::Error> {
        match self.attributes.get(key) {
            Some(value) => Ok(Some(serde_json::from_value(value.clone())?)),
            None => Ok(None),
        }
    }

    /// Check if this tag should be hidden from normal search results
    pub fn is_searchable(&self) -> bool {
        match self.privacy_level {
            PrivacyLevel::Normal => true,
            PrivacyLevel::Archive | PrivacyLevel::Hidden => false,
        }
    }

    /// Get the fully qualified name including namespace
    pub fn get_qualified_name(&self) -> String {
        match &self.namespace {
            Some(ns) => format!("{}::{}", ns, self.canonical_name),
            None => self.canonical_name.clone(),
        }
    }
}

impl TagApplication {
    /// Create a new tag application
    pub fn new(
        tag_id: Uuid,
        source: TagSource,
        device_uuid: Uuid,
    ) -> Self {
        Self {
            tag_id,
            applied_context: None,
            applied_variant: None,
            confidence: 1.0,
            source,
            instance_attributes: HashMap::new(),
            created_at: Utc::now(),
            device_uuid,
        }
    }

    /// Create a user-applied tag application
    pub fn user_applied(tag_id: Uuid, device_uuid: Uuid) -> Self {
        Self::new(tag_id, TagSource::User, device_uuid)
    }

    /// Create an AI-applied tag application with confidence
    pub fn ai_applied(tag_id: Uuid, confidence: f32, device_uuid: Uuid) -> Self {
        let mut app = Self::new(tag_id, TagSource::AI, device_uuid);
        app.confidence = confidence;
        app
    }

    /// Set an instance-specific attribute
    pub fn set_instance_attribute<T: Serialize>(&mut self, key: String, value: T) -> Result<(), serde_json::Error> {
        let json_value = serde_json::to_value(value)?;
        self.instance_attributes.insert(key, json_value);
        Ok(())
    }

    /// Check if this application has high confidence
    pub fn is_high_confidence(&self) -> bool {
        self.confidence >= 0.8
    }
}

/// Error types for semantic tag operations
#[derive(Debug, thiserror::Error)]
pub enum TagError {
    #[error("Tag not found")]
    TagNotFound,

    #[error("Invalid tag relationship: {0}")]
    InvalidRelationship(String),

    #[error("Circular reference detected")]
    CircularReference,

    #[error("Conflicting tag names in namespace: {0}")]
    NameConflict(String),

    #[error("Invalid composition rule: {0}")]
    InvalidCompositionRule(String),

    #[error("Serialization error: {0}")]
    SerializationError(#[from] serde_json::Error),

    #[error("Database error: {0}")]
    DatabaseError(String),
}