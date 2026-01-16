//! Volume list output

use crate::domain::volume::Volume;
use serde::{Deserialize, Serialize};
use specta::Type;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct VolumeListOutput {
	pub volumes: Vec<Volume>,
}
