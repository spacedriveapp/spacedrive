//! Entry entity

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "entries")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    pub uuid: Option<Uuid>, // None until content identification phase complete (sync readiness indicator)
    pub location_id: i32,  // References location table
    pub relative_path: String,  // Directory path within location
    pub name: String,
    pub kind: i32,  // Entry type: 0=File, 1=Directory, 2=Symlink
    pub extension: Option<String>,  // File extension (without dot), None for directories
    pub metadata_id: Option<i32>,  // Optional - only when user adds metadata
    pub content_id: Option<i32>,  // Optional - for deduplication
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
        belongs_to = "super::location::Entity",
        from = "Column::LocationId",
        to = "super::location::Column::Id"
    )]
    Location,
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

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum EntryKind {
    File = 0,
    Directory = 1,
    Symlink = 2,
}

impl From<i32> for EntryKind {
    fn from(value: i32) -> Self {
        match value {
            0 => EntryKind::File,
            1 => EntryKind::Directory,
            2 => EntryKind::Symlink,
            _ => EntryKind::File, // Default fallback
        }
    }
}

impl From<EntryKind> for i32 {
    fn from(kind: EntryKind) -> Self {
        kind as i32
    }
}

impl Model {
    /// Get the entry kind as enum
    pub fn entry_kind(&self) -> EntryKind {
        EntryKind::from(self.kind)
    }

    /// UUID Assignment Rules:
    /// - Directories: Assign UUID immediately (no content to identify)
    /// - Empty files: Assign UUID immediately (size = 0, no content to hash)
    /// - Regular files: Assign UUID after content identification completes
    pub fn should_assign_uuid_immediately(&self) -> bool {
        self.entry_kind() == EntryKind::Directory || self.size == 0
    }

    /// Check if this entry is ready for sync (has UUID assigned)
    pub fn is_sync_ready(&self) -> bool {
        self.uuid.is_some()
    }
}