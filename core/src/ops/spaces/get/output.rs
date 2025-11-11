use crate::domain::Space;
use serde::{Deserialize, Serialize};
use specta::Type;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct SpaceGetOutput {
	pub space: Space,
}
