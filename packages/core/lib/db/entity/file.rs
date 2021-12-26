use chrono::NaiveDateTime;
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

// -------------------------------------
// Entity: File
// Represents an item discovered on the filesystem
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, DeriveEntityModel, Default, TS)]
#[sea_orm(table_name = "files")]
#[serde(rename = "File")]
#[ts(export)]
// -------------------------------------
pub struct Model {
    // identity
    #[sea_orm(primary_key)]
    pub id: u32,
    // pub buffer_checksum: String,
    #[sea_orm(unique)]
    pub meta_checksum: String,
    pub uri: String,
    pub is_dir: bool,
    // metadata
    pub name: String,
    pub extension: String,
    pub size_in_bytes: String,
    pub library_id: u32,
    // date
    #[ts(type = "string")]
    pub date_created: Option<NaiveDateTime>,
    #[ts(type = "string")]
    pub date_modified: Option<NaiveDateTime>,
    #[ts(type = "string")]
    pub date_indexed: Option<NaiveDateTime>,
    // #[sea_orm(column_type = "Int")]
    // pub encryption: crypto::Encryption,
    // ownership
    #[sea_orm(nullable)]
    pub ipfs_id: Option<String>,

    #[sea_orm(nullable)]
    pub storage_device_id: Option<u32>,

    #[sea_orm(nullable)]
    pub capture_device_id: Option<u32>,

    #[sea_orm(nullable)]
    pub parent_id: Option<u32>,
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
        belongs_to = "super::storage_device::Entity",
        from = "Column::StorageDeviceId",
        to = "super::storage_device::Column::Id"
    )]
    StorageDevice,

    #[sea_orm(
        belongs_to = "super::capture_device::Entity",
        from = "Column::CaptureDeviceId",
        to = "super::capture_device::Column::Id"
    )]
    CaptureDevice,

    #[sea_orm(belongs_to = "Entity", from = "Column::ParentId", to = "Column::Id")]
    ParentFile,
}

impl ActiveModelBehavior for ActiveModel {}
