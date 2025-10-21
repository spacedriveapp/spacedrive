//! User metadata entity

use crate::infra::sync::{ChangeType, SharedChangeEntry, Syncable};
use sea_orm::entity::prelude::*;
use sea_orm::{ActiveValue::NotSet, Set};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "user_metadata")]
pub struct Model {
	#[sea_orm(primary_key)]
	pub id: i32,
	pub uuid: Uuid,

	// Exactly one of these is set - defines the scope
	pub entry_uuid: Option<Uuid>, // File-specific metadata (higher priority in hierarchy)
	pub content_identity_uuid: Option<Uuid>, // Content-universal metadata (lower priority in hierarchy)

	// All metadata types benefit from scope flexibility
	pub notes: Option<String>,
	pub favorite: bool,
	pub hidden: bool,
	pub custom_data: Json, // Arbitrary JSON data
	pub created_at: DateTimeUtc,
	pub updated_at: DateTimeUtc,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
	#[sea_orm(
		belongs_to = "super::entry::Entity",
		from = "Column::EntryUuid",
		to = "super::entry::Column::Uuid"
	)]
	Entry,
	#[sea_orm(
		belongs_to = "super::content_identity::Entity",
		from = "Column::ContentIdentityUuid",
		to = "super::content_identity::Column::Uuid"
	)]
	ContentIdentity,
}

impl Related<super::entry::Entity> for Entity {
	fn to() -> RelationDef {
		Relation::Entry.def()
	}
}

impl Related<super::content_identity::Entity> for Entity {
	fn to() -> RelationDef {
		Relation::ContentIdentity.def()
	}
}

impl Related<super::tag::Entity> for Entity {
	fn to() -> RelationDef {
		super::user_metadata_tag::Relation::Tag.def()
	}

	fn via() -> Option<RelationDef> {
		Some(super::user_metadata_tag::Relation::UserMetadata.def().rev())
	}
}

impl ActiveModelBehavior for ActiveModel {}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum MetadataScope {
	Entry,   // File-specific (higher priority)
	Content, // Content-universal (lower priority)
}

impl Model {
	/// Get the scope of this metadata (entry or content-level)
	pub fn scope(&self) -> Option<MetadataScope> {
		if self.entry_uuid.is_some() {
			Some(MetadataScope::Entry)
		} else if self.content_identity_uuid.is_some() {
			Some(MetadataScope::Content)
		} else {
			None // Invalid state - should be caught by DB constraint
		}
	}

	/// Check if this metadata is entry-scoped
	pub fn is_entry_scoped(&self) -> bool {
		self.entry_uuid.is_some()
	}

	/// Check if this metadata is content-scoped
	pub fn is_content_scoped(&self) -> bool {
		self.content_identity_uuid.is_some()
	}
}

// Syncable Implementation
//
// UserMetadata is a SHARED resource using HLC-ordered log-based replication.
// Both entry-scoped and content-scoped metadata are synced across devices.
// This allows user preferences (favorites, notes, etc.) to follow content
// regardless of which device it's accessed from.
impl Syncable for Model {
	const SYNC_MODEL: &'static str = "user_metadata";

	fn sync_id(&self) -> Uuid {
		self.uuid
	}

	fn version(&self) -> i64 {
		1
	}

	fn exclude_fields() -> Option<&'static [&'static str]> {
		Some(&["id", "created_at", "updated_at"])
	}

	fn sync_depends_on() -> &'static [&'static str] {
		&[]
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
		for metadata in results {
			let json = match metadata.to_sync_json() {
				Ok(j) => j,
				Err(e) => {
					tracing::warn!(error = %e, uuid = %metadata.uuid, "Failed to serialize user_metadata for sync");
					continue;
				}
			};

			sync_results.push((metadata.uuid, json, metadata.updated_at));
		}

		Ok(sync_results)
	}

	async fn apply_shared_change(
		entry: SharedChangeEntry,
		db: &DatabaseConnection,
	) -> Result<(), sea_orm::DbErr> {
		match entry.change_type {
			ChangeType::Insert | ChangeType::Update => {
				let data = entry.data.as_object().ok_or_else(|| {
					sea_orm::DbErr::Custom("UserMetadata data is not an object".to_string())
				})?;

				let uuid: Uuid = serde_json::from_value(
					data.get("uuid")
						.ok_or_else(|| sea_orm::DbErr::Custom("Missing uuid".to_string()))?
						.clone(),
				)
				.map_err(|e| sea_orm::DbErr::Custom(format!("Invalid uuid: {}", e)))?;

				let active = ActiveModel {
					id: NotSet,
					uuid: Set(uuid),
					entry_uuid: Set(serde_json::from_value(
						data.get("entry_uuid")
							.cloned()
							.unwrap_or(serde_json::Value::Null),
					)
					.unwrap()),
					content_identity_uuid: Set(serde_json::from_value(
						data.get("content_identity_uuid")
							.cloned()
							.unwrap_or(serde_json::Value::Null),
					)
					.unwrap()),
					notes: Set(serde_json::from_value(
						data.get("notes")
							.cloned()
							.unwrap_or(serde_json::Value::Null),
					)
					.unwrap()),
					favorite: Set(serde_json::from_value(
						data.get("favorite")
							.cloned()
							.unwrap_or(serde_json::Value::Bool(false)),
					)
					.unwrap()),
					hidden: Set(serde_json::from_value(
						data.get("hidden")
							.cloned()
							.unwrap_or(serde_json::Value::Bool(false)),
					)
					.unwrap()),
					custom_data: Set(serde_json::from_value(
						data.get("custom_data")
							.cloned()
							.unwrap_or(serde_json::json!({})),
					)
					.unwrap()),
					created_at: Set(chrono::Utc::now().into()),
					updated_at: Set(chrono::Utc::now().into()),
				};

				Entity::insert(active)
					.on_conflict(
						sea_orm::sea_query::OnConflict::column(Column::Uuid)
							.update_columns([
								Column::EntryUuid,
								Column::ContentIdentityUuid,
								Column::Notes,
								Column::Favorite,
								Column::Hidden,
								Column::CustomData,
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
