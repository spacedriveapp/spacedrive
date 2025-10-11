use spacedrive_sdk::{model, persist_strategy};

use serde::{Deserialize, Serialize};
use spacedrive_sdk::prelude::*;
use uuid::Uuid;

pub type PlaceId = Uuid;

#[derive(Serialize, Deserialize, Clone)]
#[model(version = "1.0.0")]
#[persist_strategy("always")]
pub struct Place {
	pub id: PlaceId,

	#[sync(shared)]
	pub name: String,

	#[sync(shared)]
	pub latitude: f64,

	#[sync(shared)]
	pub longitude: f64,

	#[sync(shared)]
	pub radius_meters: f32,

	#[sync(device_owned)]
	pub photo_count: usize,

	#[sync(shared)]
	pub thumbnail_photo_id: Option<Uuid>,
}
