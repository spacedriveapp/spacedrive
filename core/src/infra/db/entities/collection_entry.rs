use crate::infra::sync::{ChangeType, FKMapping, SharedChangeEntry, Syncable};
use chrono::{DateTime, Utc};
use sea_orm::entity::prelude::*;
use sea_orm::{ActiveValue::NotSet, Set};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "collection_entry")]
pub struct Model {
	#[sea_orm(primary_key, auto_increment = false)]
	pub collection_id: i32,

	#[sea_orm(primary_key, auto_increment = false)]
	pub entry_id: i32,

	pub added_at: DateTime<Utc>,

	// Sync fields
	pub uuid: Uuid,
	pub version: i64,
	pub updated_at: DateTime<Utc>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
	#[sea_orm(
		belongs_to = "super::collection::Entity",
		from = "Column::CollectionId",
		to = "super::collection::Column::Id",
		on_delete = "Cascade"
	)]
	Collection,

	#[sea_orm(
		belongs_to = "super::entry::Entity",
		from = "Column::EntryId",
		to = "super::entry::Column::Id",
		on_delete = "Cascade"
	)]
	Entry,
}

impl Related<super::collection::Entity> for Entity {
	fn to() -> RelationDef {
		Relation::Collection.def()
	}
}

impl Related<super::entry::Entity> for Entity {
	fn to() -> RelationDef {
		Relation::Entry.def()
	}
}

impl ActiveModelBehavior for ActiveModel {}

// Syncable Implementation
//
// CollectionEntry is a SHARED M2M junction table linking collections to entries.
// Collections are shared resources, and entries are device-owned, but the relationships
// themselves are shared across devices using HLC-based replication.
impl Syncable for Model {
	const SYNC_MODEL: &'static str = "collection_entry";

	fn sync_id(&self) -> Uuid {
		self.uuid
	}

	fn version(&self) -> i64 {
		self.version
	}

	fn exclude_fields() -> Option<&'static [&'static str]> {
		None // FK fields need to be present for UUID conversion
	}

	fn sync_depends_on() -> &'static [&'static str] {
		&["collection", "entry"]
	}

	fn foreign_key_mappings() -> Vec<FKMapping> {
		vec![
			FKMapping::new("collection_id", "collection"),
			FKMapping::new("entry_id", "entries"),
		]
	}

	async fn query_for_sync(
		_device_id: Option<Uuid>,
		since: Option<DateTime<Utc>>,
		_cursor: Option<(DateTime<Utc>, Uuid)>,
		batch_size: usize,
		db: &DatabaseConnection,
	) -> Result<Vec<(Uuid, serde_json::Value, DateTime<Utc>)>, sea_orm::DbErr> {
		use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QuerySelect};

		let mut query = Entity::find();

		if let Some(since_time) = since {
			query = query.filter(Column::UpdatedAt.gte(since_time));
		}

		query = query.limit(batch_size as u64);

		let results = query.all(db).await?;

		let mut sync_results = Vec::new();
		for ce in results {
			let mut json = match ce.to_sync_json() {
				Ok(j) => j,
				Err(e) => {
					tracing::warn!(error = %e, uuid = %ce.uuid, "Failed to serialize collection_entry for sync");
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
						uuid = %ce.uuid,
						"Failed to convert FK to UUID for collection_entry"
					);
					continue;
				}
			}

			sync_results.push((ce.uuid, json, ce.updated_at));
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
					sea_orm::DbErr::Custom("CollectionEntry data is not an object".to_string())
				})?;

				let uuid: Uuid = serde_json::from_value(
					data.get("uuid")
						.ok_or_else(|| sea_orm::DbErr::Custom("Missing uuid".to_string()))?
						.clone(),
				)
				.map_err(|e| sea_orm::DbErr::Custom(format!("Invalid uuid: {}", e)))?;

				let collection_id: i32 = serde_json::from_value(
					data.get("collection_id")
						.ok_or_else(|| sea_orm::DbErr::Custom("Missing collection_id".to_string()))?
						.clone(),
				)
				.map_err(|e| sea_orm::DbErr::Custom(format!("Invalid collection_id: {}", e)))?;

				let entry_id: i32 = serde_json::from_value(
					data.get("entry_id")
						.ok_or_else(|| sea_orm::DbErr::Custom("Missing entry_id".to_string()))?
						.clone(),
				)
				.map_err(|e| sea_orm::DbErr::Custom(format!("Invalid entry_id: {}", e)))?;

				let added_at: DateTime<Utc> = serde_json::from_value(
					data.get("added_at")
						.ok_or_else(|| sea_orm::DbErr::Custom("Missing added_at".to_string()))?
						.clone(),
				)
				.map_err(|e| sea_orm::DbErr::Custom(format!("Invalid added_at: {}", e)))?;

				let version: i64 = serde_json::from_value(
					data.get("version")
						.ok_or_else(|| sea_orm::DbErr::Custom("Missing version".to_string()))?
						.clone(),
				)
				.map_err(|e| sea_orm::DbErr::Custom(format!("Invalid version: {}", e)))?;

				let active = ActiveModel {
					collection_id: Set(collection_id),
					entry_id: Set(entry_id),
					added_at: Set(added_at),
					uuid: Set(uuid),
					version: Set(version),
					updated_at: Set(Utc::now()),
				};

				Entity::insert(active)
					.on_conflict(
						sea_orm::sea_query::OnConflict::column(Column::Uuid)
							.update_columns([
								Column::CollectionId,
								Column::EntryId,
								Column::AddedAt,
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
