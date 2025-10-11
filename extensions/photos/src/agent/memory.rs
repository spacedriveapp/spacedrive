use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use spacedrive_sdk::prelude::*;
use spacedrive_sdk::{agent_memory, memory_config};
use uuid::Uuid;

use crate::models::*;

#[agent_memory]
#[memory_config(decay_rate = 0.01, summarization_trigger = 500)]
pub struct PhotosMind {
	pub history: TemporalMemory<PhotoEvent>,
	pub knowledge: AssociativeMemory<PhotoKnowledge>,
	pub plan: WorkingMemory<AnalysisPlan>,
}

#[derive(Serialize, Deserialize, Clone)]
pub enum PhotoEvent {
	PhotoAnalyzed {
		photo_id: Uuid,
		faces_detected: usize,
		scene_tags: Vec<String>,
		location: Option<GpsCoordinates>,
	},
	PersonIdentified {
		person_id: PersonId,
		photo_id: Uuid,
		confidence: f32,
	},
	MomentCreated {
		moment_id: MomentId,
		photo_count: usize,
		date_range: (DateTime<Utc>, DateTime<Utc>),
	},
}

impl MemoryVariant for PhotoEvent {
	fn variant_name(&self) -> &'static str {
		match self {
			PhotoEvent::PhotoAnalyzed { .. } => "PhotoAnalyzed",
			PhotoEvent::PersonIdentified { .. } => "PersonIdentified",
			PhotoEvent::MomentCreated { .. } => "MomentCreated",
		}
	}
}

#[derive(Serialize, Deserialize, Clone)]
pub enum PhotoKnowledge {
	FaceCluster {
		person_id: PersonId,
		representative_embedding: Vec<f32>,
		photo_ids: Vec<Uuid>,
	},
	PlaceCluster {
		place_id: PlaceId,
		center: GpsCoordinates,
		photos: Vec<Uuid>,
	},
	ScenePattern {
		scene_type: String,
		typical_times: Vec<u8>,
		common_locations: Vec<PlaceId>,
	},
}

impl MemoryVariant for PhotoKnowledge {
	fn variant_name(&self) -> &'static str {
		match self {
			PhotoKnowledge::FaceCluster { .. } => "FaceCluster",
			PhotoKnowledge::PlaceCluster { .. } => "PlaceCluster",
			PhotoKnowledge::ScenePattern { .. } => "ScenePattern",
		}
	}
}

#[derive(Serialize, Deserialize, Clone, Default)]
pub struct AnalysisPlan {
	pub pending_locations: Vec<SdPath>,
	pub photos_needing_faces: Vec<Uuid>,
	pub photos_needing_clustering: Vec<Uuid>,
	pub moments_to_generate: Vec<DateRange>,
}

impl PhotosMind {
	pub async fn photos_of_person(&self, person_id: PersonId) -> Vec<Uuid> {
		self.knowledge
			.query()
			.where_field("person_id", equals(person_id))
			.collect()
			.await
			.unwrap_or_default()
			.into_iter()
			.flat_map(|k| match k {
				PhotoKnowledge::FaceCluster { photo_ids, .. } => photo_ids,
				_ => vec![],
			})
			.collect()
	}

	pub async fn photos_at_place(&self, place_id: PlaceId) -> Vec<Uuid> {
		self.knowledge
			.query()
			.where_field("place_id", equals(place_id))
			.top_k(1000)
			.collect()
			.await
			.unwrap_or_default()
			.into_iter()
			.flat_map(|k| match k {
				PhotoKnowledge::PlaceCluster { photos, .. } => photos,
				_ => vec![],
			})
			.collect()
	}

	pub async fn similar_scenes(&self, scene_type: &str) -> Vec<String> {
		self.knowledge
			.query_similar(scene_type)
			.min_similarity(0.8)
			.top_k(5)
			.collect()
			.await
			.unwrap_or_default()
			.into_iter()
			.filter_map(|k| match k {
				PhotoKnowledge::ScenePattern { scene_type, .. } => Some(scene_type),
				_ => None,
			})
			.collect()
	}
}
