use serde::{Deserialize, Serialize};

use crate::sync::{
  crdt::{PropertyOperation, Replicate},
  engine::SyncContext,
};

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct File {
  pub id: i32,
  pub uuid: String,
  pub name: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct FileCreate {
  pub uuid: String,
  pub name: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum FileUpdate {
  Name(String),
}

#[async_trait::async_trait]
impl PropertyOperation for File {
  type Create = FileCreate;
  type Update = FileUpdate;

  async fn create(_data: Self::Create, _ctx: SyncContext) {}
  async fn update(_data: Self::Update, _ctx: SyncContext) {}
  async fn delete(_ctx: SyncContext) {}
}

#[async_trait::async_trait]
impl Replicate for File {
  type Create = FileCreate;

  async fn create(_data: Self::Create, _ctx: SyncContext) {}
  async fn delete(_ctx: SyncContext) {}
}
