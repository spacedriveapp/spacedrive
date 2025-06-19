//! User metadata - tags, labels, notes, and custom fields
//! 
//! This is the key innovation: EVERY Entry has UserMetadata, even if empty.
//! This means any file can be tagged immediately without content indexing.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use uuid::Uuid;

/// User-applied metadata for any Entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserMetadata {
    /// Unique identifier (matches Entry.metadata_id)
    pub id: Uuid,
    
    /// User-applied tags
    pub tags: Vec<Tag>,
    
    /// Labels for categorization
    pub labels: Vec<Label>,
    
    /// Free-form notes
    pub notes: Option<String>,
    
    /// Whether this entry is marked as favorite
    pub favorite: bool,
    
    /// Whether this entry should be hidden
    pub hidden: bool,
    
    /// Custom fields for future extensibility
    pub custom_fields: JsonValue,
    
    /// When this metadata was created
    pub created_at: DateTime<Utc>,
    
    /// When this metadata was last updated
    pub updated_at: DateTime<Utc>,
}

/// A user-defined tag
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Tag {
    /// Unique tag ID
    pub id: Uuid,
    
    /// Tag name
    pub name: String,
    
    /// Optional color (hex format)
    pub color: Option<String>,
    
    /// Optional emoji/icon
    pub icon: Option<String>,
}

/// A label for categorization
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Label {
    /// Unique label ID
    pub id: Uuid,
    
    /// Label name
    pub name: String,
    
    /// Label color (hex format)
    pub color: String,
}

impl UserMetadata {
    /// Create new empty metadata
    pub fn new(id: Uuid) -> Self {
        let now = Utc::now();
        Self {
            id,
            tags: Vec::new(),
            labels: Vec::new(),
            notes: None,
            favorite: false,
            hidden: false,
            custom_fields: JsonValue::Object(serde_json::Map::new()),
            created_at: now,
            updated_at: now,
        }
    }
    
    /// Add a tag
    pub fn add_tag(&mut self, tag: Tag) {
        if !self.tags.iter().any(|t| t.id == tag.id) {
            self.tags.push(tag);
            self.updated_at = Utc::now();
        }
    }
    
    /// Remove a tag
    pub fn remove_tag(&mut self, tag_id: Uuid) {
        if let Some(pos) = self.tags.iter().position(|t| t.id == tag_id) {
            self.tags.remove(pos);
            self.updated_at = Utc::now();
        }
    }
    
    /// Add a label
    pub fn add_label(&mut self, label: Label) {
        if !self.labels.iter().any(|l| l.id == label.id) {
            self.labels.push(label);
            self.updated_at = Utc::now();
        }
    }
    
    /// Remove a label
    pub fn remove_label(&mut self, label_id: Uuid) {
        if let Some(pos) = self.labels.iter().position(|l| l.id == label_id) {
            self.labels.remove(pos);
            self.updated_at = Utc::now();
        }
    }
    
    /// Set notes
    pub fn set_notes(&mut self, notes: Option<String>) {
        self.notes = notes;
        self.updated_at = Utc::now();
    }
    
    /// Toggle favorite status
    pub fn toggle_favorite(&mut self) {
        self.favorite = !self.favorite;
        self.updated_at = Utc::now();
    }
    
    /// Set hidden status
    pub fn set_hidden(&mut self, hidden: bool) {
        self.hidden = hidden;
        self.updated_at = Utc::now();
    }
    
    /// Check if metadata has any user-applied data
    pub fn is_empty(&self) -> bool {
        self.tags.is_empty() 
            && self.labels.is_empty() 
            && self.notes.is_none() 
            && !self.favorite 
            && !self.hidden
            && self.custom_fields == JsonValue::Object(serde_json::Map::new())
    }
}

impl Default for UserMetadata {
    fn default() -> Self {
        Self::new(Uuid::new_v4())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_empty_metadata() {
        let metadata = UserMetadata::new(Uuid::new_v4());
        assert!(metadata.is_empty());
        assert_eq!(metadata.tags.len(), 0);
        assert_eq!(metadata.labels.len(), 0);
        assert!(!metadata.favorite);
        assert!(!metadata.hidden);
    }
    
    #[test]
    fn test_add_tag() {
        let mut metadata = UserMetadata::new(Uuid::new_v4());
        let tag = Tag {
            id: Uuid::new_v4(),
            name: "Important".to_string(),
            color: Some("#FF0000".to_string()),
            icon: Some("⭐".to_string()),
        };
        
        metadata.add_tag(tag.clone());
        assert_eq!(metadata.tags.len(), 1);
        assert!(!metadata.is_empty());
        
        // Adding same tag again shouldn't duplicate
        metadata.add_tag(tag);
        assert_eq!(metadata.tags.len(), 1);
    }
}