use crate::domain::SdPath;
use serde::{Deserialize, Serialize};
use specta::Type;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct SuggestedLocation {
	pub name: String,
	pub path: PathBuf,
	pub sd_path: SdPath,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct SuggestedLocationsOutput {
	pub locations: Vec<SuggestedLocation>,
}
