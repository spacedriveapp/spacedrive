//! Entry closure table entity

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "entry_closure")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub ancestor_id: i32,
    #[sea_orm(primary_key, auto_increment = false)]
    pub descendant_id: i32,
    pub depth: i32,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::entry::Entity",
        from = "Column::AncestorId",
        to = "super::entry::Column::Id"
    )]
    Ancestor,
    #[sea_orm(
        belongs_to = "super::entry::Entity",
        from = "Column::DescendantId",
        to = "super::entry::Column::Id"
    )]
    Descendant,
}

impl Related<super::entry::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Ancestor.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}

impl Model {
    /// Check if this is a self-referential relationship
    pub fn is_self_reference(&self) -> bool {
        self.ancestor_id == self.descendant_id && self.depth == 0
    }
    
    /// Check if this is a direct parent-child relationship
    pub fn is_direct_relationship(&self) -> bool {
        self.depth == 1
    }
}