//! SpaceItem entity

use crate::infra::sync::Syncable;
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "space_items")]
pub struct Model {
	#[sea_orm(primary_key)]
	pub id: i32,
	pub uuid: Uuid,
	pub space_id: i32,
	pub group_id: Option<i32>, // Nullable - None = space-level item
	pub entry_id: Option<i32>, // Nullable - populated for Path items
	pub item_type: String,     // JSON-serialized ItemType enum
	pub order: i32,
	pub created_at: DateTimeUtc,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
	#[sea_orm(
		belongs_to = "super::space::Entity",
		from = "Column::SpaceId",
		to = "super::space::Column::Id"
	)]
	Space,
	#[sea_orm(
		belongs_to = "super::space_group::Entity",
		from = "Column::GroupId",
		to = "super::space_group::Column::Id"
	)]
	SpaceGroup,
	#[sea_orm(
		belongs_to = "super::entry::Entity",
		from = "Column::EntryId",
		to = "super::entry::Column::Id"
	)]
	Entry,
}

impl Related<super::space::Entity> for Entity {
	fn to() -> RelationDef {
		Relation::Space.def()
	}
}

impl Related<super::space_group::Entity> for Entity {
	fn to() -> RelationDef {
		Relation::SpaceGroup.def()
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
// SpaceItems sync with their parent SpaceGroup
impl Syncable for Model {
	const SYNC_MODEL: &'static str = "space_item";

	fn sync_id(&self) -> Uuid {
		self.uuid
	}

	fn version(&self) -> i64 {
		self.created_at.timestamp()
	}

	fn exclude_fields() -> Option<&'static [&'static str]> {
		Some(&["id"]) // Don't exclude FK fields - needed for UUID conversion
	}

	fn sync_depends_on() -> &'static [&'static str] {
		&["space", "space_group"]
	}

	fn foreign_key_mappings() -> Vec<crate::infra::sync::FKMapping> {
		vec![
			crate::infra::sync::FKMapping::new("space_id", "spaces"),
			crate::infra::sync::FKMapping::new("group_id", "space_groups"),
		]
	}

	// FK Lookup Methods (space_item is FK target - rare but consistent pattern)
	async fn lookup_id_by_uuid(
		uuid: Uuid,
		db: &DatabaseConnection,
	) -> Result<Option<i32>, sea_orm::DbErr> {
		use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
		Ok(Entity::find()
			.filter(Column::Uuid.eq(uuid))
			.one(db)
			.await?
			.map(|i| i.id))
	}

	async fn lookup_uuid_by_id(
		id: i32,
		db: &DatabaseConnection,
	) -> Result<Option<Uuid>, sea_orm::DbErr> {
		Ok(Entity::find_by_id(id).one(db).await?.map(|i| i.uuid))
	}

	async fn batch_lookup_ids_by_uuids(
		uuids: std::collections::HashSet<Uuid>,
		db: &DatabaseConnection,
	) -> Result<std::collections::HashMap<Uuid, i32>, sea_orm::DbErr> {
		use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
		if uuids.is_empty() {
			return Ok(std::collections::HashMap::new());
		}
		let records = Entity::find()
			.filter(Column::Uuid.is_in(uuids))
			.all(db)
			.await?;
		Ok(records.into_iter().map(|r| (r.uuid, r.id)).collect())
	}

	async fn batch_lookup_uuids_by_ids(
		ids: std::collections::HashSet<i32>,
		db: &DatabaseConnection,
	) -> Result<std::collections::HashMap<i32, Uuid>, sea_orm::DbErr> {
		use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
		if ids.is_empty() {
			return Ok(std::collections::HashMap::new());
		}
		let records = Entity::find()
			.filter(Column::Id.is_in(ids))
			.all(db)
			.await?;
		Ok(records.into_iter().map(|r| (r.id, r.uuid)).collect())
	}

	async fn query_for_sync(
		_device_id: Option<Uuid>,
		since: Option<chrono::DateTime<chrono::Utc>>,
		cursor: Option<(chrono::DateTime<chrono::Utc>, Uuid)>,
		batch_size: usize,
		db: &DatabaseConnection,
	) -> Result<Vec<(Uuid, serde_json::Value, chrono::DateTime<chrono::Utc>)>, sea_orm::DbErr> {
		use crate::infra::sync::Syncable;
		use sea_orm::{ColumnTrait, Condition, EntityTrait, QueryFilter, QueryOrder, QuerySelect};

		let mut query = Entity::find();

		if let Some(since_time) = since {
			query = query.filter(Column::CreatedAt.gte(since_time));
		}

		if let Some((cursor_ts, cursor_uuid)) = cursor {
			query = query.filter(
				Condition::any().add(Column::CreatedAt.gt(cursor_ts)).add(
					Condition::all()
						.add(Column::CreatedAt.eq(cursor_ts))
						.add(Column::Uuid.gt(cursor_uuid)),
				),
			);
		}

		query = query
			.order_by_asc(Column::CreatedAt)
			.order_by_asc(Column::Uuid)
			.limit(batch_size as u64);

		let results = query.all(db).await?;

		let mut sync_results = Vec::new();
		for item in results {
			let mut json = match item.to_sync_json() {
				Ok(j) => j,
				Err(e) => {
					tracing::error!("Failed to serialize space_item {}: {}", item.uuid, e);
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
						uuid = %item.uuid,
						"Failed to convert FK to UUID for space_item"
					);
					continue;
				}
			}

			sync_results.push((item.uuid, json, item.created_at));
		}

		Ok(sync_results)
	}

	async fn apply_shared_change(
		entry: crate::infra::sync::SharedChangeEntry,
		db: &DatabaseConnection,
	) -> Result<(), sea_orm::DbErr> {
		use crate::infra::sync::{ChangeType, fk_mapper};
		use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, Set, NotSet};

		match entry.change_type {
			ChangeType::Insert | ChangeType::Update => {
				// Map UUIDs to local IDs for FK fields
				let data =
					fk_mapper::map_sync_json_to_local(entry.data, Self::foreign_key_mappings(), db)
						.await
						.map_err(|e| sea_orm::DbErr::Custom(format!("FK mapping failed: {}", e)))?;

				let data = data.as_object().ok_or_else(|| {
					sea_orm::DbErr::Custom("SpaceItem data is not an object".to_string())
				})?;

				let uuid: Uuid = serde_json::from_value(
					data.get("uuid")
						.ok_or_else(|| sea_orm::DbErr::Custom("Missing uuid".to_string()))?
						.clone(),
				)
				.map_err(|e| sea_orm::DbErr::Custom(format!("Invalid uuid: {}", e)))?;

				let space_id: i32 = serde_json::from_value(
					data.get("space_id")
						.ok_or_else(|| sea_orm::DbErr::Custom("Missing space_id".to_string()))?
						.clone(),
				)
				.map_err(|e| sea_orm::DbErr::Custom(format!("Invalid space_id: {}", e)))?;

				let group_id: Option<i32> = serde_json::from_value(
					data.get("group_id")
						.ok_or_else(|| sea_orm::DbErr::Custom("Missing group_id".to_string()))?
						.clone(),
				)
				.map_err(|e| sea_orm::DbErr::Custom(format!("Invalid group_id: {}", e)))?;

				let entry_id: Option<i32> = data
					.get("entry_id")
					.map(|v| serde_json::from_value(v.clone()).ok())
					.flatten();

				let active = ActiveModel {
					id: NotSet,
					uuid: Set(uuid),
					space_id: Set(space_id),
					group_id: Set(group_id),
					entry_id: Set(entry_id),
					item_type: Set(serde_json::from_value(
						data.get("item_type")
							.ok_or_else(|| sea_orm::DbErr::Custom("Missing item_type".to_string()))?
							.clone(),
					)
					.map_err(|e| sea_orm::DbErr::Custom(format!("Invalid item_type: {}", e)))?),
					order: Set(serde_json::from_value(
						data.get("order")
							.ok_or_else(|| sea_orm::DbErr::Custom("Missing order".to_string()))?
							.clone(),
					)
					.map_err(|e| sea_orm::DbErr::Custom(format!("Invalid order: {}", e)))?),
					created_at: Set(serde_json::from_value(
						data.get("created_at")
							.ok_or_else(|| sea_orm::DbErr::Custom("Missing created_at".to_string()))?
							.clone(),
					)
					.map_err(|e| sea_orm::DbErr::Custom(format!("Invalid created_at: {}", e)))?),
				};

				// Upsert by UUID
				let existing = Entity::find().filter(Column::Uuid.eq(uuid)).one(db).await?;

				if let Some(existing_model) = existing {
					let mut active = active;
					active.id = Set(existing_model.id);
					active.update(db).await?;
				} else {
					active.insert(db).await?;
				}

				Ok(())
			}
			ChangeType::Delete => {
				let data = entry.data.as_object().ok_or_else(|| {
					sea_orm::DbErr::Custom("SpaceItem data is not an object".to_string())
				})?;

				let uuid: Uuid = serde_json::from_value(
					data.get("uuid")
						.ok_or_else(|| sea_orm::DbErr::Custom("Missing uuid".to_string()))?
						.clone(),
				)
				.map_err(|e| sea_orm::DbErr::Custom(format!("Invalid uuid: {}", e)))?;

				Entity::delete_many()
					.filter(Column::Uuid.eq(uuid))
					.exec(db)
					.await?;

				Ok(())
			}
		}
	}
}

// Register with sync system via inventory
crate::register_syncable_shared!(Model, "space_item", "space_items");
