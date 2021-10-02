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
  pub library_id: String,
  pub directory_id: String,
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

#[derive(Copy, Clone, Debug, EnumIter)]
pub enum Relation {
  Library,
  Directory,
  StorageDevice,
  CaptureDevice,
  ParentFile,
}

impl RelationTrait for Relation {
  fn def(&self) -> RelationDef {
    match self {
      Self::Library => Entity::belongs_to(super::library::Entity)
        .from(Column::LibraryId)
        .to(super::library::Column::Id)
        .into(),
      Self::Directory => Entity::belongs_to(super::dir::Entity)
        .from(Column::DirectoryId)
        .to(super::dir::Column::Id)
        .into(),
      Self::StorageDevice => Entity::belongs_to(super::storage_device::Entity)
        .from(Column::StorageDeviceId)
        .to(super::storage_device::Column::Id)
        .into(),
      Self::CaptureDevice => Entity::belongs_to(super::capture_device::Entity)
        .from(Column::CaptureDeviceId)
        .to(super::capture_device::Column::Id)
        .into(),
      Self::ParentFile => Entity::belongs_to(Entity)
        .from(Column::ParentFileId)
        .to(Column::Id)
        .into(),
    }
  }
}
impl Related<super::library::Entity> for Entity {
  fn to() -> RelationDef {
    Relation::Library.def()
  }
}
impl Related<super::dir::Entity> for Entity {
  fn to() -> RelationDef {
    Relation::Directory.def()
  }
}
impl Related<super::storage_device::Entity> for Entity {
  fn to() -> RelationDef {
    Relation::StorageDevice.def()
  }
}
impl Related<super::capture_device::Entity> for Entity {
  fn to() -> RelationDef {
    Relation::CaptureDevice.def()
  }
}
impl Related<Entity> for Entity {
  fn to() -> RelationDef {
    Relation::ParentFile.def()
  }
}

impl ActiveModelBehavior for ActiveModel {}
