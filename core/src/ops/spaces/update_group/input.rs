use serde::{Deserialize, Serialize};
use specta::Type;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct UpdateGroupInput {
	pub group_id: Uuid,
	pub name: Option<String>,
	pub is_collapsed: Option<bool>,
}
