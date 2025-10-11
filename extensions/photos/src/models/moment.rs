use spacedrive_sdk::model;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use spacedrive_sdk::prelude::*;
use uuid::Uuid;

use super::PlaceId;

pub type MomentId = Uuid;

#[derive(Serialize, Deserialize, Clone)]
#[model(version = "1.0.0")]
pub struct Moment {
	pub id: MomentId,

	#[sync(shared)]
	pub title: String,

	#[sync(shared)]
	pub start_date: DateTime<Utc>,

	#[sync(shared)]
	pub end_date: DateTime<Utc>,

	#[sync(shared)]
	pub location: Option<PlaceId>,

	#[sync(shared, conflict = "union_merge")]
	pub photo_ids: Vec<Uuid>,

	#[computed]
	pub photo_count: usize,
}

pub type DateRange = (DateTime<Utc>, DateTime<Utc>);

#[derive(Serialize, Deserialize, Clone)]
pub struct MomentGroup {
	pub photo_ids: Vec<Uuid>,
	pub start_date: DateTime<Utc>,
	pub end_date: DateTime<Utc>,
	pub place_id: Option<PlaceId>,
	pub place_name: Option<String>,
	pub common_scenes: Vec<String>,
}
