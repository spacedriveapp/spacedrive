use spacedrive_sdk::{model, persist_strategy};

use serde::{Deserialize, Serialize};
use spacedrive_sdk::prelude::*;
use uuid::Uuid;

pub type PersonId = Uuid;

#[derive(Serialize, Deserialize, Clone)]
#[model(version = "1.0.0")]
#[persist_strategy("always")]
pub struct Person {
	pub id: PersonId,

	#[sync(shared, conflict = "last_writer_wins")]
	pub name: Option<String>,

	#[sync(shared)]
	pub thumbnail_photo_id: Option<Uuid>,

	#[sidecar(kind = "face_embeddings")]
	pub embeddings: Vec<Vec<f32>>,

	#[sync(device_owned)]
	pub photo_count: usize,

	#[vectorized(strategy = "average", model = "registered:face_embedding")]
	pub representative_embedding: Vec<f32>,
}
