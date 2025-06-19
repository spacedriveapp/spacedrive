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
    pub mime_type: Option<String>,
    pub kind: String,  // ContentKind as string
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
}

impl Related<super::entry::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Entries.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}