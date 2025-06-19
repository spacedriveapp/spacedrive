//! Content identity entity

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "content_identities")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    pub uuid: Uuid,
    pub full_hash: Option<String>,
    pub cas_id: String,
    pub cas_version: i16,
    pub mime_type_id: Option<i32>,
    pub kind_id: i32,  // ContentKind foreign key
    pub media_data: Option<Json>,  // MediaData as JSON
    pub text_content: Option<String>,
    pub total_size: i64,
    pub entry_count: i32,
    pub first_seen_at: DateTimeUtc,
    pub last_verified_at: DateTimeUtc,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::entry::Entity")]
    Entries,
    #[sea_orm(
        belongs_to = "super::content_kind::Entity",
        from = "Column::KindId",
        to = "super::content_kind::Column::Id"
    )]
    ContentKind,
    #[sea_orm(
        belongs_to = "super::mime_type::Entity",
        from = "Column::MimeTypeId",
        to = "super::mime_type::Column::Id"
    )]
    MimeType,
}

impl Related<super::entry::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Entries.def()
    }
}

impl Related<super::content_kind::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::ContentKind.def()
    }
}

impl Related<super::mime_type::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::MimeType.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}