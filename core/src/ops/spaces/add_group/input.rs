use crate::domain::GroupType;
use serde::{Deserialize, Serialize};
use specta::Type;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct AddGroupInput {
	pub space_id: Uuid,
	pub name: String,
	pub group_type: GroupType,
}
