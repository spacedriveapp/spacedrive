//! Semantic Tag entity
//!
//! SeaORM entity for the enhanced semantic tagging system

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "semantic_tags")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    pub uuid: Uuid,
    
    // Core identity
    pub canonical_name: String,
    pub display_name: Option<String>,
    
    // Semantic variants
    pub formal_name: Option<String>,
    pub abbreviation: Option<String>,
    pub aliases: Option<Json>,  // Vec<String> as JSON
    
    // Context and categorization
    pub namespace: Option<String>,
    pub tag_type: String,  // TagType enum as string
    
    // Visual and behavioral properties
    pub color: Option<String>,
    pub icon: Option<String>,
    pub description: Option<String>,
    
    // Advanced capabilities
    pub is_organizational_anchor: bool,
    pub privacy_level: String,  // PrivacyLevel enum as string
    pub search_weight: i32,
    
    // Compositional attributes
    pub attributes: Option<Json>,  // HashMap<String, serde_json::Value> as JSON
    pub composition_rules: Option<Json>,  // Vec<CompositionRule> as JSON
    
    // Metadata
    pub created_at: DateTimeUtc,
    pub updated_at: DateTimeUtc,
    pub created_by_device: Option<Uuid>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::tag_relationship::Entity")]
    ParentRelationships,
    
    #[sea_orm(has_many = "super::tag_relationship::Entity")]
    ChildRelationships,
    
    #[sea_orm(has_many = "super::user_metadata_semantic_tag::Entity")]
    UserMetadataSemanticTags,
    
    #[sea_orm(has_many = "super::tag_usage_pattern::Entity")]
    UsagePatterns,
}

impl Related<super::user_metadata_semantic_tag::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::UserMetadataSemanticTags.def()
    }
}

impl Related<super::tag_relationship::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::ParentRelationships.def()
    }
}

impl Related<super::tag_usage_pattern::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::UsagePatterns.def()
    }
}

impl ActiveModelBehavior for ActiveModel {
    fn new() -> Self {
        Self {
            uuid: Set(Uuid::new_v4()),
            tag_type: Set("standard".to_owned()),
            privacy_level: Set("normal".to_owned()),
            search_weight: Set(100),
            is_organizational_anchor: Set(false),
            created_at: Set(chrono::Utc::now()),
            updated_at: Set(chrono::Utc::now()),
            ..ActiveModelTrait::default()
        }
    }
    
    fn before_save<C>(mut self, _db: &C, insert: bool) -> Result<Self, DbErr>
    where
        C: ConnectionTrait,
    {
        if !insert {
            self.updated_at = Set(chrono::Utc::now());
        }
        Ok(self)
    }
}

impl Model {
    /// Get aliases as a vector of strings
    pub fn get_aliases(&self) -> Vec<String> {
        self.aliases
            .as_ref()
            .and_then(|json| serde_json::from_value(json.clone()).ok())
            .unwrap_or_default()
    }
    
    /// Set aliases from a vector of strings
    pub fn set_aliases(&mut self, aliases: Vec<String>) {
        self.aliases = Some(serde_json::to_value(aliases).unwrap().into());
    }
    
    /// Get attributes as a HashMap
    pub fn get_attributes(&self) -> HashMap<String, serde_json::Value> {
        self.attributes
            .as_ref()
            .and_then(|json| serde_json::from_value(json.clone()).ok())
            .unwrap_or_default()
    }
    
    /// Set attributes from a HashMap
    pub fn set_attributes(&mut self, attributes: HashMap<String, serde_json::Value>) {
        self.attributes = Some(serde_json::to_value(attributes).unwrap().into());
    }
    
    /// Get all possible names this tag can be accessed by
    pub fn get_all_names(&self) -> Vec<String> {
        let mut names = vec![self.canonical_name.clone()];
        
        if let Some(display) = &self.display_name {
            names.push(display.clone());
        }
        
        if let Some(formal) = &self.formal_name {
            names.push(formal.clone());
        }
        
        if let Some(abbrev) = &self.abbreviation {
            names.push(abbrev.clone());
        }
        
        names.extend(self.get_aliases());
        
        names
    }
    
    /// Check if this tag matches the given name in any variant
    pub fn matches_name(&self, name: &str) -> bool {
        self.get_all_names().iter().any(|n| n.eq_ignore_ascii_case(name))
    }
    
    /// Check if this tag should be hidden from normal search results
    pub fn is_searchable(&self) -> bool {
        self.privacy_level == "normal"
    }
    
    /// Get the fully qualified name including namespace
    pub fn get_qualified_name(&self) -> String {
        match &self.namespace {
            Some(ns) => format!("{}::{}", ns, self.canonical_name),
            None => self.canonical_name.clone(),
        }
    }
}

/// Helper enum for tag types (for validation)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TagType {
    Standard,
    Organizational,
    Privacy,
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

/// Helper enum for privacy levels (for validation)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PrivacyLevel {
    Normal,
    Archive,
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