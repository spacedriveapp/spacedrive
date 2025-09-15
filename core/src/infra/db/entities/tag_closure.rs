//! Tag Closure entity
//!
//! SeaORM entity for the closure table that enables efficient hierarchical queries

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "tag_closure")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub ancestor_id: i32,
    #[sea_orm(primary_key, auto_increment = false)]
    pub descendant_id: i32,
    pub depth: i32,
    pub path_strength: f32,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::semantic_tag::Entity",
        from = "Column::AncestorId",
        to = "super::semantic_tag::Column::Id"
    )]
    Ancestor,
    
    #[sea_orm(
        belongs_to = "super::semantic_tag::Entity",
        from = "Column::DescendantId", 
        to = "super::semantic_tag::Column::Id"
    )]
    Descendant,
}

impl Related<super::semantic_tag::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Ancestor.def()
    }
}

impl ActiveModelBehavior for ActiveModel {
    fn new() -> Self {
        Self {
            path_strength: Set(1.0),
            ..ActiveModelTrait::default()
        }
    }
}

impl Model {
    /// Check if this is a self-referential relationship
    pub fn is_self_reference(&self) -> bool {
        self.ancestor_id == self.descendant_id && self.depth == 0
    }
    
    /// Check if this is a direct parent-child relationship
    pub fn is_direct_relationship(&self) -> bool {
        self.depth == 1
    }
    
    /// Get the normalized path strength (0.0-1.0)
    pub fn normalized_path_strength(&self) -> f32 {
        self.path_strength.clamp(0.0, 1.0)
    }
    
    /// Calculate relationship strength based on depth (closer = stronger)
    pub fn calculated_strength(&self) -> f32 {
        if self.depth == 0 {
            1.0 // Self-reference
        } else {
            (1.0 / (self.depth as f32)).min(1.0)
        }
    }
}