//! Semantic Tag entity
//!
//! SeaORM entity for the enhanced semantic tagging system

use crate::infra::sync::{ChangeType, SharedChangeEntry, Syncable};
use sea_orm::entity::prelude::*;
use sea_orm::{ActiveValue::NotSet, Set};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "tag")]
pub struct Model {
	#[sea_orm(primary_key)]
	pub id: i32,
	pub uuid: Uuid,

	// Core identity
	pub canonical_name: String,
	pub display_name: Option<String>,

	// Semantic variants
	pub formal_name: Option<String>,
	pub abbreviation: Option<String>,
	pub aliases: Option<Json>, // Vec<String> as JSON

	// Context and categorization
	pub namespace: Option<String>,
	pub tag_type: String, // TagType enum as string

	// Visual and behavioral properties
	pub color: Option<String>,
	pub icon: Option<String>,
	pub description: Option<String>,

	// Advanced capabilities
	pub is_organizational_anchor: bool,
	pub privacy_level: String, // PrivacyLevel enum as string
	pub search_weight: i32,

	// Compositional attributes
	pub attributes: Option<Json>, // HashMap<String, serde_json::Value> as JSON
	pub composition_rules: Option<Json>, // Vec<CompositionRule> as JSON

	// Metadata
	pub created_at: DateTimeUtc,
	pub updated_at: DateTimeUtc,
	pub created_by_device: Option<Uuid>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
	#[sea_orm(
		has_many = "super::tag_relationship::Entity",
		from = "Column::Id",
		to = "super::tag_relationship::Column::ParentTagId"
	)]
	ParentRelationships,

	#[sea_orm(
		has_many = "super::tag_relationship::Entity",
		from = "Column::Id",
		to = "super::tag_relationship::Column::ChildTagId"
	)]
	ChildRelationships,

	#[sea_orm(
		has_many = "super::user_metadata_tag::Entity",
		from = "Column::Id",
		to = "super::user_metadata_tag::Column::TagId"
	)]
	UserMetadataTags,

	#[sea_orm(
		has_many = "super::tag_usage_pattern::Entity",
		from = "Column::Id",
		to = "super::tag_usage_pattern::Column::TagId"
	)]
	UsagePatterns,
}

impl Related<super::user_metadata_tag::Entity> for Entity {
	fn to() -> RelationDef {
		Relation::UserMetadataTags.def()
	}
}

// Note: We don't implement Related for tag_relationship since it has ambiguous relationships
// (both parent and child). Use the specific relation instead.

impl Related<super::tag_usage_pattern::Entity> for Entity {
	fn to() -> RelationDef {
		Relation::UsagePatterns.def()
	}
}

impl ActiveModelBehavior for ActiveModel {
	fn new() -> Self {
		Self {
			uuid: Set(Uuid::new_v4()),
			tag_type: Set("standard".to_owned()),
			privacy_level: Set("normal".to_owned()),
			search_weight: Set(100),
			is_organizational_anchor: Set(false),
			created_at: Set(chrono::Utc::now()),
			updated_at: Set(chrono::Utc::now()),
			..ActiveModelTrait::default()
		}
	}
}

impl Model {
	/// Get aliases as a vector of strings
	pub fn get_aliases(&self) -> Vec<String> {
		self.aliases
			.as_ref()
			.and_then(|json| serde_json::from_value(json.clone()).ok())
			.unwrap_or_default()
	}

	/// Set aliases from a vector of strings
	pub fn set_aliases(&mut self, aliases: Vec<String>) {
		self.aliases = Some(serde_json::to_value(aliases).unwrap().into());
	}

	/// Get attributes as a HashMap
	pub fn get_attributes(&self) -> HashMap<String, serde_json::Value> {
		self.attributes
			.as_ref()
			.and_then(|json| serde_json::from_value(json.clone()).ok())
			.unwrap_or_default()
	}

	/// Set attributes from a HashMap
	pub fn set_attributes(&mut self, attributes: HashMap<String, serde_json::Value>) {
		self.attributes = Some(serde_json::to_value(attributes).unwrap().into());
	}

	/// Get all possible names this tag can be accessed by
	pub fn get_all_names(&self) -> Vec<String> {
		let mut names = vec![self.canonical_name.clone()];

		if let Some(display) = &self.display_name {
			names.push(display.clone());
		}

		if let Some(formal) = &self.formal_name {
			names.push(formal.clone());
		}

		if let Some(abbrev) = &self.abbreviation {
			names.push(abbrev.clone());
		}

		names.extend(self.get_aliases());

		names
	}

	/// Check if this tag matches the given name in any variant
	pub fn matches_name(&self, name: &str) -> bool {
		self.get_all_names()
			.iter()
			.any(|n| n.eq_ignore_ascii_case(name))
	}

	/// Check if this tag should be hidden from normal search results
	pub fn is_searchable(&self) -> bool {
		self.privacy_level == "normal"
	}

	/// Get the fully qualified name including namespace
	pub fn get_qualified_name(&self) -> String {
		match &self.namespace {
			Some(ns) => format!("{}::{}", ns, self.canonical_name),
			None => self.canonical_name.clone(),
		}
	}
}

// Syncable Implementation
//
// Tags are SHARED resources using HLC-ordered log-based replication with union merge.
// Multiple tags with the same canonical_name are preserved if they have different UUIDs.
// This supports polymorphic naming where context (namespace) disambiguates meaning.
impl Syncable for Model {
	const SYNC_MODEL: &'static str = "tag";

	fn sync_id(&self) -> Uuid {
		self.uuid
	}

	fn version(&self) -> i64 {
		// TODO: Add version field to tags table via migration
		// Migration SQL:
		//   ALTER TABLE tag ADD COLUMN version INTEGER NOT NULL DEFAULT 1;
		// For now, return a default value
		1
	}

	fn exclude_fields() -> Option<&'static [&'static str]> {
		Some(&["id", "created_at", "updated_at"])
	}

	fn sync_depends_on() -> &'static [&'static str] {
		&[] // Tag is shared, no FK dependencies
	}

	/// Apply shared change with union merge conflict resolution.
	/// Different UUIDs with same canonical_name coexist (polymorphic naming).
	async fn apply_shared_change(
		entry: SharedChangeEntry,
		db: &DatabaseConnection,
	) -> Result<(), sea_orm::DbErr> {
		match entry.change_type {
			ChangeType::Insert | ChangeType::Update => {
				// Deserialize incoming tag
				let tag: Self = serde_json::from_value(entry.data).map_err(|e| {
					sea_orm::DbErr::Custom(format!("Tag deserialization failed: {}", e))
				})?;

				// Build ActiveModel for upsert
				let active = ActiveModel {
					id: NotSet, // Database PK, not synced
					uuid: Set(tag.uuid),
					canonical_name: Set(tag.canonical_name),
					display_name: Set(tag.display_name),
					formal_name: Set(tag.formal_name),
					abbreviation: Set(tag.abbreviation),
					aliases: Set(tag.aliases),
					namespace: Set(tag.namespace),
					tag_type: Set(tag.tag_type),
					color: Set(tag.color),
					icon: Set(tag.icon),
					description: Set(tag.description),
					is_organizational_anchor: Set(tag.is_organizational_anchor),
					privacy_level: Set(tag.privacy_level),
					search_weight: Set(tag.search_weight),
					attributes: Set(tag.attributes),
					composition_rules: Set(tag.composition_rules),
					created_at: Set(chrono::Utc::now().into()), // Local timestamp
					updated_at: Set(chrono::Utc::now().into()), // Local timestamp
					created_by_device: Set(tag.created_by_device),
				};

				// Idempotent upsert: insert or update based on UUID
				// Union merge: different UUIDs = different tags (even with same canonical_name)
				Entity::insert(active)
					.on_conflict(
						sea_orm::sea_query::OnConflict::column(Column::Uuid)
							.update_columns([
								Column::CanonicalName,
								Column::DisplayName,
								Column::FormalName,
								Column::Abbreviation,
								Column::Aliases,
								Column::Namespace,
								Column::TagType,
								Column::Color,
								Column::Icon,
								Column::Description,
								Column::IsOrganizationalAnchor,
								Column::PrivacyLevel,
								Column::SearchWeight,
								Column::Attributes,
								Column::CompositionRules,
								Column::UpdatedAt,
								Column::CreatedByDevice,
							])
							.to_owned(),
					)
					.exec(db)
					.await?;
			}

			ChangeType::Delete => {
				// Delete by UUID (tombstone record)
				Entity::delete_many()
					.filter(Column::Uuid.eq(entry.record_uuid))
					.exec(db)
					.await?;
			}
		}

		Ok(())
	}
}

/// Helper enum for tag types (for validation)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TagType {
	Standard,
	Organizational,
	Privacy,
	System,
}

impl TagType {
	pub fn as_str(&self) -> &'static str {
		match self {
			TagType::Standard => "standard",
			TagType::Organizational => "organizational",
			TagType::Privacy => "privacy",
			TagType::System => "system",
		}
	}

	pub fn from_str(s: &str) -> Option<Self> {
		match s {
			"standard" => Some(TagType::Standard),
			"organizational" => Some(TagType::Organizational),
			"privacy" => Some(TagType::Privacy),
			"system" => Some(TagType::System),
			_ => None,
		}
	}
}

/// Helper enum for privacy levels (for validation)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PrivacyLevel {
	Normal,
	Archive,
	Hidden,
}

impl PrivacyLevel {
	pub fn as_str(&self) -> &'static str {
		match self {
			PrivacyLevel::Normal => "normal",
			PrivacyLevel::Archive => "archive",
			PrivacyLevel::Hidden => "hidden",
		}
	}

	pub fn from_str(s: &str) -> Option<Self> {
		match s {
			"normal" => Some(PrivacyLevel::Normal),
			"archive" => Some(PrivacyLevel::Archive),
			"hidden" => Some(PrivacyLevel::Hidden),
			_ => None,
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_tag_syncable() {
		let tag = Model {
			id: 1,
			uuid: Uuid::new_v4(),
			canonical_name: "vacation".to_string(),
			display_name: Some("Vacation".to_string()),
			formal_name: None,
			abbreviation: None,
			aliases: None,
			namespace: Some("travel".to_string()),
			tag_type: "standard".to_string(),
			color: Some("#FF5733".to_string()),
			icon: None,
			description: Some("Travel vacation photos".to_string()),
			is_organizational_anchor: false,
			privacy_level: "normal".to_string(),
			search_weight: 100,
			attributes: None,
			composition_rules: None,
			created_at: chrono::Utc::now().into(),
			updated_at: chrono::Utc::now().into(),
			created_by_device: Some(Uuid::new_v4()),
		};

		// Test sync methods
		assert_eq!(Model::SYNC_MODEL, "tag");
		assert_eq!(tag.sync_id(), tag.uuid);
		assert_eq!(tag.version(), 1);

		// Test JSON serialization
		let json = tag.to_sync_json().unwrap();

		// Excluded fields
		assert!(json.get("id").is_none());
		assert!(json.get("created_at").is_none());
		assert!(json.get("updated_at").is_none());

		// Fields that SHOULD sync
		assert!(json.get("uuid").is_some());
		assert!(json.get("canonical_name").is_some());
		assert!(json.get("display_name").is_some());
		assert!(json.get("namespace").is_some());
		assert!(json.get("tag_type").is_some());
		assert!(json.get("color").is_some());
		assert!(json.get("description").is_some());
		assert!(json.get("privacy_level").is_some());
		assert!(json.get("search_weight").is_some());
		assert!(json.get("created_by_device").is_some());

		assert_eq!(
			json.get("canonical_name").unwrap().as_str().unwrap(),
			"vacation"
		);
		assert_eq!(
			json.get("display_name").unwrap().as_str().unwrap(),
			"Vacation"
		);
	}

	#[test]
	fn test_tag_polymorphic_naming() {
		// Test that tags with same canonical_name but different UUIDs are different
		let uuid1 = Uuid::new_v4();
		let uuid2 = Uuid::new_v4();

		let tag1 = Model {
			id: 1,
			uuid: uuid1,
			canonical_name: "vacation".to_string(),
			namespace: Some("travel".to_string()),
			display_name: None,
			formal_name: None,
			abbreviation: None,
			aliases: None,
			tag_type: "standard".to_string(),
			color: None,
			icon: None,
			description: None,
			is_organizational_anchor: false,
			privacy_level: "normal".to_string(),
			search_weight: 100,
			attributes: None,
			composition_rules: None,
			created_at: chrono::Utc::now().into(),
			updated_at: chrono::Utc::now().into(),
			created_by_device: None,
		};

		let tag2 = Model {
			uuid: uuid2,
			namespace: Some("work".to_string()),
			..tag1.clone()
		};

		// Different UUIDs = different tags (polymorphic naming)
		assert_ne!(tag1.uuid, tag2.uuid);
		assert_eq!(tag1.canonical_name, tag2.canonical_name);

		// Qualified names are different
		assert_eq!(tag1.get_qualified_name(), "travel::vacation");
		assert_eq!(tag2.get_qualified_name(), "work::vacation");
	}

	// Note: apply_shared_change requires database setup, tested in integration tests
	// See core/tests/sync/tag_sync_test.rs
}
