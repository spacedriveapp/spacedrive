use crate::infra::sync::{ChangeType, SharedChangeEntry, Syncable};
use chrono::{DateTime, Utc};
use sea_orm::entity::prelude::*;
use sea_orm::{ActiveValue::NotSet, Set};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "collection")]
pub struct Model {
	#[sea_orm(primary_key)]
	pub id: i32,

	#[sea_orm(unique)]
	pub uuid: Uuid,

	pub name: String,

	pub description: Option<String>,

	pub created_at: DateTime<Utc>,

	pub updated_at: DateTime<Utc>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
	#[sea_orm(has_many = "super::collection_entry::Entity")]
	CollectionEntries,
}

impl Related<super::collection_entry::Entity> for Entity {
	fn to() -> RelationDef {
		Relation::CollectionEntries.def()
	}
}

impl Related<super::entry::Entity> for Entity {
	fn to() -> RelationDef {
		super::collection_entry::Relation::Entry.def()
	}

	fn via() -> Option<RelationDef> {
		Some(super::collection_entry::Relation::Collection.def().rev())
	}
}

impl ActiveModelBehavior for ActiveModel {}

// Syncable Implementation
//
// Collections are SHARED resources using HLC-ordered log-based replication.
// Multiple users can create and modify collections across devices.
impl Syncable for Model {
	const SYNC_MODEL: &'static str = "collection";

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
		for collection in results {
			let json = match collection.to_sync_json() {
				Ok(j) => j,
				Err(e) => {
					tracing::warn!(error = %e, uuid = %collection.uuid, "Failed to serialize collection for sync");
					continue;
				}
			};

			sync_results.push((collection.uuid, json, collection.updated_at));
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
					sea_orm::DbErr::Custom("Collection data is not an object".to_string())
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
					name: Set(serde_json::from_value(
						data.get("name")
							.cloned()
							.unwrap_or(serde_json::Value::Null),
					)
					.unwrap()),
					description: Set(serde_json::from_value(
						data.get("description")
							.cloned()
							.unwrap_or(serde_json::Value::Null),
					)
					.unwrap()),
					created_at: Set(chrono::Utc::now()),
					updated_at: Set(chrono::Utc::now()),
				};

				Entity::insert(active)
					.on_conflict(
						sea_orm::sea_query::OnConflict::column(Column::Uuid)
							.update_columns([Column::Name, Column::Description, Column::UpdatedAt])
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
