//! What this extension keeps in its own store.
//!
//! `Footage` and `Person` are generation: a re-run rebuilds them from the
//! sidecars. `RosterEdit` is the assertion half, the names and corrections
//! a person made, which nothing can rebuild. Edits address a face by the
//! media uuid of its bytes, the shot id and the face's index in that shot.
//! All three derive from the bytes, so an edit made on one device rebinds
//! on another that holds the same footage.

use serde::{Deserialize, Serialize};
use spacedrive_sdk::model;
use spacedrive_sdk::prelude::*;

/// One face in one shot, by keys that survive a move and cross devices.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct FaceRef {
	pub media_uuid: Uuid,
	pub shot_id: String,
	pub index: u32,
}

/// One analyzed video, keyed by its media uuid. The client's per-file shot
/// list reads this, so it carries what the list shows.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[model(version = "1.0.0")]
pub struct Footage {
	pub id: Uuid,
	pub record: Uuid,
	pub source_id: Uuid,
	pub name: String,
	pub relative_path: String,
	pub content_uuid: Option<Uuid>,
	/// `analyzed`, or `waiting_for_shots` until the core splits the video.
	pub status: String,
	pub duration_ms: u64,
	pub shots: Vec<ShotSummary>,
	/// Categories that had no provider on the last run.
	pub unavailable: Vec<String>,
	pub updated_ms: i64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ShotSummary {
	pub id: String,
	pub index: u32,
	pub start_ms: u64,
	pub end_ms: u64,
	pub keyframe: String,
	pub description: Option<String>,
	pub places: Vec<String>,
	pub faces: u32,
	pub people: Vec<Uuid>,
}

/// A cluster of faces, and the name a person gave it.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[model(version = "1.0.0")]
pub struct Person {
	pub id: Uuid,
	pub name: Option<String>,
	pub hidden: bool,
	pub faces: Vec<FaceRef>,
	pub cover: Option<FaceRef>,
	pub centroid: Vec<f32>,
	pub shot_count: u32,
	pub file_count: u32,
	/// The tag this person's name was last applied as, so a rename can
	/// take the old one off.
	pub tag: Option<String>,
}

/// One correction to the roster, replayed after every clustering pass in
/// HLC order with the device as the tiebreak. The stamp comes from the
/// host's assertion clock, the one tag assertions use, so when sync merges
/// two devices' edits they interleave in the order they were made.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[model(version = "1.0.0")]
pub struct RosterEdit {
	pub id: Uuid,
	/// Empty on an edit written before edits were stamped. Those sort first
	/// and keep their order by `at_ms`.
	#[serde(default, deserialize_with = "null_as_default")]
	pub hlc: String,
	#[serde(default, deserialize_with = "null_as_default")]
	pub device_uuid: Uuid,
	/// Wall-clock time. It orders only the unstamped edits.
	pub at_ms: i64,
	/// `name`, `hide`, `show`, `merge`, `assign` or `exclude`.
	pub action: String,
	pub person: Uuid,
	pub name: Option<String>,
	/// The person `merge` folds `person` into.
	pub into: Option<Uuid>,
	/// The face `assign` pins to `person`, or `exclude` takes from it.
	pub face: Option<FaceRef>,
	/// The faces `person` had when `merge` folded it into `into`. They are
	/// pinned to `into` on replay, because two clusters a person merges are
	/// usually ones similarity alone keeps apart.
	#[serde(default, deserialize_with = "null_as_default")]
	pub faces: Vec<FaceRef>,
}

/// A column added to a model after rows were written reads back as null
/// on those rows.
fn null_as_default<'de, D, T>(deserializer: D) -> std::result::Result<T, D::Error>
where
	D: serde::Deserializer<'de>,
	T: Default + Deserialize<'de>,
{
	Ok(Option::<T>::deserialize(deserializer)?.unwrap_or_default())
}

/// The `scene` sidecar: what each shot's keyframe shows.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SceneDoc {
	pub version: u32,
	pub shots: Vec<ShotScene>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ShotScene {
	pub shot_id: String,
	pub description: Option<String>,
	pub places: Vec<SceneTag>,
}

/// The `faces` sidecar: the faces found in each shot's keyframe.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FacesDoc {
	pub version: u32,
	pub shots: Vec<ShotFaces>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ShotFaces {
	pub shot_id: String,
	pub faces: Vec<FaceDetection>,
}
