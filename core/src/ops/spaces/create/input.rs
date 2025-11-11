use serde::{Deserialize, Serialize};
use specta::Type;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct SpaceCreateInput {
	pub name: String,
	pub icon: String,
	pub color: String,
}
