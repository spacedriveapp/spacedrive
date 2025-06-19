//! Entry entity

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "entries")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    pub uuid: Uuid,
    pub prefix_id: i32,  // References path_prefixes table
    pub relative_path: String,  // Path relative to prefix
    pub name: String,
    pub kind: i32,  // Entry type: 0=File, 1=Directory, 2=Symlink
    pub extension: Option<String>,  // File extension (without dot), None for directories
    pub metadata_id: Option<i32>,  // Optional - only when user adds metadata
    pub content_id: Option<i32>,  // Optional - for deduplication
    pub location_id: Option<i32>,
    pub size: i64,
    pub aggregate_size: i64,  // Total size including all children (for directories)
    pub child_count: i32,  // Total number of direct children
    pub file_count: i32,  // Total number of files in this directory and subdirectories
    pub created_at: DateTimeUtc,
    pub modified_at: DateTimeUtc,
    pub accessed_at: Option<DateTimeUtc>,
    pub permissions: Option<String>,  // Unix permissions as string
    pub inode: Option<i64>,  // Platform-specific file identifier for change detection
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::path_prefix::Entity",
        from = "Column::PrefixId",
        to = "super::path_prefix::Column::Id"
    )]
    PathPrefix,
    #[sea_orm(
        belongs_to = "super::user_metadata::Entity",
        from = "Column::MetadataId",
        to = "super::user_metadata::Column::Id"
    )]
    UserMetadata,
    #[sea_orm(
        belongs_to = "super::content_identity::Entity",
        from = "Column::ContentId",
        to = "super::content_identity::Column::Id"
    )]
    ContentIdentity,
    #[sea_orm(
        belongs_to = "super::location::Entity",
        from = "Column::LocationId",
        to = "super::location::Column::Id"
    )]
    Location,
}

impl Related<super::path_prefix::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::PathPrefix.def()
    }
}

impl Related<super::user_metadata::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::UserMetadata.def()
    }
}

impl Related<super::content_identity::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::ContentIdentity.def()
    }
}

impl Related<super::location::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Location.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}