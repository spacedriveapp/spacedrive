use serde::{Deserialize, Serialize};
use specta::Type;
use std::path::PathBuf;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct LocationInfo {
	pub id: Uuid,
	pub path: PathBuf,
	pub name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct LocationsListOutput {
	pub locations: Vec<LocationInfo>,
}
