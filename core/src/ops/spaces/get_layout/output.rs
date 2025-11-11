use crate::domain::{SpaceLayout};
use serde::{Deserialize, Serialize};
use specta::Type;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct SpaceLayoutOutput {
	pub layout: SpaceLayout,
}
