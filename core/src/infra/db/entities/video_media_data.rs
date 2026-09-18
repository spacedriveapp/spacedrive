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

// Register with sync system via inventory
