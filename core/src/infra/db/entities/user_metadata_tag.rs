//! User Metadata Semantic Tag entity
//!
//! Enhanced junction table for associating semantic tags with user metadata

use crate::infra::sync::{ChangeType, FKMapping, SharedChangeEntry, Syncable};
use sea_orm::entity::prelude::*;
use sea_orm::{ActiveValue::NotSet, Set};
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
	pub uuid: Uuid,
	pub version: i64,
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

// Syncable Implementation
//
// UserMetadataTag is a SHARED M2M junction table with dynamic ownership enforcement.
// When the parent user_metadata is entry-scoped (device-owned), only the owning device
// can modify the tag associations. When content-scoped (shared), all devices can modify.
impl Syncable for Model {
	const SYNC_MODEL: &'static str = "user_metadata_tag";

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
		&["user_metadata", "tag"]
	}

	fn foreign_key_mappings() -> Vec<FKMapping> {
		vec![
			FKMapping::new("user_metadata_id", "user_metadata"),
			FKMapping::new("tag_id", "tag"),
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
		for umt in results {
			let mut json = match umt.to_sync_json() {
				Ok(j) => j,
				Err(e) => {
					tracing::warn!(error = %e, uuid = %umt.uuid, "Failed to serialize user_metadata_tag for sync");
					continue;
				}
			};

			// Convert FK integer IDs to UUIDs
			for fk in <Model as Syncable>::foreign_key_mappings() {
				if let Err(e) =
					crate::infra::sync::fk_mapper::convert_fk_to_uuid(&mut json, &fk, db).await
				{
					tracing::warn!(
						error = %e,
						uuid = %umt.uuid,
						"Failed to convert FK to UUID for user_metadata_tag"
					);
					continue;
				}
			}

			sync_results.push((umt.uuid, json, umt.updated_at));
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
				let data = fk_mapper::map_sync_json_to_local(entry.data, Self::foreign_key_mappings(), db)
					.await
					.map_err(|e| sea_orm::DbErr::Custom(format!("FK mapping failed: {}", e)))?;

				let data = data.as_object().ok_or_else(|| {
					sea_orm::DbErr::Custom("UserMetadataTag data is not an object".to_string())
				})?;

				let uuid: Uuid = serde_json::from_value(
					data.get("uuid")
						.ok_or_else(|| sea_orm::DbErr::Custom("Missing uuid".to_string()))?
						.clone(),
				)
				.map_err(|e| sea_orm::DbErr::Custom(format!("Invalid uuid: {}", e)))?;

				let user_metadata_id: i32 = serde_json::from_value(
					data.get("user_metadata_id")
						.ok_or_else(|| sea_orm::DbErr::Custom("Missing user_metadata_id".to_string()))?
						.clone(),
				)
				.map_err(|e| sea_orm::DbErr::Custom(format!("Invalid user_metadata_id: {}", e)))?;

				let tag_id: i32 = serde_json::from_value(
					data.get("tag_id")
						.ok_or_else(|| sea_orm::DbErr::Custom("Missing tag_id".to_string()))?
						.clone(),
				)
				.map_err(|e| sea_orm::DbErr::Custom(format!("Invalid tag_id: {}", e)))?;

				let device_uuid: Uuid = serde_json::from_value(
					data.get("device_uuid")
						.ok_or_else(|| sea_orm::DbErr::Custom("Missing device_uuid".to_string()))?
						.clone(),
				)
				.map_err(|e| sea_orm::DbErr::Custom(format!("Invalid device_uuid: {}", e)))?;

				// Dynamic ownership enforcement: check parent user_metadata scope
				use sea_orm::{EntityTrait, QueryFilter, ColumnTrait};
				let parent_metadata = super::user_metadata::Entity::find()
					.filter(super::user_metadata::Column::Id.eq(user_metadata_id))
					.one(db)
					.await?
					.ok_or_else(|| sea_orm::DbErr::Custom(format!(
						"Parent user_metadata not found for id: {}", user_metadata_id
					)))?;

				// If entry-scoped, verify the change came from the entry's owning device
				if let Some(entry_uuid) = parent_metadata.entry_uuid {
					// Entry ownership is tracked through locations
					// First find the entry
					let entry = super::entry::Entity::find()
						.filter(super::entry::Column::Uuid.eq(entry_uuid))
						.one(db)
						.await?;

					if let Some(entry_model) = entry {
						// Find the location that contains this entry
						let location = super::location::Entity::find()
							.filter(super::location::Column::EntryId.eq(entry_model.id))
							.one(db)
							.await?;

						if let Some(location_model) = location {
							// Get the device from the location
							let device = super::device::Entity::find()
								.filter(super::device::Column::Id.eq(location_model.device_id))
								.one(db)
								.await?;

							if let Some(device_model) = device {
								// Check if the change came from the owning device
								if device_model.uuid != device_uuid {
									tracing::warn!(
										entry_uuid = %entry_uuid,
										owning_device = %device_model.uuid,
										sync_device = %device_uuid,
										"Rejecting user_metadata_tag sync - entry-scoped metadata can only be modified by owning device"
									);
									return Ok(()); // Silently ignore changes from non-owning devices
								}
							}
						}
					}
				}
				// If content-scoped, no ownership enforcement needed

				let applied_context: Option<String> = serde_json::from_value(
					data.get("applied_context")
						.cloned()
						.unwrap_or(serde_json::Value::Null),
				)
				.unwrap();

				let applied_variant: Option<String> = serde_json::from_value(
					data.get("applied_variant")
						.cloned()
						.unwrap_or(serde_json::Value::Null),
				)
				.unwrap();

				let confidence: f32 = serde_json::from_value(
					data.get("confidence")
						.cloned()
						.unwrap_or(serde_json::Value::from(1.0)),
				)
				.unwrap();

				let source: String = serde_json::from_value(
					data.get("source")
						.cloned()
						.unwrap_or(serde_json::Value::String("sync".to_string())),
				)
				.unwrap();

				let instance_attributes: Option<Json> = serde_json::from_value(
					data.get("instance_attributes")
						.cloned()
						.unwrap_or(serde_json::Value::Null),
				)
				.unwrap();

				let version: i64 = serde_json::from_value(
					data.get("version")
						.ok_or_else(|| sea_orm::DbErr::Custom("Missing version".to_string()))?
						.clone(),
				)
				.map_err(|e| sea_orm::DbErr::Custom(format!("Invalid version: {}", e)))?;

				let active = ActiveModel {
					id: NotSet,
					user_metadata_id: Set(user_metadata_id),
					tag_id: Set(tag_id),
					applied_context: Set(applied_context),
					applied_variant: Set(applied_variant),
					confidence: Set(confidence),
					source: Set(source),
					instance_attributes: Set(instance_attributes),
					device_uuid: Set(device_uuid),
					uuid: Set(uuid),
					version: Set(version),
					created_at: Set(chrono::Utc::now()),
					updated_at: Set(chrono::Utc::now()),
				};

				Entity::insert(active)
					.on_conflict(
						sea_orm::sea_query::OnConflict::column(Column::Uuid)
							.update_columns([
								Column::UserMetadataId,
								Column::TagId,
								Column::AppliedContext,
								Column::AppliedVariant,
								Column::Confidence,
								Column::Source,
								Column::InstanceAttributes,
								Column::DeviceUuid,
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
