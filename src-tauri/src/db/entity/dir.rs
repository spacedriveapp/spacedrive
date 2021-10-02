use crate::crypto;
use chrono::NaiveDateTime;
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

// -------------------------------------
// Entity: Directory
// Represents an item discovered on the filesystem
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, DeriveEntityModel, Default)]
#[sea_orm(table_name = "directories")]
// -------------------------------------
pub struct Directory {
  // identity
  #[sea_orm(primary_key)]
  pub id: u32,
  pub name: String,
  pub uri String,
  // calculations
  pub calculated_size_in_bytes: Option<String>,
  pub calculated_file_count: Option<u32>,
  // ownership
  pub storage_device_id: Option<u32>,
  pub parent_directory_id: Option<u32>,
  // date
  pub date_created: DateTime<Utc>,
  pub date_modified: DateTime<Utc>,
  pub date_indexed: DateTime<Utc>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}