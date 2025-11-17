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
impl Syncable for Model {
	const SYNC_MODEL: &'static str = "tag_relationship";

	fn sync_id(&self) -> Uuid {
		self.uuid
	}

	fn version(&self) -> i64 {
		self.version
	}

	fn exclude_fields() -> Option<&'static [&'static str]> {
		Some(&["id", "created_at"])
	}

	fn sync_depends_on() -> &'static [&'static str] {
		&["tag"]
	}

	fn foreign_key_mappings() -> Vec<FKMapping> {
		vec![
			FKMapping::new("parent_tag_id", "tag"),
			FKMapping::new("child_tag_id", "tag"),
		]
	}

	async fn query_for_sync(
		_device_id: Option<Uuid>,
		since: Option<chrono::DateTime<chrono::Utc>>,
		_cursor: Option<(chrono::DateTime<chrono::Utc>, Uuid)>,
		batch_size: usize,
		db: &DatabaseConnection,
	) -> Result<Vec<(Uuid, serde_json::Value, chrono::DateTime<chrono::Utc>)>, sea_orm::DbErr> {
		use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QuerySelect};

		let mut query = Entity::find();

		if let Some(since_time) = since {
			query = query.filter(Column::UpdatedAt.gte(since_time));
		}

		query = query.limit(batch_size as u64);

		let results = query.all(db).await?;

		let mut sync_results = Vec::new();
		for rel in results {
			let mut json = match rel.to_sync_json() {
				Ok(j) => j,
				Err(e) => {
					tracing::warn!(error = %e, uuid = %rel.uuid, "Failed to serialize tag_relationship for sync");
					continue;
				}
			};

			// Convert FK integer IDs to UUIDs
			let mut skip_record = false;
			for fk in <Model as Syncable>::foreign_key_mappings() {
				if let Err(e) =
					crate::infra::sync::fk_mapper::convert_fk_to_uuid(&mut json, &fk, db).await
				{
					tracing::error!(
						error = %e,
						uuid = %rel.uuid,
						fk_field = %fk.local_field,
						"Failed to convert FK to UUID for tag_relationship, skipping record"
					);
					skip_record = true;
					break;
				}
			}

			if !skip_record {
				sync_results.push((rel.uuid, json, rel.updated_at));
			}
		}

		Ok(sync_results)
	}

	async fn apply_shared_change(
		entry: SharedChangeEntry,
		db: &DatabaseConnection,
	) -> Result<(), sea_orm::DbErr> {
		match entry.change_type {
			ChangeType::Insert | ChangeType::Update => {
				// Map UUIDs to local IDs for FK fields
				use crate::infra::sync::fk_mapper;
				let data =
					fk_mapper::map_sync_json_to_local(entry.data, Self::foreign_key_mappings(), db)
						.await
						.map_err(|e| sea_orm::DbErr::Custom(format!("FK mapping failed: {}", e)))?;

				let data = data.as_object().ok_or_else(|| {
					sea_orm::DbErr::Custom("TagRelationship data is not an object".to_string())
				})?;

				let uuid: Uuid = serde_json::from_value(
					data.get("uuid")
						.ok_or_else(|| sea_orm::DbErr::Custom("Missing uuid".to_string()))?
						.clone(),
				)
				.map_err(|e| sea_orm::DbErr::Custom(format!("Invalid uuid: {}", e)))?;

				let parent_tag_id: i32 = serde_json::from_value(
					data.get("parent_tag_id")
						.ok_or_else(|| sea_orm::DbErr::Custom("Missing parent_tag_id".to_string()))?
						.clone(),
				)
				.map_err(|e| sea_orm::DbErr::Custom(format!("Invalid parent_tag_id: {}", e)))?;

				let child_tag_id: i32 = serde_json::from_value(
					data.get("child_tag_id")
						.ok_or_else(|| sea_orm::DbErr::Custom("Missing child_tag_id".to_string()))?
						.clone(),
				)
				.map_err(|e| sea_orm::DbErr::Custom(format!("Invalid child_tag_id: {}", e)))?;

				let relationship_type: String = serde_json::from_value(
					data.get("relationship_type")
						.ok_or_else(|| {
							sea_orm::DbErr::Custom("Missing relationship_type".to_string())
						})?
						.clone(),
				)
				.map_err(|e| sea_orm::DbErr::Custom(format!("Invalid relationship_type: {}", e)))?;

				let strength: f32 = serde_json::from_value(
					data.get("strength")
						.ok_or_else(|| sea_orm::DbErr::Custom("Missing strength".to_string()))?
						.clone(),
				)
				.map_err(|e| sea_orm::DbErr::Custom(format!("Invalid strength: {}", e)))?;

				let version: i64 = serde_json::from_value(
					data.get("version")
						.ok_or_else(|| sea_orm::DbErr::Custom("Missing version".to_string()))?
						.clone(),
				)
				.map_err(|e| sea_orm::DbErr::Custom(format!("Invalid version: {}", e)))?;

				let active = ActiveModel {
					id: NotSet,
					parent_tag_id: Set(parent_tag_id),
					child_tag_id: Set(child_tag_id),
					relationship_type: Set(relationship_type),
					strength: Set(strength),
					uuid: Set(uuid),
					version: Set(version),
					created_at: Set(chrono::Utc::now()),
					updated_at: Set(chrono::Utc::now()),
				};

				Entity::insert(active)
					.on_conflict(
						sea_orm::sea_query::OnConflict::column(Column::Uuid)
							.update_columns([
								Column::ParentTagId,
								Column::ChildTagId,
								Column::RelationshipType,
								Column::Strength,
								Column::Version,
								Column::UpdatedAt,
							])
							.to_owned(),
					)
					.exec(db)
					.await?;
			}

			ChangeType::Delete => {
				Entity::delete_many()
					.filter(Column::Uuid.eq(entry.record_uuid))
					.exec(db)
					.await?;
			}
		}

		Ok(())
	}
}
