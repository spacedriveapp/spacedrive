//! The `ai.infer` calls this extension makes, one per keyframe.
//!
//! The core picks the model for a category, so the name after the colon is
//! only a label. Face detection fills each face's embedding when a
//! `face_embedding` model is installed, which is why the manifest grants
//! both categories.

use serde::de::DeserializeOwned;
use spacedrive_sdk::prelude::*;

fn infer<O: DeserializeOwned>(category: &str, task: &str, image: &[u8]) -> Result<O> {
	spacedrive_sdk::ffi::op_framed(
		"ai.infer",
		&serde_json::json!({ "model": format!("{category}:videos"), "task": task }),
		image,
	)
}

pub fn caption(image: &[u8]) -> Result<String> {
	infer("image_caption", "caption", image)
}

pub fn classify(image: &[u8]) -> Result<Vec<SceneTag>> {
	infer("scene_classification", "classify", image)
}

pub fn detect_faces(image: &[u8]) -> Result<Vec<FaceDetection>> {
	infer("face_detection", "detect_faces", image)
}
