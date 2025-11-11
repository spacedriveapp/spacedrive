use serde::{Deserialize, Serialize};
use specta::Type;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct SpaceUpdateInput {
	pub space_id: Uuid,
	pub name: Option<String>,
	pub icon: Option<String>,
	pub color: Option<String>,
}
