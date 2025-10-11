use spacedrive_sdk::model;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use spacedrive_sdk::prelude::*;
use uuid::Uuid;

#[derive(Serialize, Deserialize, Clone)]
#[model(version = "1.0.0")]
pub struct Album {
	pub id: Uuid,

	#[sync(shared, conflict = "last_writer_wins")]
	pub name: String,

	#[sync(shared, conflict = "union_merge")]
	pub photo_ids: Vec<Uuid>,

	#[sync(shared)]
	pub cover_photo_id: Option<Uuid>,

	#[sync(shared)]
	pub created_at: DateTime<Utc>,

	#[custom_field]
	pub album_type: AlbumType,
}

#[derive(Serialize, Deserialize, Clone)]
pub enum AlbumType {
	Manual,
	Smart,
	Shared,
	Favorites,
	Hidden,
}
