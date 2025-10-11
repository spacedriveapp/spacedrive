use serde::{Deserialize, Serialize};

/// Extension configuration with user-facing settings
///
/// The `#[setting(...)]` attributes would be processed by a `#[derive(ExtensionConfig)]` macro
/// to generate metadata for Spacedrive's configuration UI. For the stub implementation,
/// we use doc comments to show the intended design.
#[derive(Serialize, Deserialize)]
pub struct PhotosConfig {
	/// Enable Face Recognition
	///
	/// Setting: label = "Enable Face Recognition", default = true
	#[serde(default = "default_true")]
	pub face_recognition: bool,

	/// Enable Place Identification
	///
	/// Setting: label = "Enable Place Identification", default = true
	#[serde(default = "default_true")]
	pub place_identification: bool,

	/// Automatically Create Memories
	///
	/// Setting: label = "Automatically Create Memories", default = true
	#[serde(default = "default_true")]
	pub auto_memories: bool,

	/// Scene Detection Confidence
	///
	/// Setting: label = "Scene Detection Confidence", default = 0.7, min = 0.0, max = 1.0
	#[serde(default = "default_scene_confidence")]
	pub scene_confidence_threshold: f32,

	/// Face Clustering Threshold
	///
	/// Setting: label = "Face Clustering Threshold", default = 0.6, min = 0.0, max = 1.0
	#[serde(default = "default_face_threshold")]
	pub face_clustering_threshold: f32,
}

fn default_true() -> bool {
	true
}

fn default_scene_confidence() -> f32 {
	0.7
}

fn default_face_threshold() -> f32 {
	0.6
}
