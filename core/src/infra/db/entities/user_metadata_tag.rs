//! User Metadata Semantic Tag entity
//!
//! Enhanced junction table for associating semantic tags with user metadata

use sea_orm::entity::prelude::*;
use sea_orm::{NotSet, Set};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "user_metadata_tag")]
pub struct Model {
	#[sea_orm(primary_key)]
	pub id: i32,
	pub user_metadata_id: i32,
	pub tag_id: i32,

	// Context for this specific tagging instance
	pub applied_context: Option<String>,
	pub applied_variant: Option<String>,
	pub confidence: f32,
	pub source: String, // TagSource enum as string

	// Instance-specific attributes
	pub instance_attributes: Option<Json>, // HashMap<String, serde_json::Value> as JSON

	// Audit and sync
	pub created_at: DateTimeUtc,
	pub updated_at: DateTimeUtc,
	pub device_uuid: Uuid,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
	#[sea_orm(
		belongs_to = "super::user_metadata::Entity",
		from = "Column::UserMetadataId",
		to = "super::user_metadata::Column::Id"
	)]
	UserMetadata,

	#[sea_orm(
		belongs_to = "super::tag::Entity",
		from = "Column::TagId",
		to = "super::tag::Column::Id"
	)]
	Tag,

	#[sea_orm(
		belongs_to = "super::device::Entity",
		from = "Column::DeviceUuid",
		to = "super::device::Column::Uuid"
	)]
	Device,
}

impl Related<super::user_metadata::Entity> for Entity {
	fn to() -> RelationDef {
		Relation::UserMetadata.def()
	}
}

impl Related<super::tag::Entity> for Entity {
	fn to() -> RelationDef {
		Relation::Tag.def()
	}
}

impl Related<super::device::Entity> for Entity {
	fn to() -> RelationDef {
		Relation::Device.def()
	}
}

impl ActiveModelBehavior for ActiveModel {
	fn new() -> Self {
		Self {
			confidence: Set(1.0),
			source: Set("user".to_owned()),
			created_at: Set(chrono::Utc::now()),
			updated_at: Set(chrono::Utc::now()),
			..ActiveModelTrait::default()
		}
	}
}

impl Model {
	/// Get instance attributes as a HashMap
	pub fn get_instance_attributes(&self) -> HashMap<String, serde_json::Value> {
		self.instance_attributes
			.as_ref()
			.and_then(|json| serde_json::from_value(json.clone()).ok())
			.unwrap_or_default()
	}

	/// Set instance attributes from a HashMap
	pub fn set_instance_attributes(&mut self, attributes: HashMap<String, serde_json::Value>) {
		self.instance_attributes = Some(serde_json::to_value(attributes).unwrap().into());
	}

	/// Check if this is a high-confidence tag application
	pub fn is_high_confidence(&self) -> bool {
		self.confidence >= 0.8
	}

	/// Check if this tag was applied by AI
	pub fn is_ai_applied(&self) -> bool {
		self.source == "ai"
	}

	/// Check if this tag was applied by user
	pub fn is_user_applied(&self) -> bool {
		self.source == "user"
	}

	/// Get normalized confidence (0.0-1.0)
	pub fn normalized_confidence(&self) -> f32 {
		self.confidence.clamp(0.0, 1.0)
	}
}

/// Helper enum for tag sources
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TagSource {
	User,
	AI,
	Import,
	Sync,
}

impl TagSource {
	pub fn as_str(&self) -> &'static str {
		match self {
			TagSource::User => "user",
			TagSource::AI => "ai",
			TagSource::Import => "import",
			TagSource::Sync => "sync",
		}
	}

	pub fn from_str(s: &str) -> Option<Self> {
		match s {
			"user" => Some(TagSource::User),
			"ai" => Some(TagSource::AI),
			"import" => Some(TagSource::Import),
			"sync" => Some(TagSource::Sync),
			_ => None,
		}
	}
}
