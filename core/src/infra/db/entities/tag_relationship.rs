//! Tag Relationship entity
//!
//! SeaORM entity for managing hierarchical relationships between semantic tags

use crate::infra::sync::{ChangeType, FKMapping, SharedChangeEntry, Syncable};
use sea_orm::entity::prelude::*;
use sea_orm::{ActiveValue::NotSet, Set};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "tag_relationship")]
pub struct Model {
	#[sea_orm(primary_key)]
	pub id: i32,
	pub parent_tag_id: i32,
	pub child_tag_id: i32,
	pub relationship_type: String, // RelationshipType enum as string
	pub strength: f32,
	pub created_at: DateTimeUtc,

	// Sync fields
	pub uuid: Uuid,
	pub version: i64,
	pub updated_at: DateTimeUtc,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
	#[sea_orm(
		belongs_to = "super::tag::Entity",
		from = "Column::ParentTagId",
		to = "super::tag::Column::Id"
	)]
	ParentTag,

	#[sea_orm(
		belongs_to = "super::tag::Entity",
		from = "Column::ChildTagId",
		to = "super::tag::Column::Id"
	)]
	ChildTag,
}

impl Related<super::tag::Entity> for Entity {
	fn to() -> RelationDef {
		Relation::ParentTag.def()
	}
}

impl ActiveModelBehavior for ActiveModel {
	fn new() -> Self {
		Self {
			relationship_type: Set("parent_child".to_owned()),
			strength: Set(1.0),
			created_at: Set(chrono::Utc::now()),
			..ActiveModelTrait::default()
		}
	}
}

impl Model {
	/// Check if this relationship would create a cycle
	pub fn would_create_cycle(&self) -> bool {
		self.parent_tag_id == self.child_tag_id
	}

	/// Get the relationship strength as a normalized value (0.0-1.0)
	pub fn normalized_strength(&self) -> f32 {
		self.strength.clamp(0.0, 1.0)
	}
}

/// Helper enum for relationship types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RelationshipType {
	ParentChild,
	Synonym,
	Related,
}

impl RelationshipType {
	pub fn as_str(&self) -> &'static str {
		match self {
			RelationshipType::ParentChild => "parent_child",
			RelationshipType::Synonym => "synonym",
			RelationshipType::Related => "related",
		}
	}

	pub fn from_str(s: &str) -> Option<Self> {
		match s {
			"parent_child" => Some(RelationshipType::ParentChild),
			"synonym" => Some(RelationshipType::Synonym),
			"related" => Some(RelationshipType::Related),
			_ => None,
		}
	}
}

// Syncable Implementation
//
// TagRelationship is a SHARED M2M junction table for tag hierarchies.
// Both parent and child are shared tag entities, so relationships are
// synced across all devices using HLC-based replication.

// Register with sync system via inventory (with_rebuild for tag_closure table)
