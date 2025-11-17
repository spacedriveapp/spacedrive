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
}
