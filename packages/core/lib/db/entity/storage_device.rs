use chrono::NaiveDateTime;
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

// -------------------------------------
// Entity: StorageDevice
// Represents a folder, drive or cloud
// Two can exist on the same volume, but not on the same path or intersecting paths
// We can create suggestions for these, such as Macintosh HD, Windows C presets, etc.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, DeriveEntityModel, Default, TS)]
#[sea_orm(table_name = "storage_devices")]
#[serde(rename = "StorageDevice")]
#[ts(export)]
// -------------------------------------
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: u32,
    pub name: String,
    pub path: String,
    pub total_capacity: u32,
    pub available_capacity: u32,
    pub is_removable: bool,
    pub is_ejectable: bool,
    pub is_root_filesystem: bool,
    pub is_online: bool,

    pub watch_active: bool,
    #[ts(type = "string")]
    pub date_created: Option<NaiveDateTime>,
    #[ts(type = "string")]
    pub last_indexed: Option<NaiveDateTime>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
