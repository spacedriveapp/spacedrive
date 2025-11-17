use chrono::{DateTime, Utc};
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "sidecar")]
pub struct Model {
	#[sea_orm(primary_key)]
	pub id: i32,

	pub uuid: Uuid,

	pub content_uuid: Uuid,

	pub kind: String,

	pub variant: String,

	pub format: String,

	pub rel_path: String,

	/// For reference sidecars, the entry ID of the original file
	/// This allows sidecars to reference existing entries without moving them
	pub source_entry_id: Option<i32>,

	pub size: i64,

	pub checksum: Option<String>,

	pub status: String,

	pub source: Option<String>,

	pub version: i32,

	pub created_at: DateTime<Utc>,

	pub updated_at: DateTime<Utc>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
	#[sea_orm(
		belongs_to = "super::content_identity::Entity",
		from = "Column::ContentUuid",
		to = "super::content_identity::Column::Uuid"
	)]
	ContentIdentity,

	#[sea_orm(
		belongs_to = "super::entry::Entity",
		from = "Column::SourceEntryId",
		to = "super::entry::Column::Id"
	)]
	SourceEntry,
}

impl Related<super::content_identity::Entity> for Entity {
	fn to() -> RelationDef {
		Relation::ContentIdentity.def()
	}
}

impl Related<super::entry::Entity> for Entity {
	fn to() -> RelationDef {
		Relation::SourceEntry.def()
	}
}

impl ActiveModelBehavior for ActiveModel {}

// Sidecars are SHARED resources (content-scoped, not device-owned).
// All devices should know what sidecars exist globally via library sync.
// Actual file availability is tracked separately in sidecar_availability (local only).
impl crate::infra::sync::Syncable for Model {
	const SYNC_MODEL: &'static str = "sidecar";

	fn sync_id(&self) -> Uuid {
		self.uuid
	}

	fn version(&self) -> i64 {
		self.version as i64
	}

	fn exclude_fields() -> Option<&'static [&'static str]> {
		Some(&[
			"id",              // Local database ID
			"source_entry_id", // Local entry reference
		])
	}

	fn sync_depends_on() -> &'static [&'static str] {
		&["content_identity"] // Sidecars depend on content existing first
	}

	fn foreign_key_mappings() -> Vec<crate::infra::sync::FKMapping> {
		vec![
			// Map content_uuid FK to content_identities table
			crate::infra::sync::FKMapping::new("content_uuid", "content_identities"),
		]
	}

	async fn query_for_sync(
		_device_id: Option<Uuid>,
		since: Option<chrono::DateTime<chrono::Utc>>,
		cursor: Option<(chrono::DateTime<chrono::Utc>, Uuid)>,
		batch_size: usize,
		db: &DatabaseConnection,
	) -> Result<Vec<(Uuid, serde_json::Value, chrono::DateTime<chrono::Utc>)>, sea_orm::DbErr> {
		use sea_orm::{ColumnTrait, Condition, EntityTrait, QueryFilter, QueryOrder, QuerySelect};

		let mut query = Entity::find();

		if let Some(since_time) = since {
			query = query.filter(Column::UpdatedAt.gte(since_time));
		}

		// Cursor-based pagination
		if let Some((cursor_ts, cursor_uuid)) = cursor {
			query = query.filter(
				Condition::any().add(Column::UpdatedAt.gt(cursor_ts)).add(
					Condition::all()
						.add(Column::UpdatedAt.eq(cursor_ts))
						.add(Column::Uuid.gt(cursor_uuid)),
				),
			);
		}

		let results = query
			.order_by_asc(Column::UpdatedAt)
			.order_by_asc(Column::Uuid)
			.limit(batch_size as u64)
			.all(db)
			.await?;

		// Convert to sync format
		let mut sync_data = Vec::new();
		for model in results {
			let json = serde_json::to_value(&model)
				.map_err(|e| DbErr::Custom(format!("Failed to serialize sidecar: {}", e)))?;
			sync_data.push((model.uuid, json, model.updated_at));
		}

		Ok(sync_data)
	}
}
