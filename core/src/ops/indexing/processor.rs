//! # Processor Inputs and Results
//!
//! The shape a media processor is handed and the shape it answers with. Every
//! processor in `ops::media` speaks these types, so a caller can drive
//! thumbnails, proxies, OCR and transcripts without knowing which is which.

use super::state::EntryKind;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use uuid::Uuid;

/// Minimal entry snapshot required for content processing without full database models.
#[derive(Debug, Clone)]
pub struct ProcessorEntry {
	pub id: i32,
	pub uuid: Option<Uuid>,
	pub path: PathBuf,
	pub kind: EntryKind,
	pub size: u64,
	pub content_id: Option<i32>,
	pub mime_type: Option<String>,
}

/// Outcome of a single processor run: success/failure, artifacts created, and bytes processed.
#[derive(Debug, Clone)]
pub struct ProcessorResult {
	pub success: bool,
	pub artifacts_created: usize,
	pub bytes_processed: u64,
	pub error: Option<String>,
}

impl ProcessorResult {
	pub fn success(artifacts: usize, bytes: u64) -> Self {
		Self {
			success: true,
			artifacts_created: artifacts,
			bytes_processed: bytes,
			error: None,
		}
	}

	pub fn failure(error: String) -> Self {
		Self {
			success: false,
			artifacts_created: 0,
			bytes_processed: 0,
			error: Some(error),
		}
	}
}

/// Per-processor settings: type, enabled flag, and arbitrary JSON config.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessorConfig {
	pub processor_type: String,
	pub enabled: bool,
	#[serde(default)]
	pub settings: serde_json::Value,
}

/// Collection of processors that run automatically on watcher events for a location.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocationProcessorConfig {
	#[serde(default)]
	pub watcher_processors: Vec<ProcessorConfig>,
}

impl Default for LocationProcessorConfig {
	fn default() -> Self {
		Self {
			watcher_processors: vec![
				ProcessorConfig {
					processor_type: "content_hash".to_string(),
					enabled: true,
					settings: serde_json::json!({}),
				},
				ProcessorConfig {
					processor_type: "thumbnail".to_string(),
					enabled: true,
					settings: serde_json::json!({
						"variants": ["grid@1x", "grid@2x"],
						"quality": 80
					}),
				},
				ProcessorConfig {
					processor_type: "thumbstrip".to_string(),
					enabled: true, // ~6s per video, acceptable for auto-generation.
					settings: serde_json::json!({
						"variants": ["thumbstrip_preview"],
						"regenerate": false
					}),
				},
				ProcessorConfig {
					processor_type: "proxy".to_string(),
					enabled: false, // User opt-in required (~8s per video).
					settings: serde_json::json!({
						"enabled": false,
						"max_file_size_gb": 5,
						"use_hardware_accel": true
					}),
				},
				ProcessorConfig {
					processor_type: "ocr".to_string(),
					enabled: false, // Expensive, user opt-in.
					settings: serde_json::json!({
						"languages": ["eng"],
						"min_confidence": 0.6
					}),
				},
				ProcessorConfig {
					processor_type: "speech_to_text".to_string(),
					enabled: false, // Very expensive, user opt-in.
					settings: serde_json::json!({
						"model": "base",
						"language": null
					}),
				},
			],
		}
	}
}
