//! Content kind entity (lookup table)

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "content_kinds")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: i32,
    pub name: String,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::content_identity::Entity")]
    ContentIdentities,
}

impl Related<super::content_identity::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::ContentIdentities.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}