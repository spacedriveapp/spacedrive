//! Migration: Create semantic tagging system
//!
//! This migration creates the complete semantic tagging infrastructure:
//! - Enhanced tag table with polymorphic naming
//! - Hierarchical relationships with closure table
//! - Context-aware tag applications
//! - Usage pattern tracking for intelligent suggestions
//! - Full-text search across all tag variants

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Create the enhanced tag table
        manager
            .create_table(
                Table::create()
                    .table(Alias::new("tag"))
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Alias::new("id"))
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(Alias::new("uuid")).uuid().not_null().unique_key())
                    .col(ColumnDef::new(Alias::new("canonical_name")).string().not_null())
                    .col(ColumnDef::new(Alias::new("display_name")).string())
                    .col(ColumnDef::new(Alias::new("formal_name")).string())
                    .col(ColumnDef::new(Alias::new("abbreviation")).string())
                    .col(ColumnDef::new(Alias::new("aliases")).json())
                    .col(ColumnDef::new(Alias::new("namespace")).string())
                    .col(ColumnDef::new(Alias::new("tag_type")).string().not_null().default("standard"))
                    .col(ColumnDef::new(Alias::new("color")).string())
                    .col(ColumnDef::new(Alias::new("icon")).string())
                    .col(ColumnDef::new(Alias::new("description")).text())
                    .col(ColumnDef::new(Alias::new("is_organizational_anchor")).boolean().default(false))
                    .col(ColumnDef::new(Alias::new("privacy_level")).string().default("normal"))
                    .col(ColumnDef::new(Alias::new("search_weight")).integer().default(100))
                    .col(ColumnDef::new(Alias::new("attributes")).json())
                    .col(ColumnDef::new(Alias::new("composition_rules")).json())
                    .col(ColumnDef::new(Alias::new("created_at")).timestamp_with_time_zone().not_null())
                    .col(ColumnDef::new(Alias::new("updated_at")).timestamp_with_time_zone().not_null())
                    .col(ColumnDef::new(Alias::new("created_by_device")).uuid())
                    .to_owned(),
            )
            .await?;

        // Create indexes for the tag table
        manager
            .create_index(
                Index::create()
                    .name("idx_tag_canonical_name")
                    .table(Alias::new("tag"))
                    .col(Alias::new("canonical_name"))
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_tag_namespace")
                    .table(Alias::new("tag"))
                    .col(Alias::new("namespace"))
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_tag_type")
                    .table(Alias::new("tag"))
                    .col(Alias::new("tag_type"))
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_tag_privacy_level")
                    .table(Alias::new("tag"))
                    .col(Alias::new("privacy_level"))
                    .to_owned(),
            )
            .await?;

        // Create the tag_relationship table
        manager
            .create_table(
                Table::create()
                    .table(Alias::new("tag_relationship"))
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Alias::new("id"))
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(Alias::new("parent_tag_id")).integer().not_null())
                    .col(ColumnDef::new(Alias::new("child_tag_id")).integer().not_null())
                    .col(ColumnDef::new(Alias::new("relationship_type")).string().not_null().default("parent_child"))
                    .col(ColumnDef::new(Alias::new("strength")).float().default(1.0))
                    .col(ColumnDef::new(Alias::new("created_at")).timestamp_with_time_zone().not_null())
                    .to_owned(),
            )
            .await?;

        // Create foreign key constraints for tag_relationship
        manager
            .create_foreign_key(
                ForeignKey::create()
                    .name("fk_tag_relationship_parent")
                    .from(Alias::new("tag_relationship"), Alias::new("parent_tag_id"))
                    .to(Alias::new("tag"), Alias::new("id"))
                    .on_delete(ForeignKeyAction::Cascade)
                    .to_owned(),
            )
            .await?;

        manager
            .create_foreign_key(
                ForeignKey::create()
                    .name("fk_tag_relationship_child")
                    .from(Alias::new("tag_relationship"), Alias::new("child_tag_id"))
                    .to(Alias::new("tag"), Alias::new("id"))
                    .on_delete(ForeignKeyAction::Cascade)
                    .to_owned(),
            )
            .await?;

        // Create indexes for tag_relationship
        manager
            .create_index(
                Index::create()
                    .name("idx_tag_relationship_parent")
                    .table(Alias::new("tag_relationship"))
                    .col(Alias::new("parent_tag_id"))
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_tag_relationship_child")
                    .table(Alias::new("tag_relationship"))
                    .col(Alias::new("child_tag_id"))
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_tag_relationship_type")
                    .table(Alias::new("tag_relationship"))
                    .col(Alias::new("relationship_type"))
                    .to_owned(),
            )
            .await?;

        // Create the tag_closure table for efficient hierarchical queries
        manager
            .create_table(
                Table::create()
                    .table(Alias::new("tag_closure"))
                    .if_not_exists()
                    .col(ColumnDef::new(Alias::new("ancestor_id")).integer().not_null())
                    .col(ColumnDef::new(Alias::new("descendant_id")).integer().not_null())
                    .col(ColumnDef::new(Alias::new("depth")).integer().not_null())
                    .col(ColumnDef::new(Alias::new("path_strength")).float().not_null())
                    .primary_key(
                        Index::create()
                            .col(Alias::new("ancestor_id"))
                            .col(Alias::new("descendant_id")),
                    )
                    .to_owned(),
            )
            .await?;

        // Create foreign key constraints for tag_closure
        manager
            .create_foreign_key(
                ForeignKey::create()
                    .name("fk_tag_closure_ancestor")
                    .from(Alias::new("tag_closure"), Alias::new("ancestor_id"))
                    .to(Alias::new("tag"), Alias::new("id"))
                    .on_delete(ForeignKeyAction::Cascade)
                    .to_owned(),
            )
            .await?;

        manager
            .create_foreign_key(
                ForeignKey::create()
                    .name("fk_tag_closure_descendant")
                    .from(Alias::new("tag_closure"), Alias::new("descendant_id"))
                    .to(Alias::new("tag"), Alias::new("id"))
                    .on_delete(ForeignKeyAction::Cascade)
                    .to_owned(),
            )
            .await?;

        // Create indexes for tag_closure
        manager
            .create_index(
                Index::create()
                    .name("idx_tag_closure_ancestor")
                    .table(Alias::new("tag_closure"))
                    .col(Alias::new("ancestor_id"))
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_tag_closure_descendant")
                    .table(Alias::new("tag_closure"))
                    .col(Alias::new("descendant_id"))
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_tag_closure_depth")
                    .table(Alias::new("tag_closure"))
                    .col(Alias::new("depth"))
                    .to_owned(),
            )
            .await?;

        // Create the user_metadata_tag table
        manager
            .create_table(
                Table::create()
                    .table(Alias::new("user_metadata_tag"))
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Alias::new("id"))
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(Alias::new("user_metadata_id")).integer().not_null())
                    .col(ColumnDef::new(Alias::new("tag_id")).integer().not_null())
                    .col(ColumnDef::new(Alias::new("applied_context")).string())
                    .col(ColumnDef::new(Alias::new("applied_variant")).string())
                    .col(ColumnDef::new(Alias::new("confidence")).float().default(1.0))
                    .col(ColumnDef::new(Alias::new("source")).string().default("user"))
                    .col(ColumnDef::new(Alias::new("instance_attributes")).json())
                    .col(ColumnDef::new(Alias::new("created_at")).timestamp_with_time_zone().not_null())
                    .col(ColumnDef::new(Alias::new("updated_at")).timestamp_with_time_zone().not_null())
                    .col(ColumnDef::new(Alias::new("device_uuid")).uuid().not_null())
                    .to_owned(),
            )
            .await?;

        // Create foreign key constraints for user_metadata_tag
        manager
            .create_foreign_key(
                ForeignKey::create()
                    .name("fk_user_metadata_tag_metadata")
                    .from(Alias::new("user_metadata_tag"), Alias::new("user_metadata_id"))
                    .to(Alias::new("user_metadata"), Alias::new("id"))
                    .on_delete(ForeignKeyAction::Cascade)
                    .to_owned(),
            )
            .await?;

        manager
            .create_foreign_key(
                ForeignKey::create()
                    .name("fk_user_metadata_tag_tag")
                    .from(Alias::new("user_metadata_tag"), Alias::new("tag_id"))
                    .to(Alias::new("tag"), Alias::new("id"))
                    .on_delete(ForeignKeyAction::Cascade)
                    .to_owned(),
            )
            .await?;

        // Create indexes for user_metadata_tag
        manager
            .create_index(
                Index::create()
                    .name("idx_user_metadata_tag_metadata")
                    .table(Alias::new("user_metadata_tag"))
                    .col(Alias::new("user_metadata_id"))
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_user_metadata_tag_tag")
                    .table(Alias::new("user_metadata_tag"))
                    .col(Alias::new("tag_id"))
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_user_metadata_tag_source")
                    .table(Alias::new("user_metadata_tag"))
                    .col(Alias::new("source"))
                    .to_owned(),
            )
            .await?;

        // Create the tag_usage_pattern table
        manager
            .create_table(
                Table::create()
                    .table(Alias::new("tag_usage_pattern"))
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Alias::new("id"))
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(Alias::new("tag_id")).integer().not_null())
                    .col(ColumnDef::new(Alias::new("co_occurrence_tag_id")).integer().not_null())
                    .col(ColumnDef::new(Alias::new("occurrence_count")).integer().default(1))
                    .col(ColumnDef::new(Alias::new("last_used_together")).timestamp_with_time_zone().not_null())
                    .to_owned(),
            )
            .await?;

        // Create foreign key constraints for tag_usage_pattern
        manager
            .create_foreign_key(
                ForeignKey::create()
                    .name("fk_tag_usage_pattern_tag")
                    .from(Alias::new("tag_usage_pattern"), Alias::new("tag_id"))
                    .to(Alias::new("tag"), Alias::new("id"))
                    .on_delete(ForeignKeyAction::Cascade)
                    .to_owned(),
            )
            .await?;

        manager
            .create_foreign_key(
                ForeignKey::create()
                    .name("fk_tag_usage_pattern_co_occurrence")
                    .from(Alias::new("tag_usage_pattern"), Alias::new("co_occurrence_tag_id"))
                    .to(Alias::new("tag"), Alias::new("id"))
                    .on_delete(ForeignKeyAction::Cascade)
                    .to_owned(),
            )
            .await?;

        // Create indexes for tag_usage_pattern
        manager
            .create_index(
                Index::create()
                    .name("idx_tag_usage_pattern_tag")
                    .table(Alias::new("tag_usage_pattern"))
                    .col(Alias::new("tag_id"))
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_tag_usage_pattern_co_occurrence")
                    .table(Alias::new("tag_usage_pattern"))
                    .col(Alias::new("co_occurrence_tag_id"))
                    .to_owned(),
            )
            .await?;

        // Create full-text search indexes
        manager
            .create_index(
                Index::create()
                    .name("idx_tag_fulltext")
                    .table(Alias::new("tag"))
                    .col(Alias::new("canonical_name"))
                    .col(Alias::new("display_name"))
                    .col(Alias::new("formal_name"))
                    .col(Alias::new("abbreviation"))
                    .col(Alias::new("aliases"))
                    .col(Alias::new("description"))
                    .to_owned(),
            )
            .await?;

        // Create FTS5 virtual table for full-text search
        manager
            .get_connection()
            .execute_unprepared(
                "CREATE VIRTUAL TABLE IF NOT EXISTS tag_search_fts USING fts5(
                    tag_id UNINDEXED,
                    canonical_name,
                    display_name,
                    formal_name,
                    abbreviation,
                    aliases,
                    description,
                    content='tag',
                    content_rowid='id'
                )"
            )
            .await?;

        // Create triggers to maintain FTS5 table
        manager
            .get_connection()
            .execute_unprepared(
                "CREATE TRIGGER IF NOT EXISTS tag_ai AFTER INSERT ON tag BEGIN
                    INSERT INTO tag_search_fts(
                        tag_id, canonical_name, display_name, formal_name,
                        abbreviation, aliases, description
                    ) VALUES (
                        NEW.id, NEW.canonical_name, NEW.display_name, NEW.formal_name,
                        NEW.abbreviation, NEW.aliases, NEW.description
                    );
                END"
            )
            .await?;

        manager
            .get_connection()
            .execute_unprepared(
                "CREATE TRIGGER IF NOT EXISTS tag_au AFTER UPDATE ON tag BEGIN
                    UPDATE tag_search_fts SET
                        canonical_name = NEW.canonical_name,
                        display_name = NEW.display_name,
                        formal_name = NEW.formal_name,
                        abbreviation = NEW.abbreviation,
                        aliases = NEW.aliases,
                        description = NEW.description
                    WHERE tag_id = NEW.id;
                END"
            )
            .await?;

        manager
            .get_connection()
            .execute_unprepared(
                "CREATE TRIGGER IF NOT EXISTS tag_ad AFTER DELETE ON tag BEGIN
                    DELETE FROM tag_search_fts WHERE tag_id = OLD.id;
                END"
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Drop FTS5 table and triggers first
        manager
            .get_connection()
            .execute_unprepared("DROP TRIGGER IF EXISTS tag_ad")
            .await?;
        manager
            .get_connection()
            .execute_unprepared("DROP TRIGGER IF EXISTS tag_au")
            .await?;
        manager
            .get_connection()
            .execute_unprepared("DROP TRIGGER IF EXISTS tag_ai")
            .await?;
        manager
            .get_connection()
            .execute_unprepared("DROP TABLE IF EXISTS tag_search_fts")
            .await?;

        // Drop tables in reverse order
        manager
            .drop_table(Table::drop().table(Alias::new("tag_usage_pattern")).to_owned())
            .await?;

        manager
            .drop_table(Table::drop().table(Alias::new("user_metadata_tag")).to_owned())
            .await?;

        manager
            .drop_table(Table::drop().table(Alias::new("tag_closure")).to_owned())
            .await?;

        manager
            .drop_table(Table::drop().table(Alias::new("tag_relationship")).to_owned())
            .await?;

        manager
            .drop_table(Table::drop().table(Alias::new("tag")).to_owned())
            .await?;

        Ok(())
    }
}