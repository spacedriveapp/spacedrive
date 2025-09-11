//! Device entity

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "devices")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    pub uuid: Uuid,
    pub name: String,
    pub os: String,
    pub os_version: Option<String>,
    pub hardware_model: Option<String>,
    pub network_addresses: Json,  // Vec<String> as JSON
    pub is_online: bool,
    pub last_seen_at: DateTimeUtc,
    pub capabilities: Json,  // DeviceCapabilities as JSON
    pub sync_leadership: Json,  // HashMap<Uuid, SyncRole> as JSON
    pub created_at: DateTimeUtc,
    pub updated_at: DateTimeUtc,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::location::Entity")]
    Locations,
}

impl Related<super::location::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Locations.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}