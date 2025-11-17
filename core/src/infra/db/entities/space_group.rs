//! SpaceGroup entity

use crate::infra::sync::Syncable;
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "space_groups")]
pub struct Model {
	#[sea_orm(primary_key)]
	pub id: i32,
	pub uuid: Uuid,
	pub space_id: i32,
	pub name: String,
	pub group_type: String, // JSON-serialized GroupType enum
	pub is_collapsed: bool,
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
	#[sea_orm(has_many = "super::space_item::Entity")]
	SpaceItems,
}

impl Related<super::space::Entity> for Entity {
	fn to() -> RelationDef {
		Relation::Space.def()
	}
}

impl Related<super::space_item::Entity> for Entity {
	fn to() -> RelationDef {
		Relation::SpaceItems.def()
	}
}

impl ActiveModelBehavior for ActiveModel {}

// Syncable Implementation
//
// SpaceGroups sync with their parent Space
impl Syncable for Model {
	const SYNC_MODEL: &'static str = "space_group";

	fn sync_id(&self) -> Uuid {
		self.uuid
	}

	fn version(&self) -> i64 {
		self.created_at.timestamp()
	}

	fn exclude_fields() -> Option<&'static [&'static str]> {
		Some(&["id", "space_id"])
	}

	fn sync_depends_on() -> &'static [&'static str] {
		&["space"]
	}

	fn foreign_key_mappings() -> Vec<crate::infra::sync::FKMapping> {
		vec![crate::infra::sync::FKMapping::new("space_id", "spaces")]
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
		for group in results {
			let json = match group.to_sync_json() {
				Ok(j) => j,
				Err(e) => {
					tracing::error!("Failed to serialize space_group {}: {}", group.uuid, e);
					continue;
				}
			};

			sync_results.push((group.uuid, json, group.created_at));
		}

		Ok(sync_results)
	}
}
