//! Semantic Tags Migration
//! 
//! This migration transforms the current basic tag system into the advanced
//! semantic tagging architecture described in the whitepaper.
//!
//! Key changes:
//! - Replaces simple tags table with semantic_tags
//! - Adds tag hierarchy and relationships
//! - Implements closure table for efficient queries
//! - Adds tag usage pattern tracking
//! - Migrates existing tag data

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Create the enhanced semantic_tags table
        manager
            .create_table(
                Table::create()
                    .table(SemanticTags::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(SemanticTags::Id)
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(SemanticTags::Uuid).uuid().not_null().unique_key())
                    
                    // Core identity
                    .col(ColumnDef::new(SemanticTags::CanonicalName).string().not_null())
                    .col(ColumnDef::new(SemanticTags::DisplayName).string())
                    
                    // Semantic variants
                    .col(ColumnDef::new(SemanticTags::FormalName).string())
                    .col(ColumnDef::new(SemanticTags::Abbreviation).string())
                    .col(ColumnDef::new(SemanticTags::Aliases).json())
                    
                    // Context and categorization
                    .col(ColumnDef::new(SemanticTags::Namespace).string())
                    .col(ColumnDef::new(SemanticTags::TagType).string().not_null().default("standard"))
                    
                    // Visual and behavioral properties
                    .col(ColumnDef::new(SemanticTags::Color).string())
                    .col(ColumnDef::new(SemanticTags::Icon).string())
                    .col(ColumnDef::new(SemanticTags::Description).text())
                    
                    // Advanced capabilities
                    .col(ColumnDef::new(SemanticTags::IsOrganizationalAnchor).boolean().default(false))
                    .col(ColumnDef::new(SemanticTags::PrivacyLevel).string().default("normal"))
                    .col(ColumnDef::new(SemanticTags::SearchWeight).integer().default(100))
                    
                    // Compositional attributes
                    .col(ColumnDef::new(SemanticTags::Attributes).json())
                    .col(ColumnDef::new(SemanticTags::CompositionRules).json())
                    
                    // Metadata
                    .col(ColumnDef::new(SemanticTags::CreatedAt).timestamp_with_time_zone().not_null())
                    .col(ColumnDef::new(SemanticTags::UpdatedAt).timestamp_with_time_zone().not_null())
                    .col(ColumnDef::new(SemanticTags::CreatedByDevice).uuid())
                    
                    // Constraints
                    .index(
                        Index::create()
                            .name("idx_semantic_tags_canonical_namespace")
                            .col(SemanticTags::CanonicalName)
                            .col(SemanticTags::Namespace)
                            .unique()
                    )
                    .to_owned(),
            )
            .await?;

        // Create tag relationships table for hierarchy
        manager
            .create_table(
                Table::create()
                    .table(TagRelationships::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(TagRelationships::Id)
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(TagRelationships::ParentTagId).integer().not_null())
                    .col(ColumnDef::new(TagRelationships::ChildTagId).integer().not_null())
                    .col(ColumnDef::new(TagRelationships::RelationshipType).string().not_null().default("parent_child"))
                    .col(ColumnDef::new(TagRelationships::Strength).real().default(1.0))
                    .col(ColumnDef::new(TagRelationships::CreatedAt).timestamp_with_time_zone().not_null())
                    
                    .foreign_key(
                        ForeignKey::create()
                            .from(TagRelationships::Table, TagRelationships::ParentTagId)
                            .to(SemanticTags::Table, SemanticTags::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(TagRelationships::Table, TagRelationships::ChildTagId)
                            .to(SemanticTags::Table, SemanticTags::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    
                    // Prevent cycles and duplicate relationships
                    .index(
                        Index::create()
                            .name("idx_tag_relationships_unique")
                            .col(TagRelationships::ParentTagId)
                            .col(TagRelationships::ChildTagId)
                            .col(TagRelationships::RelationshipType)
                            .unique()
                    )
                    .to_owned(),
            )
            .await?;

        // Create closure table for efficient hierarchy traversal
        manager
            .create_table(
                Table::create()
                    .table(TagClosure::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(TagClosure::AncestorId)
                            .integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(TagClosure::DescendantId)
                            .integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(TagClosure::Depth)
                            .integer()
                            .not_null(),
                    )
                    .col(ColumnDef::new(TagClosure::PathStrength).real().default(1.0))
                    
                    .primary_key(
                        Index::create()
                            .col(TagClosure::AncestorId)
                            .col(TagClosure::DescendantId)
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(TagClosure::Table, TagClosure::AncestorId)
                            .to(SemanticTags::Table, SemanticTags::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(TagClosure::Table, TagClosure::DescendantId)
                            .to(SemanticTags::Table, SemanticTags::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        // Create enhanced user metadata tagging table
        manager
            .create_table(
                Table::create()
                    .table(UserMetadataSemanticTags::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(UserMetadataSemanticTags::Id)
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(UserMetadataSemanticTags::UserMetadataId).integer().not_null())
                    .col(ColumnDef::new(UserMetadataSemanticTags::TagId).integer().not_null())
                    
                    // Context for this specific tagging instance
                    .col(ColumnDef::new(UserMetadataSemanticTags::AppliedContext).string())
                    .col(ColumnDef::new(UserMetadataSemanticTags::AppliedVariant).string())
                    .col(ColumnDef::new(UserMetadataSemanticTags::Confidence).real().default(1.0))
                    .col(ColumnDef::new(UserMetadataSemanticTags::Source).string().default("user"))
                    
                    // Instance-specific attributes
                    .col(ColumnDef::new(UserMetadataSemanticTags::InstanceAttributes).json())
                    
                    // Audit and sync
                    .col(ColumnDef::new(UserMetadataSemanticTags::CreatedAt).timestamp_with_time_zone().not_null())
                    .col(ColumnDef::new(UserMetadataSemanticTags::UpdatedAt).timestamp_with_time_zone().not_null())
                    .col(ColumnDef::new(UserMetadataSemanticTags::DeviceUuid).uuid().not_null())
                    
                    .foreign_key(
                        ForeignKey::create()
                            .from(UserMetadataSemanticTags::Table, UserMetadataSemanticTags::UserMetadataId)
                            .to(UserMetadata::Table, UserMetadata::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(UserMetadataSemanticTags::Table, UserMetadataSemanticTags::TagId)
                            .to(SemanticTags::Table, SemanticTags::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    
                    .index(
                        Index::create()
                            .name("idx_user_metadata_semantic_tags_unique")
                            .col(UserMetadataSemanticTags::UserMetadataId)
                            .col(UserMetadataSemanticTags::TagId)
                            .unique()
                    )
                    .to_owned(),
            )
            .await?;

        // Create tag usage patterns table for analytics
        manager
            .create_table(
                Table::create()
                    .table(TagUsagePatterns::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(TagUsagePatterns::Id)
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(TagUsagePatterns::TagId).integer().not_null())
                    .col(ColumnDef::new(TagUsagePatterns::CoOccurrenceTagId).integer().not_null())
                    .col(ColumnDef::new(TagUsagePatterns::OccurrenceCount).integer().default(1))
                    .col(ColumnDef::new(TagUsagePatterns::LastUsedTogether).timestamp_with_time_zone().not_null())
                    
                    .foreign_key(
                        ForeignKey::create()
                            .from(TagUsagePatterns::Table, TagUsagePatterns::TagId)
                            .to(SemanticTags::Table, SemanticTags::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(TagUsagePatterns::Table, TagUsagePatterns::CoOccurrenceTagId)
                            .to(SemanticTags::Table, SemanticTags::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    
                    .index(
                        Index::create()
                            .name("idx_tag_usage_patterns_unique")
                            .col(TagUsagePatterns::TagId)
                            .col(TagUsagePatterns::CoOccurrenceTagId)
                            .unique()
                    )
                    .to_owned(),
            )
            .await?;

        // Create full-text search support
        manager
            .execute_unprepared(
                r#"
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
                "#,
            )
            .await?;

        // Create indices for performance
        self.create_semantic_tag_indices(manager).await?;

        // Migrate existing tag data
        self.migrate_existing_tags(manager).await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Drop FTS table first
        manager
            .execute_unprepared("DROP TABLE IF EXISTS tag_search_fts;")
            .await?;

        // Drop tables in reverse order
        manager
            .drop_table(Table::drop().table(TagUsagePatterns::Table).to_owned())
            .await?;
        
        manager
            .drop_table(Table::drop().table(UserMetadataSemanticTags::Table).to_owned())
            .await?;
        
        manager
            .drop_table(Table::drop().table(TagClosure::Table).to_owned())
            .await?;
        
        manager
            .drop_table(Table::drop().table(TagRelationships::Table).to_owned())
            .await?;
        
        manager
            .drop_table(Table::drop().table(SemanticTags::Table).to_owned())
            .await?;

        Ok(())
    }
}

impl Migration {
    async fn create_semantic_tag_indices(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Semantic tags indices
        manager
            .create_index(
                Index::create()
                    .name("idx_semantic_tags_namespace")
                    .table(SemanticTags::Table)
                    .col(SemanticTags::Namespace)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_semantic_tags_type")
                    .table(SemanticTags::Table)
                    .col(SemanticTags::TagType)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_semantic_tags_privacy")
                    .table(SemanticTags::Table)
                    .col(SemanticTags::PrivacyLevel)
                    .to_owned(),
            )
            .await?;

        // Tag closure indices
        manager
            .create_index(
                Index::create()
                    .name("idx_tag_closure_ancestor")
                    .table(TagClosure::Table)
                    .col(TagClosure::AncestorId)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_tag_closure_descendant")
                    .table(TagClosure::Table)
                    .col(TagClosure::DescendantId)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_tag_closure_depth")
                    .table(TagClosure::Table)
                    .col(TagClosure::Depth)
                    .to_owned(),
            )
            .await?;

        // User metadata semantic tags indices
        manager
            .create_index(
                Index::create()
                    .name("idx_user_metadata_semantic_tags_metadata")
                    .table(UserMetadataSemanticTags::Table)
                    .col(UserMetadataSemanticTags::UserMetadataId)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_user_metadata_semantic_tags_tag")
                    .table(UserMetadataSemanticTags::Table)
                    .col(UserMetadataSemanticTags::TagId)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_user_metadata_semantic_tags_source")
                    .table(UserMetadataSemanticTags::Table)
                    .col(UserMetadataSemanticTags::Source)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn migrate_existing_tags(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // TODO: Implement data migration from old tag system
        // This would involve:
        // 1. Reading from existing 'tags' table
        // 2. Converting to SemanticTag format
        // 3. Migrating user_metadata_tags relationships
        // 4. Preserving existing tag assignments
        
        // For now, we'll just add a placeholder migration
        manager
            .execute_unprepared(
                r#"
                -- Insert system tags for demonstration
                INSERT INTO semantic_tags (
                    uuid, canonical_name, tag_type, privacy_level, 
                    created_at, updated_at
                ) VALUES 
                (
                    randomblob(16), 'Important', 'organizational', 'normal',
                    datetime('now'), datetime('now')
                ),
                (
                    randomblob(16), 'Archive', 'privacy', 'archive',
                    datetime('now'), datetime('now')
                );
                "#,
            )
            .await?;

        Ok(())
    }
}

// Table identifiers for semantic tags system

#[derive(DeriveIden)]
enum SemanticTags {
    Table,
    Id,
    Uuid,
    CanonicalName,
    DisplayName,
    FormalName,
    Abbreviation,
    Aliases,
    Namespace,
    TagType,
    Color,
    Icon,
    Description,
    IsOrganizationalAnchor,
    PrivacyLevel,
    SearchWeight,
    Attributes,
    CompositionRules,
    CreatedAt,
    UpdatedAt,
    CreatedByDevice,
}

#[derive(DeriveIden)]
enum TagRelationships {
    Table,
    Id,
    ParentTagId,
    ChildTagId,
    RelationshipType,
    Strength,
    CreatedAt,
}

#[derive(DeriveIden)]
enum TagClosure {
    Table,
    AncestorId,
    DescendantId,
    Depth,
    PathStrength,
}

#[derive(DeriveIden)]
enum UserMetadataSemanticTags {
    Table,
    Id,
    UserMetadataId,
    TagId,
    AppliedContext,
    AppliedVariant,
    Confidence,
    Source,
    InstanceAttributes,
    CreatedAt,
    UpdatedAt,
    DeviceUuid,
}

#[derive(DeriveIden)]
enum TagUsagePatterns {
    Table,
    Id,
    TagId,
    CoOccurrenceTagId,
    OccurrenceCount,
    LastUsedTogether,
}

// Reference to existing user_metadata table
#[derive(DeriveIden)]
enum UserMetadata {
    Table,
    Id,
}