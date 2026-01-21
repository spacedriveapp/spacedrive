//! MIME type entity (runtime discovered)

use crate::infra::sync::{ChangeType, SharedChangeEntry, Syncable};
use sea_orm::entity::prelude::*;
use sea_orm::{ActiveValue::NotSet, Set};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "mime_types")]
pub struct Model {
	#[sea_orm(primary_key)]
	pub id: i32,
	pub uuid: Uuid,
	pub mime_type: String,
	pub created_at: DateTimeUtc,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
	#[sea_orm(has_many = "super::content_identity::Entity")]
	ContentIdentities,
}

impl Related<super::content_identity::Entity> for Entity {
	fn to() -> RelationDef {
		Relation::ContentIdentities.def()
	}
}

impl ActiveModelBehavior for ActiveModel {}

// Syncable Implementation
//
// MimeType is a SHARED resource that syncs across devices.
// MIME types are discovered during indexing and should be available on all devices.
impl Syncable for Model {
	const SYNC_MODEL: &'static str = "mime_type";

	fn sync_id(&self) -> Uuid {
		self.uuid
	}

	fn version(&self) -> i64 {
		1
	}

	fn exclude_fields() -> Option<&'static [&'static str]> {
		Some(&["id", "created_at"])
	}

	fn sync_depends_on() -> &'static [&'static str] {
		&[]
	}

	// FK Lookup Methods (mime_type is FK target for content_identities)
	async fn lookup_id_by_uuid(
		uuid: Uuid,
		db: &DatabaseConnection,
	) -> Result<Option<i32>, sea_orm::DbErr> {
		use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
		Ok(Entity::find()
			.filter(Column::Uuid.eq(uuid))
			.one(db)
			.await?
			.map(|m| m.id))
	}

	async fn lookup_uuid_by_id(
		id: i32,
		db: &DatabaseConnection,
	) -> Result<Option<Uuid>, sea_orm::DbErr> {
		Ok(Entity::find_by_id(id).one(db).await?.map(|m| m.uuid))
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
		let records = Entity::find().filter(Column::Id.is_in(ids)).all(db).await?;
		Ok(records.into_iter().map(|r| (r.id, r.uuid)).collect())
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
			query = query.filter(Column::CreatedAt.gte(since_time));
		}

		// Cursor-based pagination with tie-breaker
		if let Some((cursor_ts, cursor_uuid)) = cursor {
			query = query.filter(
				Condition::any()
					.add(Column::CreatedAt.gt(cursor_ts))
					.add(
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
		for mime in results {
			let json = serde_json::to_value(&mime)
				.map_err(|e| sea_orm::DbErr::Custom(format!("Serialization error: {}", e)))?;
			sync_results.push((mime.uuid, json, mime.created_at));
		}

		Ok(sync_results)
	}

	async fn apply_shared_change(
		entry: SharedChangeEntry,
		db: &DatabaseConnection,
	) -> Result<(), sea_orm::DbErr> {
		use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter};

		match entry.change_type {
			ChangeType::Insert | ChangeType::Update => {
				let data = entry.data.as_object().ok_or_else(|| {
					sea_orm::DbErr::Custom("MimeType data is not an object".to_string())
				})?;

				let uuid: Uuid = serde_json::from_value(
					data.get("uuid")
						.ok_or_else(|| sea_orm::DbErr::Custom("Missing uuid".to_string()))?
						.clone(),
				)
				.map_err(|e| sea_orm::DbErr::Custom(format!("Invalid uuid: {}", e)))?;

				let mime_type: String = serde_json::from_value(
					data.get("mime_type")
						.ok_or_else(|| sea_orm::DbErr::Custom("Missing mime_type".to_string()))?
						.clone(),
				)
				.map_err(|e| sea_orm::DbErr::Custom(format!("Invalid mime_type: {}", e)))?;

				let active = ActiveModel {
					id: NotSet,
					uuid: Set(uuid),
					mime_type: Set(mime_type),
					created_at: Set(chrono::Utc::now().into()),
				};

				Entity::insert(active)
					.on_conflict(
						sea_orm::sea_query::OnConflict::column(Column::Uuid)
							.update_column(Column::MimeType)
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

// Register with sync system via inventory
crate::register_syncable_shared!(Model, "mime_type", "mime_types");
