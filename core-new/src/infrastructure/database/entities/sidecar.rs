use chrono::{DateTime, Utc};
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "sidecars")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    
    pub content_uuid: Uuid,
    
    pub kind: String,
    
    pub variant: String,
    
    pub format: String,
    
    pub rel_path: String,
    
    pub size: i64,
    
    pub checksum: Option<String>,
    
    pub status: String,
    
    pub source: Option<String>,
    
    pub version: i32,
    
    pub created_at: DateTime<Utc>,
    
    pub updated_at: DateTime<Utc>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::content_identity::Entity",
        from = "Column::ContentUuid",
        to = "super::content_identity::Column::Uuid"
    )]
    ContentIdentity,
}

impl Related<super::content_identity::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::ContentIdentity.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}