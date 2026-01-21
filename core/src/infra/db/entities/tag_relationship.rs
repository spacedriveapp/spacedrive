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

	async fn post_backfill_rebuild(db: &DatabaseConnection) -> Result<(), sea_orm::DbErr> {
		use super::tag_closure;
		use sea_orm::{ConnectionTrait, DbBackend, PaginatorTrait, Statement};

		tracing::debug!("Starting tag_closure rebuild from tag_relationships...");

		// Clear existing tag_closure table
		tag_closure::Entity::delete_many().exec(db).await?;

		// 1. Insert self-references for all tags (depth 0)
		db.execute(Statement::from_sql_and_values(
			DbBackend::Sqlite,
			r#"
			INSERT INTO tag_closure (ancestor_id, descendant_id, depth, path_strength)
			SELECT id, id, 0, 1.0 FROM tag
			"#,
			vec![],
		))
		.await
		.map_err(|e| sea_orm::DbErr::Custom(format!("Failed to insert self-refs: {}", e)))?;

		// 2. Insert direct relationships from tag_relationship (depth 1)
		db.execute(Statement::from_sql_and_values(
			DbBackend::Sqlite,
			r#"
			INSERT OR IGNORE INTO tag_closure (ancestor_id, descendant_id, depth, path_strength)
			SELECT parent_tag_id, child_tag_id, 1, strength
			FROM tag_relationship
			"#,
			vec![],
		))
		.await
		.map_err(|e| sea_orm::DbErr::Custom(format!("Failed to insert direct rels: {}", e)))?;

		// 3. Recursively build transitive relationships
		let mut iteration = 0;
		loop {
			let result = db
				.execute(Statement::from_sql_and_values(
					DbBackend::Sqlite,
					r#"
					INSERT OR IGNORE INTO tag_closure (ancestor_id, descendant_id, depth, path_strength)
					SELECT tc1.ancestor_id, tc2.descendant_id, tc1.depth + tc2.depth, tc1.path_strength * tc2.path_strength
					FROM tag_closure tc1
					INNER JOIN tag_closure tc2 ON tc1.descendant_id = tc2.ancestor_id
					WHERE tc1.depth > 0 OR tc2.depth > 0
					  AND NOT EXISTS (
						SELECT 1 FROM tag_closure
						WHERE ancestor_id = tc1.ancestor_id
						  AND descendant_id = tc2.descendant_id
					  )
					"#,
					vec![],
				))
				.await
				.map_err(|e| {
					sea_orm::DbErr::Custom(format!("Failed to build transitive rels: {}", e))
				})?;

			iteration += 1;
			let rows_affected = result.rows_affected();

			tracing::debug!(
				iteration = iteration,
				rows_inserted = rows_affected,
				"tag_closure rebuild iteration"
			);

			if rows_affected == 0 {
				break;
			}

			if iteration > 100 {
				return Err(sea_orm::DbErr::Custom(
					"tag_closure rebuild exceeded max iterations - possible cycle".to_string(),
				));
			}
		}

		let total = tag_closure::Entity::find().count(db).await?;

		tracing::debug!(
			iterations = iteration,
			total_relationships = total,
			"tag_closure rebuild complete"
		);

		Ok(())
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

// Register with sync system via inventory (with_rebuild for tag_closure table)
crate::register_syncable_shared!(Model, "tag_relationship", "tag_relationship", with_rebuild);
