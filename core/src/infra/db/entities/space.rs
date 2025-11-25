//! Space entity

use crate::infra::sync::Syncable;
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "spaces")]
pub struct Model {
	#[sea_orm(primary_key)]
	pub id: i32,
	pub uuid: Uuid,
	pub name: String,
	pub icon: String,
	pub color: String,
	pub order: i32,
	pub created_at: DateTimeUtc,
	pub updated_at: DateTimeUtc,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
	#[sea_orm(has_many = "super::space_group::Entity")]
	SpaceGroups,
}

impl Related<super::space_group::Entity> for Entity {
	fn to() -> RelationDef {
		Relation::SpaceGroups.def()
	}
}

impl ActiveModelBehavior for ActiveModel {}

// Syncable Implementation
//
// Spaces are LIBRARY-SCOPED and sync across all devices in the library.
// When a user creates or modifies a space on one device, it syncs to all other devices.
impl Syncable for Model {
	const SYNC_MODEL: &'static str = "space";

	fn sync_id(&self) -> Uuid {
		self.uuid
	}

	fn version(&self) -> i64 {
		// Use updated_at as version for now
		self.updated_at.timestamp()
	}

	fn exclude_fields() -> Option<&'static [&'static str]> {
		Some(&["id"])
	}

	fn sync_depends_on() -> &'static [&'static str] {
		&[]
	}

	fn foreign_key_mappings() -> Vec<crate::infra::sync::FKMapping> {
		vec![]
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
			query = query.filter(Column::UpdatedAt.gte(since_time));
		}

		if let Some((cursor_ts, cursor_uuid)) = cursor {
			query = query.filter(
				Condition::any().add(Column::UpdatedAt.gt(cursor_ts)).add(
					Condition::all()
						.add(Column::UpdatedAt.eq(cursor_ts))
						.add(Column::Uuid.gt(cursor_uuid)),
				),
			);
		}

		query = query
			.order_by_asc(Column::UpdatedAt)
			.order_by_asc(Column::Uuid)
			.limit(batch_size as u64);

		let results = query.all(db).await?;

		let mut sync_results = Vec::new();
		for space in results {
			let json = match space.to_sync_json() {
				Ok(j) => j,
				Err(e) => {
					tracing::error!("Failed to serialize space {}: {}", space.uuid, e);
					continue;
				}
			};

			sync_results.push((space.uuid, json, space.updated_at));
		}

		Ok(sync_results)
	}

	async fn apply_shared_change(
		entry: crate::infra::sync::SharedChangeEntry,
		db: &DatabaseConnection,
	) -> Result<(), sea_orm::DbErr> {
		use crate::infra::sync::ChangeType;
		use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, Set, NotSet};

		match entry.change_type {
			ChangeType::Insert | ChangeType::Update => {
				let data = entry.data.as_object().ok_or_else(|| {
					sea_orm::DbErr::Custom("Space data is not an object".to_string())
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
							.ok_or_else(|| sea_orm::DbErr::Custom("Missing name".to_string()))?
							.clone(),
					)
					.map_err(|e| sea_orm::DbErr::Custom(format!("Invalid name: {}", e)))?),
					icon: Set(serde_json::from_value(
						data.get("icon")
							.ok_or_else(|| sea_orm::DbErr::Custom("Missing icon".to_string()))?
							.clone(),
					)
					.map_err(|e| sea_orm::DbErr::Custom(format!("Invalid icon: {}", e)))?),
					color: Set(serde_json::from_value(
						data.get("color")
							.ok_or_else(|| sea_orm::DbErr::Custom("Missing color".to_string()))?
							.clone(),
					)
					.map_err(|e| sea_orm::DbErr::Custom(format!("Invalid color: {}", e)))?),
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
					updated_at: Set(serde_json::from_value(
						data.get("updated_at")
							.ok_or_else(|| sea_orm::DbErr::Custom("Missing updated_at".to_string()))?
							.clone(),
					)
					.map_err(|e| sea_orm::DbErr::Custom(format!("Invalid updated_at: {}", e)))?),
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
					sea_orm::DbErr::Custom("Space data is not an object".to_string())
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
