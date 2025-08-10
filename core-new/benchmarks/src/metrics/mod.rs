use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ScenarioResult {
	pub id: uuid::Uuid,
	pub scenario: String,
	pub recipe_name: String,
	#[serde(default, skip_serializing_if = "Vec::is_empty")]
	pub location_paths: Vec<PathBuf>,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub hardware_label: Option<String>,
	pub duration_s: f64,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub discovery_duration_s: Option<f64>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub processing_duration_s: Option<f64>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub content_duration_s: Option<f64>,
	pub files: u64,
	pub files_per_s: f64,
	pub directories: u64,
	pub directories_per_s: f64,
	pub total_gb: f64,
	pub errors: u64,
	pub raw_artifacts: Vec<PathBuf>,
}

pub mod sources;
