//! Validation rules for semantic tags
//!
//! This module provides comprehensive validation for semantic tag operations
//! to ensure data integrity and user experience consistency.

use crate::domain::semantic_tag::{SemanticTag, TagType, PrivacyLevel, TagError};
use regex::Regex;
use std::collections::HashSet;

/// Validation rules for semantic tags
pub struct SemanticTagValidator;

impl SemanticTagValidator {
    /// Validate a tag name (canonical, formal, abbreviation, or alias)
    pub fn validate_tag_name(name: &str) -> Result<(), TagError> {
        if name.trim().is_empty() {
            return Err(TagError::InvalidCompositionRule("Tag name cannot be empty".to_string()));
        }
        
        if name.len() > 255 {
            return Err(TagError::InvalidCompositionRule("Tag name cannot exceed 255 characters".to_string()));
        }
        
        // Allow Unicode but prevent control characters
        if name.chars().any(|c| c.is_control() && c != '\n' && c != '\r' && c != '\t') {
            return Err(TagError::InvalidCompositionRule("Tag name cannot contain control characters".to_string()));
        }
        
        // Prevent leading/trailing whitespace
        if name != name.trim() {
            return Err(TagError::InvalidCompositionRule("Tag name cannot have leading or trailing whitespace".to_string()));
        }
        
        Ok(())
    }
    
    /// Validate a namespace name
    pub fn validate_namespace(namespace: &str) -> Result<(), TagError> {
        Self::validate_tag_name(namespace)?;
        
        if namespace.len() > 100 {
            return Err(TagError::InvalidCompositionRule("Namespace cannot exceed 100 characters".to_string()));
        }
        
        // Namespace should follow a simple pattern
        let namespace_regex = Regex::new(r"^[a-zA-Z0-9_\-\s]+$").unwrap();
        if !namespace_regex.is_match(namespace) {
            return Err(TagError::InvalidCompositionRule(
                "Namespace can only contain letters, numbers, underscores, hyphens, and spaces".to_string()
            ));
        }
        
        Ok(())
    }
    
    /// Validate a color hex code
    pub fn validate_color(color: &str) -> Result<(), TagError> {
        let color_regex = Regex::new(r"^#[0-9A-Fa-f]{6}$").unwrap();
        if !color_regex.is_match(color) {
            return Err(TagError::InvalidCompositionRule(
                "Color must be in hex format (#RRGGBB)".to_string()
            ));
        }
        Ok(())
    }
    
    /// Validate a complete semantic tag
    pub fn validate_semantic_tag(tag: &SemanticTag) -> Result<(), TagError> {
        // Validate canonical name
        Self::validate_tag_name(&tag.canonical_name)?;
        
        // Validate namespace if present
        if let Some(namespace) = &tag.namespace {
            Self::validate_namespace(namespace)?;
        }
        
        // Validate formal name if present
        if let Some(formal_name) = &tag.formal_name {
            Self::validate_tag_name(formal_name)?;
        }
        
        // Validate abbreviation if present
        if let Some(abbreviation) = &tag.abbreviation {
            Self::validate_tag_name(abbreviation)?;
            
            if abbreviation.len() > 10 {
                return Err(TagError::InvalidCompositionRule(
                    "Abbreviation should be 10 characters or less".to_string()
                ));
            }
        }
        
        // Validate aliases
        let mut alias_set = HashSet::new();
        for alias in &tag.aliases {
            Self::validate_tag_name(alias)?;
            
            // Check for duplicate aliases
            if !alias_set.insert(alias.to_lowercase()) {
                return Err(TagError::InvalidCompositionRule(
                    format!("Duplicate alias: {}", alias)
                ));
            }
        }
        
        // Validate color if present
        if let Some(color) = &tag.color {
            Self::validate_color(color)?;
        }
        
        // Validate search weight
        if tag.search_weight < 0 || tag.search_weight > 1000 {
            return Err(TagError::InvalidCompositionRule(
                "Search weight must be between 0 and 1000".to_string()
            ));
        }
        
        // Validate description length
        if let Some(description) = &tag.description {
            if description.len() > 2000 {
                return Err(TagError::InvalidCompositionRule(
                    "Description cannot exceed 2000 characters".to_string()
                ));
            }
        }
        
        // Business rule validations
        Self::validate_tag_type_rules(tag)?;
        Self::validate_privacy_level_rules(tag)?;
        
        Ok(())
    }
    
    fn validate_tag_type_rules(tag: &SemanticTag) -> Result<(), TagError> {
        match tag.tag_type {
            TagType::Organizational => {
                // Organizational tags should be anchors
                if !tag.is_organizational_anchor {
                    return Err(TagError::InvalidCompositionRule(
                        "Organizational tags should be marked as organizational anchors".to_string()
                    ));
                }
            }
            TagType::Privacy => {
                // Privacy tags should have non-normal privacy level
                if tag.privacy_level == PrivacyLevel::Normal {
                    return Err(TagError::InvalidCompositionRule(
                        "Privacy tags should have Archive or Hidden privacy level".to_string()
                    ));
                }
            }
            TagType::System => {
                // System tags shouldn't be organizational anchors by default
                if tag.is_organizational_anchor {
                    return Err(TagError::InvalidCompositionRule(
                        "System tags should not be organizational anchors unless specifically needed".to_string()
                    ));
                }
            }
            TagType::Standard => {
                // No special rules for standard tags
            }
        }
        
        Ok(())
    }
    
    fn validate_privacy_level_rules(tag: &SemanticTag) -> Result<(), TagError> {
        match tag.privacy_level {
            PrivacyLevel::Hidden => {
                // Hidden tags should have low search weight
                if tag.search_weight > 50 {
                    return Err(TagError::InvalidCompositionRule(
                        "Hidden tags should have low search weight (≤50)".to_string()
                    ));
                }
            }
            PrivacyLevel::Archive => {
                // Archive tags should have reduced search weight
                if tag.search_weight > 200 {
                    return Err(TagError::InvalidCompositionRule(
                        "Archive tags should have reduced search weight (≤200)".to_string()
                    ));
                }
            }
            PrivacyLevel::Normal => {
                // No special rules for normal privacy
            }
        }
        
        Ok(())
    }
    
    /// Validate tag name conflicts within a namespace
    pub fn validate_no_name_conflicts(
        new_tag: &SemanticTag,
        existing_tags: &[SemanticTag],
    ) -> Result<(), TagError> {
        for existing in existing_tags {
            // Skip if different namespace
            if existing.namespace != new_tag.namespace {
                continue;
            }
            
            // Check canonical name conflict
            if existing.canonical_name.eq_ignore_ascii_case(&new_tag.canonical_name) {
                return Err(TagError::NameConflict(format!(
                    "Tag with canonical name '{}' already exists in namespace '{:?}'",
                    new_tag.canonical_name, new_tag.namespace
                )));
            }
            
            // Check against all variants of existing tag
            let existing_names = existing.get_all_names();
            let new_names = new_tag.get_all_names();
            
            for new_name in &new_names {
                for existing_name in &existing_names {
                    if new_name.eq_ignore_ascii_case(existing_name) {
                        return Err(TagError::NameConflict(format!(
                            "Tag variant '{}' conflicts with existing tag '{}' in namespace '{:?}'",
                            new_name, existing.canonical_name, new_tag.namespace
                        )));
                    }
                }
            }
        }
        
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;
    
    #[test]
    fn test_tag_name_validation() {
        // Valid names
        assert!(SemanticTagValidator::validate_tag_name("JavaScript").is_ok());
        assert!(SemanticTagValidator::validate_tag_name("日本語").is_ok()); // Unicode
        assert!(SemanticTagValidator::validate_tag_name("Project-2024").is_ok());
        
        // Invalid names
        assert!(SemanticTagValidator::validate_tag_name("").is_err()); // Empty
        assert!(SemanticTagValidator::validate_tag_name("   ").is_err()); // Whitespace only
        assert!(SemanticTagValidator::validate_tag_name(" JavaScript ").is_err()); // Leading/trailing space
        
        // Long name
        let long_name = "a".repeat(256);
        assert!(SemanticTagValidator::validate_tag_name(&long_name).is_err());
    }
    
    #[test]
    fn test_namespace_validation() {
        // Valid namespaces
        assert!(SemanticTagValidator::validate_namespace("Technology").is_ok());
        assert!(SemanticTagValidator::validate_namespace("Web Development").is_ok());
        assert!(SemanticTagValidator::validate_namespace("AI_Models").is_ok());
        
        // Invalid namespaces
        assert!(SemanticTagValidator::validate_namespace("").is_err());
        assert!(SemanticTagValidator::validate_namespace("Tech@!#").is_err()); // Special chars
    }
    
    #[test]
    fn test_color_validation() {
        // Valid colors
        assert!(SemanticTagValidator::validate_color("#FF0000").is_ok());
        assert!(SemanticTagValidator::validate_color("#123abc").is_ok());
        
        // Invalid colors
        assert!(SemanticTagValidator::validate_color("FF0000").is_err()); // No #
        assert!(SemanticTagValidator::validate_color("#FF00").is_err()); // Too short
        assert!(SemanticTagValidator::validate_color("#GG0000").is_err()); // Invalid hex
    }
}