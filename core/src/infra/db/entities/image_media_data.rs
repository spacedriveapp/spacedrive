//! Image media data entity

use crate::infra::sync::{ChangeType, SharedChangeEntry, Syncable};
use sea_orm::entity::prelude::*;
use sea_orm::{ActiveValue::NotSet, Set};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "image_media_data")]
pub struct Model {
	#[sea_orm(primary_key)]
	pub id: i32,
	pub uuid: Uuid,
	pub width: i32,
	pub height: i32,
	pub date_taken: Option<DateTimeUtc>,
	pub latitude: Option<f64>,
	pub longitude: Option<f64>,
	pub camera_make: Option<String>,
	pub camera_model: Option<String>,
	pub lens_model: Option<String>,
	pub focal_length: Option<String>,
	pub aperture: Option<String>,
	pub shutter_speed: Option<String>,
	pub iso: Option<i32>,
	pub orientation: Option<i16>,
	pub color_space: Option<String>,
	pub color_profile: Option<String>,
	pub bit_depth: Option<String>,
	pub artist: Option<String>,
	pub copyright: Option<String>,
	pub description: Option<String>,
	pub created_at: DateTimeUtc,
	pub updated_at: DateTimeUtc,
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
// ImageMediaData is a SHARED resource with deterministic UUIDs.
// Uses HLC-ordered log-based replication.
impl Syncable for Model {
	const SYNC_MODEL: &'static str = "image_media_data";

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
		cursor: Option<(chrono::DateTime<chrono::Utc>, Uuid)>,
		batch_size: usize,
		db: &DatabaseConnection,
	) -> Result<Vec<(Uuid, serde_json::Value, chrono::DateTime<chrono::Utc>)>, sea_orm::DbErr> {
		use sea_orm::{ColumnTrait, Condition, EntityTrait, QueryFilter, QueryOrder, QuerySelect};

		let mut query = Entity::find();

		if let Some(since_time) = since {
			query = query.filter(Column::UpdatedAt.gte(since_time));
		}

		if let Some((cursor_ts, cursor_uuid)) = cursor {
			query = query.filter(
				Condition::any()
					.add(Column::UpdatedAt.gt(cursor_ts))
					.add(
						Condition::all()
							.add(Column::UpdatedAt.eq(cursor_ts))
							.add(Column::Uuid.gt(cursor_uuid)),
					),
			);
		}

		query = query
			.order_by_asc(Column::UpdatedAt)
			.order_by_asc(Column::Uuid);

		query = query.limit(batch_size as u64);

		let results = query.all(db).await?;

		let mut sync_results = Vec::new();
		for record in results {
			let json = match record.to_sync_json() {
				Ok(j) => j,
				Err(e) => {
					tracing::warn!(error = %e, uuid = %record.uuid, "Failed to serialize image_media_data for sync");
					continue;
				}
			};

			sync_results.push((record.uuid, json, record.updated_at));
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
					sea_orm::DbErr::Custom("ImageMediaData data is not an object".to_string())
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
					width: Set(serde_json::from_value(
						data.get("width").cloned().unwrap_or(serde_json::Value::Null),
					)
					.unwrap_or(0)),
					height: Set(serde_json::from_value(
						data.get("height")
							.cloned()
							.unwrap_or(serde_json::Value::Null),
					)
					.unwrap_or(0)),
					date_taken: Set(serde_json::from_value(
						data.get("date_taken")
							.cloned()
							.unwrap_or(serde_json::Value::Null),
					)
					.unwrap()),
					latitude: Set(serde_json::from_value(
						data.get("latitude")
							.cloned()
							.unwrap_or(serde_json::Value::Null),
					)
					.unwrap()),
					longitude: Set(serde_json::from_value(
						data.get("longitude")
							.cloned()
							.unwrap_or(serde_json::Value::Null),
					)
					.unwrap()),
					camera_make: Set(serde_json::from_value(
						data.get("camera_make")
							.cloned()
							.unwrap_or(serde_json::Value::Null),
					)
					.unwrap()),
					camera_model: Set(serde_json::from_value(
						data.get("camera_model")
							.cloned()
							.unwrap_or(serde_json::Value::Null),
					)
					.unwrap()),
					lens_model: Set(serde_json::from_value(
						data.get("lens_model")
							.cloned()
							.unwrap_or(serde_json::Value::Null),
					)
					.unwrap()),
					focal_length: Set(serde_json::from_value(
						data.get("focal_length")
							.cloned()
							.unwrap_or(serde_json::Value::Null),
					)
					.unwrap()),
					aperture: Set(serde_json::from_value(
						data.get("aperture")
							.cloned()
							.unwrap_or(serde_json::Value::Null),
					)
					.unwrap()),
					shutter_speed: Set(serde_json::from_value(
						data.get("shutter_speed")
							.cloned()
							.unwrap_or(serde_json::Value::Null),
					)
					.unwrap()),
					iso: Set(serde_json::from_value(
						data.get("iso").cloned().unwrap_or(serde_json::Value::Null),
					)
					.unwrap()),
					orientation: Set(serde_json::from_value(
						data.get("orientation")
							.cloned()
							.unwrap_or(serde_json::Value::Null),
					)
					.unwrap()),
					color_space: Set(serde_json::from_value(
						data.get("color_space")
							.cloned()
							.unwrap_or(serde_json::Value::Null),
					)
					.unwrap()),
					color_profile: Set(serde_json::from_value(
						data.get("color_profile")
							.cloned()
							.unwrap_or(serde_json::Value::Null),
					)
					.unwrap()),
					bit_depth: Set(serde_json::from_value(
						data.get("bit_depth")
							.cloned()
							.unwrap_or(serde_json::Value::Null),
					)
					.unwrap()),
					artist: Set(serde_json::from_value(
						data.get("artist")
							.cloned()
							.unwrap_or(serde_json::Value::Null),
					)
					.unwrap()),
					copyright: Set(serde_json::from_value(
						data.get("copyright")
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
					created_at: Set(chrono::Utc::now().into()),
					updated_at: Set(chrono::Utc::now().into()),
				};

				Entity::insert(active)
					.on_conflict(
						sea_orm::sea_query::OnConflict::column(Column::Uuid)
							.update_columns([
								Column::Width,
								Column::Height,
								Column::DateTaken,
								Column::Latitude,
								Column::Longitude,
								Column::CameraMake,
								Column::CameraModel,
								Column::LensModel,
								Column::FocalLength,
								Column::Aperture,
								Column::ShutterSpeed,
								Column::Iso,
								Column::Orientation,
								Column::ColorSpace,
								Column::ColorProfile,
								Column::BitDepth,
								Column::Artist,
								Column::Copyright,
								Column::Description,
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
