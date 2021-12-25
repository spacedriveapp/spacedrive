use chrono::NaiveDateTime;
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

// -------------------------------------
// Entity: Directory
// Represents an item discovered on the filesystem
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, DeriveEntityModel, Default)]
#[sea_orm(table_name = "libraries")]
// -------------------------------------
pub struct Model {
  // identity
  #[sea_orm(primary_key)]
  pub id: u32,
  pub name: String,
  pub is_primary: bool,
  pub remote_id: Option<String>,
  pub total_file_count: Option<u32>,
  pub total_bytes_used: Option<String>,
  pub total_byte_capacity: Option<String>,
  pub date_created: Option<NaiveDateTime>,
  pub timezone: Option<String>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
