use chrono::NaiveDateTime;
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

// -------------------------------------
// Entity: File
// Represents an item discovered on the filesystem, can be a file or directory.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, DeriveEntityModel, TS)]
#[sea_orm(table_name = "files")]
#[serde(rename = "File")]
#[ts(export)]
// -------------------------------------
pub struct Model {
    // identity
    #[sea_orm(primary_key)]
    pub id: u32,
    #[ts(type = "json")]
    pub materialized_path: Json,
    #[sea_orm(nullable)]
    pub parent_id: Option<u32>,
    // pub buffer_checksum: String,
    #[sea_orm(unique)]
    pub meta_integrity_hash: String,
    pub sampled_byte_integrity_hash: Option<String>,
    pub byte_integrity_hash: Option<String>,

    // pub uri: String,
    pub is_dir: bool,
    // metadata
    pub name: String,
    pub extension: String,
    pub size_in_bytes: String,
    // date
    #[ts(type = "string")]
    pub date_created: Option<NaiveDateTime>,
    #[ts(type = "string")]
    pub date_modified: Option<NaiveDateTime>,
    // #[ts(type = "string")]
    // pub date_indexed: Option<NaiveDateTime>,
    pub encryption: Encryption,
    // ownership
    #[sea_orm(nullable)]
    pub ipfs_id: Option<String>,

    #[sea_orm(nullable)]
    pub location_id: Option<u32>,

    #[sea_orm(nullable)]
    pub capture_device_id: Option<u32>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, EnumIter, DeriveActiveEnum, TS)]
#[sea_orm(rs_type = "i32", db_type = "Integer")]
#[ts(export)]
pub enum Encryption {
    #[sea_orm(num_value = 0)]
    None,
    #[sea_orm(num_value = 1)]
    AES128,
    #[sea_orm(num_value = 2)]
    AES192,
    #[sea_orm(num_value = 3)]
    AES256,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::locations::Entity",
        from = "Column::LocationId",
        to = "super::locations::Column::Id"
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
