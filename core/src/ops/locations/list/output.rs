use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocationInfo {
	pub id: Uuid,
	pub path: PathBuf,
	pub name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocationsListOutput {
	pub locations: Vec<LocationInfo>,
}

