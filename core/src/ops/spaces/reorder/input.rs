use serde::{Deserialize, Serialize};
use specta::Type;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct ReorderGroupsInput {
	pub space_id: Uuid,
	pub group_ids: Vec<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct ReorderItemsInput {
	pub group_id: Option<Uuid>,
	pub item_ids: Vec<Uuid>,
}
