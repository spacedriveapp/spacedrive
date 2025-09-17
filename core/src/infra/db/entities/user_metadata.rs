//! User metadata entity

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "user_metadata")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    pub uuid: Uuid,

    // Exactly one of these is set - defines the scope
    pub entry_uuid: Option<Uuid>, // File-specific metadata (higher priority in hierarchy)
    pub content_identity_uuid: Option<Uuid>, // Content-universal metadata (lower priority in hierarchy)

    // All metadata types benefit from scope flexibility
    pub notes: Option<String>,
    pub favorite: bool,
    pub hidden: bool,
    pub custom_data: Json,  // Arbitrary JSON data
    pub created_at: DateTimeUtc,
    pub updated_at: DateTimeUtc,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::entry::Entity",
        from = "Column::EntryUuid",
        to = "super::entry::Column::Uuid"
    )]
    Entry,
    #[sea_orm(
        belongs_to = "super::content_identity::Entity",
        from = "Column::ContentIdentityUuid",
        to = "super::content_identity::Column::Uuid"
    )]
    ContentIdentity,
}

impl Related<super::entry::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Entry.def()
    }
}

impl Related<super::content_identity::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::ContentIdentity.def()
    }
}

impl Related<super::tag::Entity> for Entity {
    fn to() -> RelationDef {
        super::user_metadata_tag::Relation::Tag.def()
    }

    fn via() -> Option<RelationDef> {
        Some(super::user_metadata_tag::Relation::UserMetadata.def().rev())
    }
}

impl ActiveModelBehavior for ActiveModel {}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum MetadataScope {
    Entry,          // File-specific (higher priority)
    Content,        // Content-universal (lower priority)
}

impl Model {
    /// Get the scope of this metadata (entry or content-level)
    pub fn scope(&self) -> Option<MetadataScope> {
        if self.entry_uuid.is_some() {
            Some(MetadataScope::Entry)
        } else if self.content_identity_uuid.is_some() {
            Some(MetadataScope::Content)
        } else {
            None // Invalid state - should be caught by DB constraint
        }
    }

    /// Check if this metadata is entry-scoped
    pub fn is_entry_scoped(&self) -> bool {
        self.entry_uuid.is_some()
    }

    /// Check if this metadata is content-scoped
    pub fn is_content_scoped(&self) -> bool {
        self.content_identity_uuid.is_some()
    }
}