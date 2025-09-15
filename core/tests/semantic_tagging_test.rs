//! Integration tests for semantic tagging system
//!
//! These tests validate the complete semantic tagging implementation including
//! database operations, hierarchy management, and context resolution.

use sd_core::{
    domain::semantic_tag::{SemanticTag, TagType, PrivacyLevel, RelationshipType, TagSource, TagApplication},
    domain::semantic_tag_validation::SemanticTagValidator,
    service::semantic_tag_service::SemanticTagService,
    service::user_metadata_service::UserMetadataService,
    infra::db::Database,
};
use std::sync::Arc;
use uuid::Uuid;

/// Test basic tag creation and validation
#[tokio::test]
async fn test_semantic_tag_creation() {
    let device_id = Uuid::new_v4();

    // Test basic tag creation
    let tag = SemanticTag::new("JavaScript".to_string(), device_id);
    assert_eq!(tag.canonical_name, "JavaScript");
    assert_eq!(tag.tag_type, TagType::Standard);
    assert_eq!(tag.privacy_level, PrivacyLevel::Normal);
    assert!(!tag.is_organizational_anchor);

    // Test validation
    assert!(SemanticTagValidator::validate_semantic_tag(&tag).is_ok());
}

/// Test tag name variants and matching
#[tokio::test]
async fn test_tag_variants() {
    let device_id = Uuid::new_v4();
    let mut tag = SemanticTag::new("JavaScript".to_string(), device_id);

    // Add variants
    tag.formal_name = Some("JavaScript Programming Language".to_string());
    tag.abbreviation = Some("JS".to_string());
    tag.add_alias("ECMAScript".to_string());
    tag.add_alias("ES".to_string());

    // Test name matching
    assert!(tag.matches_name("JavaScript"));
    assert!(tag.matches_name("js")); // Case insensitive
    assert!(tag.matches_name("ECMAScript"));
    assert!(tag.matches_name("JavaScript Programming Language"));
    assert!(!tag.matches_name("Python"));

    // Test all names collection
    let all_names = tag.get_all_names();
    assert!(all_names.contains(&"JavaScript"));
    assert!(all_names.contains(&"JS"));
    assert!(all_names.contains(&"ECMAScript"));
    assert!(all_names.contains(&"ES"));
    assert!(all_names.contains(&"JavaScript Programming Language"));
}

/// Test polymorphic naming with namespaces
#[tokio::test]
async fn test_polymorphic_naming() {
    let device_id = Uuid::new_v4();

    // Create two "Phoenix" tags in different namespaces
    let mut phoenix_city = SemanticTag::new("Phoenix".to_string(), device_id);
    phoenix_city.namespace = Some("Geography".to_string());
    phoenix_city.description = Some("City in Arizona, USA".to_string());

    let mut phoenix_myth = SemanticTag::new("Phoenix".to_string(), device_id);
    phoenix_myth.namespace = Some("Mythology".to_string());
    phoenix_myth.description = Some("Mythical bird that rises from ashes".to_string());

    // Both should have the same canonical name but different qualified names
    assert_eq!(phoenix_city.canonical_name, "Phoenix");
    assert_eq!(phoenix_myth.canonical_name, "Phoenix");
    assert_eq!(phoenix_city.get_qualified_name(), "Geography::Phoenix");
    assert_eq!(phoenix_myth.get_qualified_name(), "Mythology::Phoenix");

    // Validation should pass for both
    assert!(SemanticTagValidator::validate_semantic_tag(&phoenix_city).is_ok());
    assert!(SemanticTagValidator::validate_semantic_tag(&phoenix_myth).is_ok());
}

/// Test tag validation rules
#[tokio::test]
async fn test_tag_validation() {
    // Test valid tag names
    assert!(SemanticTagValidator::validate_tag_name("JavaScript").is_ok());
    assert!(SemanticTagValidator::validate_tag_name("日本語").is_ok()); // Unicode
    assert!(SemanticTagValidator::validate_tag_name("Project-2024").is_ok());

    // Test invalid tag names
    assert!(SemanticTagValidator::validate_tag_name("").is_err()); // Empty
    assert!(SemanticTagValidator::validate_tag_name("   ").is_err()); // Whitespace only
    assert!(SemanticTagValidator::validate_tag_name(" JavaScript ").is_err()); // Leading/trailing space

    // Test color validation
    assert!(SemanticTagValidator::validate_color("#FF0000").is_ok());
    assert!(SemanticTagValidator::validate_color("#123abc").is_ok());
    assert!(SemanticTagValidator::validate_color("FF0000").is_err()); // No #
    assert!(SemanticTagValidator::validate_color("#GG0000").is_err()); // Invalid hex

    // Test namespace validation
    assert!(SemanticTagValidator::validate_namespace("Technology").is_ok());
    assert!(SemanticTagValidator::validate_namespace("Web Development").is_ok());
    assert!(SemanticTagValidator::validate_namespace("Tech@!#").is_err()); // Special chars
}

/// Test tag application creation
#[tokio::test]
async fn test_tag_applications() {
    let tag_id = Uuid::new_v4();
    let device_id = Uuid::new_v4();

    // Test user-applied tag
    let user_app = TagApplication::user_applied(tag_id, device_id);
    assert_eq!(user_app.tag_id, tag_id);
    assert_eq!(user_app.source, TagSource::User);
    assert_eq!(user_app.confidence, 1.0);
    assert!(user_app.is_high_confidence());

    // Test AI-applied tag
    let ai_app = TagApplication::ai_applied(tag_id, 0.85, device_id);
    assert_eq!(ai_app.source, TagSource::AI);
    assert_eq!(ai_app.confidence, 0.85);
    assert!(ai_app.is_high_confidence());

    // Test low confidence AI tag
    let low_conf_app = TagApplication::ai_applied(tag_id, 0.6, device_id);
    assert!(!low_conf_app.is_high_confidence());
}

/// Test organizational tag rules
#[tokio::test]
async fn test_organizational_tags() {
    let device_id = Uuid::new_v4();

    // Create organizational tag
    let mut org_tag = SemanticTag::new("Projects".to_string(), device_id);
    org_tag.tag_type = TagType::Organizational;
    org_tag.is_organizational_anchor = true;

    // Should validate successfully
    assert!(SemanticTagValidator::validate_semantic_tag(&org_tag).is_ok());

    // Test invalid organizational tag (not marked as anchor)
    let mut invalid_org_tag = SemanticTag::new("Projects".to_string(), device_id);
    invalid_org_tag.tag_type = TagType::Organizational;
    invalid_org_tag.is_organizational_anchor = false;

    // Should fail validation
    assert!(SemanticTagValidator::validate_semantic_tag(&invalid_org_tag).is_err());
}

/// Test privacy tag rules
#[tokio::test]
async fn test_privacy_tags() {
    let device_id = Uuid::new_v4();

    // Create valid archive tag
    let mut archive_tag = SemanticTag::new("Personal".to_string(), device_id);
    archive_tag.tag_type = TagType::Privacy;
    archive_tag.privacy_level = PrivacyLevel::Archive;

    assert!(SemanticTagValidator::validate_semantic_tag(&archive_tag).is_ok());

    // Create invalid privacy tag (normal privacy level)
    let mut invalid_privacy_tag = SemanticTag::new("Personal".to_string(), device_id);
    invalid_privacy_tag.tag_type = TagType::Privacy;
    invalid_privacy_tag.privacy_level = PrivacyLevel::Normal;

    assert!(SemanticTagValidator::validate_semantic_tag(&invalid_privacy_tag).is_err());
}

/// Test tag searchability based on privacy level
#[tokio::test]
async fn test_tag_searchability() {
    let device_id = Uuid::new_v4();

    // Normal tag should be searchable
    let normal_tag = SemanticTag::new("Normal".to_string(), device_id);
    assert!(normal_tag.is_searchable());

    // Archive tag should not be searchable
    let mut archive_tag = SemanticTag::new("Archive".to_string(), device_id);
    archive_tag.privacy_level = PrivacyLevel::Archive;
    assert!(!archive_tag.is_searchable());

    // Hidden tag should not be searchable
    let mut hidden_tag = SemanticTag::new("Hidden".to_string(), device_id);
    hidden_tag.privacy_level = PrivacyLevel::Hidden;
    assert!(!hidden_tag.is_searchable());
}

// Database integration tests would go here if we had a test database setup
// These would test the actual SemanticTagService database operations:
// - Tag creation and persistence
// - Hierarchy creation and closure table maintenance
// - Context resolution with real data
// - Usage pattern tracking
// - Full-text search functionality

// Example of what a database integration test would look like:
/*
#[tokio::test]
async fn test_tag_creation_with_database() {
    let db = setup_test_database().await;
    let service = SemanticTagService::new(db);
    let device_id = Uuid::new_v4();

    // Create a tag
    let tag = service.create_tag(
        "JavaScript".to_string(),
        Some("Technology".to_string()),
        device_id,
    ).await.unwrap();

    // Verify it can be found
    let found = service.find_tag_by_name_and_namespace(
        "JavaScript",
        Some("Technology"),
    ).await.unwrap();

    assert!(found.is_some());
    assert_eq!(found.unwrap().canonical_name, "JavaScript");
}
*/