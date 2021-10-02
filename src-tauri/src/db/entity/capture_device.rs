use chrono::NaiveDateTime;
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

// -------------------------------------
// Entity: Space
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, DeriveEntityModel, Default)]
#[sea_orm(table_name = "capture_devices")]
// -------------------------------------
pub struct Model {
  #[sea_orm(primary_key)]
  pub id: u32,
  pub name: String,
  pub date_created: Option<NaiveDateTime>,
  pub date_modified: Option<NaiveDateTime>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
