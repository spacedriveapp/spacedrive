use crate::crypto;
use chrono::NaiveDateTime;
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

// -------------------------------------
// Entity: File
// Represents an item discovered on the filesystem
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, DeriveEntityModel, Default)]
#[sea_orm(table_name = "files")]
// -------------------------------------
pub struct Model {
  // identity
  #[sea_orm(primary_key)]
  pub id: u32,
  // pub buffer_checksum: String,
  #[sea_orm(unique)]
  pub meta_checksum: String,
  pub uri: String,
  // date
  pub date_created: Option<NaiveDateTime>,
  pub date_modified: Option<NaiveDateTime>,
  pub date_indexed: Option<NaiveDateTime>,
  // metadata
  pub name: String,
  pub extension: String,
  pub size_in_bytes: String,
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
  pub parent_file_id: Option<u32>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
