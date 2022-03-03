use chrono::NaiveDateTime;
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

// -------------------------------------
// Entity: Location
// Represents a folder, drive or cloud
// Two can exist on the same volume, but not on the same path or intersecting paths
// We can create suggestions for these, such as Macintosh HD, Windows C presets, etc.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, DeriveEntityModel, Default, TS)]
#[sea_orm(table_name = "locations")]
#[serde(rename = "Location")]
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
    pub library_id: u32,
    pub client_id: u32,

    #[ts(type = "string")]
    pub date_created: Option<NaiveDateTime>,
    #[ts(type = "string")]
    pub last_indexed: Option<NaiveDateTime>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::library::Entity",
        from = "Column::LibraryId",
        to = "super::library::Column::Id"
    )]
    Library,
    #[sea_orm(
        belongs_to = "super::client::Entity",
        from = "Column::ClientId",
        to = "super::client::Column::Id"
    )]
    Client,
    // TODO: fix??????
    // #[sea_orm(has_many = "super::location_paths::Entity")]
    // LocationPaths,
}

impl ActiveModelBehavior for ActiveModel {}
