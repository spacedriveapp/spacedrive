use crate::jobs::types::copier::CopyStats;
use serde::{Deserialize, Serialize};
use serde_json;
use specta::Type;
use std::{collections::HashMap, path::PathBuf};
use strum::{Display, EnumString};
use uuid::Uuid;

#[derive(
	Debug, Serialize, Deserialize, EnumString, Display, Clone, Copy, Type, Hash, PartialEq, Eq,
)]
#[strum(use_phf, serialize_all = "snake_case")]
pub enum JobName {
	Indexer,
	FileIdentifier,
	MediaProcessor,
	Copy,
	Move,
	Delete,
	Erase,
	FileValidator,
}

#[derive(Debug, Serialize, Deserialize, Type, Clone)]
#[serde(rename_all = "snake_case")]
#[serde(tag = "type", content = "data")]
pub enum ReportOutputMetadata {
	Metrics(HashMap<String, serde_json::Value>),
	Indexer {
		total_paths: (u32, u32),
	},
	FileIdentifier {
		total_orphan_paths: (u32, u32),
		total_objects_created: (u32, u32),
		total_objects_linked: (u32, u32),
	},
	MediaProcessor {
		media_data_extracted: (u32, u32),
		media_data_skipped: (u32, u32),
		thumbnails_generated: (u32, u32),
		thumbnails_skipped: (u32, u32),
	},
	Copier(CopyStats),
	Deleter {
		location_id: Uuid,
		file_path_ids: Vec<Uuid>,
	},
	FileValidator {
		location_id: Uuid,
		sub_path: Option<PathBuf>,
	},

	// DEPRECATED
	Mover {
		source_location_id: Uuid,
		target_location_id: Uuid,
		sources_file_path_ids: Vec<Uuid>,
		target_location_relative_directory_path: PathBuf,
	},

	Eraser {
		location_id: Uuid,
		file_path_ids: Vec<Uuid>,
		passes: u32,
	},
}
