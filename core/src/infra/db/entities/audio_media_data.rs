//! Audio media data entity

use crate::infra::sync::{ChangeType, SharedChangeEntry, Syncable};
use sea_orm::entity::prelude::*;
use sea_orm::{ActiveValue::NotSet, Set};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "audio_media_data")]
pub struct Model {
	#[sea_orm(primary_key)]
	pub id: i32,
	pub uuid: Uuid,
	pub duration_seconds: Option<f64>,
	pub bit_rate: Option<i64>,
	pub sample_rate: Option<i32>,
	pub channels: Option<String>,
	pub codec: Option<String>,
	pub title: Option<String>,
	pub artist: Option<String>,
	pub album: Option<String>,
	pub album_artist: Option<String>,
	pub genre: Option<String>,
	pub year: Option<i32>,
	pub track_number: Option<i32>,
	pub disc_number: Option<i32>,
	pub composer: Option<String>,
	pub publisher: Option<String>,
	pub copyright: Option<String>,
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
	const SYNC_MODEL: &'static str = "audio_media_data";

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
					tracing::warn!(error = %e, uuid = %record.uuid, "Failed to serialize audio_media_data for sync");
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
					sea_orm::DbErr::Custom("AudioMediaData data is not an object".to_string())
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
					sample_rate: Set(serde_json::from_value(
						data.get("sample_rate")
							.cloned()
							.unwrap_or(serde_json::Value::Null),
					)
					.unwrap()),
					channels: Set(serde_json::from_value(
						data.get("channels")
							.cloned()
							.unwrap_or(serde_json::Value::Null),
					)
					.unwrap()),
					codec: Set(serde_json::from_value(
						data.get("codec").cloned().unwrap_or(serde_json::Value::Null),
					)
					.unwrap()),
					title: Set(serde_json::from_value(
						data.get("title").cloned().unwrap_or(serde_json::Value::Null),
					)
					.unwrap()),
					artist: Set(serde_json::from_value(
						data.get("artist")
							.cloned()
							.unwrap_or(serde_json::Value::Null),
					)
					.unwrap()),
					album: Set(serde_json::from_value(
						data.get("album").cloned().unwrap_or(serde_json::Value::Null),
					)
					.unwrap()),
					album_artist: Set(serde_json::from_value(
						data.get("album_artist")
							.cloned()
							.unwrap_or(serde_json::Value::Null),
					)
					.unwrap()),
					genre: Set(serde_json::from_value(
						data.get("genre").cloned().unwrap_or(serde_json::Value::Null),
					)
					.unwrap()),
					year: Set(serde_json::from_value(
						data.get("year").cloned().unwrap_or(serde_json::Value::Null),
					)
					.unwrap()),
					track_number: Set(serde_json::from_value(
						data.get("track_number")
							.cloned()
							.unwrap_or(serde_json::Value::Null),
					)
					.unwrap()),
					disc_number: Set(serde_json::from_value(
						data.get("disc_number")
							.cloned()
							.unwrap_or(serde_json::Value::Null),
					)
					.unwrap()),
					composer: Set(serde_json::from_value(
						data.get("composer")
							.cloned()
							.unwrap_or(serde_json::Value::Null),
					)
					.unwrap()),
					publisher: Set(serde_json::from_value(
						data.get("publisher")
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
					created_at: Set(chrono::Utc::now().into()),
					updated_at: Set(chrono::Utc::now().into()),
				};

				Entity::insert(active)
					.on_conflict(
						sea_orm::sea_query::OnConflict::column(Column::Uuid)
							.update_columns([
								Column::DurationSeconds,
								Column::BitRate,
								Column::SampleRate,
								Column::Channels,
								Column::Codec,
								Column::Title,
								Column::Artist,
								Column::Album,
								Column::AlbumArtist,
								Column::Genre,
								Column::Year,
								Column::TrackNumber,
								Column::DiscNumber,
								Column::Composer,
								Column::Publisher,
								Column::Copyright,
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
