use spacedrive_sdk::model;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use spacedrive_sdk::prelude::*;
use uuid::Uuid;

use super::{MomentId, PersonId, PlaceId};

#[derive(Serialize, Deserialize, Clone)]
#[model(version = "1.0.0")]
pub struct Photo {
	pub id: Uuid,

	#[entry(filter = "*.{jpg,jpeg,png,heic,heif,raw,cr2,nef,dng}")]
	pub file: Entry,

	#[metadata]
	pub exif: Option<ExifData>,

	#[sidecar(kind = "faces", extension_owned)]
	pub detected_faces: Option<Vec<FaceDetection>>,

	#[sidecar(kind = "scene", extension_owned)]
	pub scene_tags: Option<Vec<SceneTag>>,

	#[sidecar(kind = "aesthetics", extension_owned)]
	pub quality_score: Option<f32>,

	#[user_metadata]
	pub tags: Vec<Tag>,

	#[custom_field]
	pub identified_people: Vec<PersonId>,

	#[custom_field]
	pub place_id: Option<PlaceId>,

	#[custom_field]
	pub moment_id: Option<MomentId>,

	#[computed]
	pub has_faces: bool,

	#[computed]
	pub taken_at: Option<DateTime<Utc>>,
}

impl Photo {
	pub fn from_entry(entry: Entry) -> Self {
		Self {
			id: entry.id(),
			file: entry.clone(),
			exif: None,
			detected_faces: None,
			scene_tags: None,
			quality_score: None,
			tags: vec![],
			identified_people: vec![],
			place_id: None,
			moment_id: None,
			has_faces: false,
			taken_at: None,
		}
	}
}

#[derive(Serialize, Deserialize, Clone)]
pub struct ExifData {
	pub camera_make: Option<String>,
	pub camera_model: Option<String>,
	pub lens_model: Option<String>,
	pub focal_length: Option<f32>,
	pub aperture: Option<f32>,
	pub iso: Option<u32>,
	pub shutter_speed: Option<String>,
	pub taken_at: Option<DateTime<Utc>>,
	pub gps: Option<GpsCoordinates>,
	pub orientation: Option<u8>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct GpsCoordinates {
	pub latitude: f64,
	pub longitude: f64,
	pub altitude: Option<f64>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct SceneTag {
	pub label: String,
	pub confidence: f32,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct FaceDetection {
	pub bbox: BoundingBox,
	pub confidence: f32,
	pub embedding: Vec<f32>,
	pub identified_as: Option<PersonId>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct BoundingBox {
	pub x: f32,
	pub y: f32,
	pub width: f32,
	pub height: f32,
}
