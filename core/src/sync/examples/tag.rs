// this is a test for sync
use serde::{Deserialize, Serialize};

use crate::sync::{crdt::PropertyOperation, engine::SyncContext};

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Tag {
  pub id: String,
  pub uuid: String,
  pub name: String,
  pub description: String,
  pub color: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct TagCreate {
  pub name: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum TagUpdate {
  Name(String),
  Description(String),
  Color(String),
}

#[async_trait::async_trait]
impl PropertyOperation for Tag {
  type Create = TagCreate;
  type Update = TagUpdate;

  async fn create(_data: Self::Create, ctx: SyncContext) {}
  async fn update(_data: Self::Update, ctx: SyncContext) {}
  async fn delete(ctx: SyncContext) {}
}
