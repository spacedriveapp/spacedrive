use crate::domain::{location::JobPolicies, SdPath};
use sea_orm::prelude::DateTimeUtc;
use serde::{Deserialize, Serialize};
use specta::Type;
use std::path::PathBuf;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct LocationInfo {
	pub id: Uuid,
	pub path: PathBuf,
	pub name: Option<String>,
	pub sd_path: SdPath,
	#[serde(default)]
	pub job_policies: JobPolicies,
	pub index_mode: String,
	pub scan_state: String,
	pub last_scan_at: Option<DateTimeUtc>,
	pub error_message: Option<String>,
	pub total_file_count: i64,
	pub total_byte_size: i64,
	pub created_at: DateTimeUtc,
	pub updated_at: DateTimeUtc,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct LocationsListOutput {
	pub locations: Vec<LocationInfo>,
}
