//! Video media data entity

use crate::infra::sync::{ChangeType, SharedChangeEntry, Syncable};
use sea_orm::entity::prelude::*;
use sea_orm::{ActiveValue::NotSet, Set};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "video_media_data")]
pub struct Model {
	#[sea_orm(primary_key)]
	pub id: i32,
	pub uuid: Uuid,
	pub width: i32,
	pub height: i32,
	pub blurhash: Option<String>,
	pub duration_seconds: Option<f64>,
	pub bit_rate: Option<i64>,
	pub codec: Option<String>,
	pub pixel_format: Option<String>,
	pub color_space: Option<String>,
	pub color_range: Option<String>,
	pub color_primaries: Option<String>,
	pub color_transfer: Option<String>,
	pub fps_num: Option<i32>,
	pub fps_den: Option<i32>,
	pub audio_codec: Option<String>,
	pub audio_channels: Option<String>,
	pub audio_sample_rate: Option<i32>,
	pub audio_bit_rate: Option<i32>,
	pub title: Option<String>,
	pub artist: Option<String>,
	pub album: Option<String>,
	pub creation_time: Option<DateTimeUtc>,
	pub date_captured: Option<DateTimeUtc>,
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
impl Syncable for Model {
	const SYNC_MODEL: &'static str = "video_media_data";

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
				Condition::any().add(Column::UpdatedAt.gt(cursor_ts)).add(
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
					tracing::warn!(error = %e, uuid = %record.uuid, "Failed to serialize video_media_data for sync");
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
					sea_orm::DbErr::Custom("VideoMediaData data is not an object".to_string())
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
						data.get("width")
							.cloned()
							.unwrap_or(serde_json::Value::Null),
					)
					.unwrap_or(0)),
					height: Set(serde_json::from_value(
						data.get("height")
							.cloned()
							.unwrap_or(serde_json::Value::Null),
					)
					.unwrap_or(0)),
					blurhash: Set(serde_json::from_value(
						data.get("blurhash")
							.cloned()
							.unwrap_or(serde_json::Value::Null),
					)
					.unwrap()),
					duration_seconds: Set(serde_json::from_value(
						data.get("duration_seconds")
							.cloned()
							.unwrap_or(serde_json::Value::Null),
					)
					.unwrap()),
					bit_rate: Set(serde_json::from_value(
						data.get("bit_rate")
							.cloned()
							.unwrap_or(serde_json::Value::Null),
					)
					.unwrap()),
					codec: Set(serde_json::from_value(
						data.get("codec")
							.cloned()
							.unwrap_or(serde_json::Value::Null),
					)
					.unwrap()),
					pixel_format: Set(serde_json::from_value(
						data.get("pixel_format")
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
					color_range: Set(serde_json::from_value(
						data.get("color_range")
							.cloned()
							.unwrap_or(serde_json::Value::Null),
					)
					.unwrap()),
					color_primaries: Set(serde_json::from_value(
						data.get("color_primaries")
							.cloned()
							.unwrap_or(serde_json::Value::Null),
					)
					.unwrap()),
					color_transfer: Set(serde_json::from_value(
						data.get("color_transfer")
							.cloned()
							.unwrap_or(serde_json::Value::Null),
					)
					.unwrap()),
					fps_num: Set(serde_json::from_value(
						data.get("fps_num")
							.cloned()
							.unwrap_or(serde_json::Value::Null),
					)
					.unwrap()),
					fps_den: Set(serde_json::from_value(
						data.get("fps_den")
							.cloned()
							.unwrap_or(serde_json::Value::Null),
					)
					.unwrap()),
					audio_codec: Set(serde_json::from_value(
						data.get("audio_codec")
							.cloned()
							.unwrap_or(serde_json::Value::Null),
					)
					.unwrap()),
					audio_channels: Set(serde_json::from_value(
						data.get("audio_channels")
							.cloned()
							.unwrap_or(serde_json::Value::Null),
					)
					.unwrap()),
					audio_sample_rate: Set(serde_json::from_value(
						data.get("audio_sample_rate")
							.cloned()
							.unwrap_or(serde_json::Value::Null),
					)
					.unwrap()),
					audio_bit_rate: Set(serde_json::from_value(
						data.get("audio_bit_rate")
							.cloned()
							.unwrap_or(serde_json::Value::Null),
					)
					.unwrap()),
					title: Set(serde_json::from_value(
						data.get("title")
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
					album: Set(serde_json::from_value(
						data.get("album")
							.cloned()
							.unwrap_or(serde_json::Value::Null),
					)
					.unwrap()),
					creation_time: Set(serde_json::from_value(
						data.get("creation_time")
							.cloned()
							.unwrap_or(serde_json::Value::Null),
					)
					.unwrap()),
					date_captured: Set(serde_json::from_value(
						data.get("date_captured")
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
								Column::Blurhash,
								Column::DurationSeconds,
								Column::BitRate,
								Column::Codec,
								Column::PixelFormat,
								Column::ColorSpace,
								Column::ColorRange,
								Column::ColorPrimaries,
								Column::ColorTransfer,
								Column::FpsNum,
								Column::FpsDen,
								Column::AudioCodec,
								Column::AudioChannels,
								Column::AudioSampleRate,
								Column::AudioBitRate,
								Column::Title,
								Column::Artist,
								Column::Album,
								Column::CreationTime,
								Column::DateCaptured,
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
