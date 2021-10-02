use chrono::NaiveDateTime;
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

// -------------------------------------
// Entity: Directory
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, DeriveEntityModel, Default)]
#[sea_orm(table_name = "directories")]
// -------------------------------------
pub struct Model {
  // identity
  #[sea_orm(primary_key)]
  pub id: u32,
  pub name: String,
  pub uri: String,
  pub watch: bool,
  pub calculated_size_in_bytes: Option<String>,
  pub calculated_file_count: Option<u32>,
  pub date_created: Option<NaiveDateTime>,
  pub date_modified: Option<NaiveDateTime>,
  pub date_indexed: Option<NaiveDateTime>,
  pub library_id: u32,
  pub parent_directory_id: Option<u32>,
  pub storage_device_id: Option<u32>,
}

#[derive(Copy, Clone, Debug, EnumIter)]
pub enum Relation {
  StorageDevice,
  Library,
}

impl RelationTrait for Relation {
  fn def(&self) -> RelationDef {
    match self {
      Self::Library => Entity::belongs_to(super::library::Entity)
        .from(Column::LibraryId)
        .to(super::library::Column::Id)
        .into(),
      Self::StorageDevice => Entity::belongs_to(super::storage_device::Entity)
        .from(Column::StorageDeviceId)
        .to(super::storage_device::Column::Id)
        .into(),
    }
  }
}

impl Related<super::library::Entity> for Entity {
  fn to() -> RelationDef {
    Relation::Library.def()
  }
}

impl Related<super::storage_device::Entity> for Entity {
  fn to() -> RelationDef {
    Relation::StorageDevice.def()
  }
}

impl ActiveModelBehavior for ActiveModel {}
