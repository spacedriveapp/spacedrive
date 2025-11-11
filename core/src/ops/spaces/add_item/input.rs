use crate::domain::ItemType;
use serde::{Deserialize, Serialize};
use specta::Type;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct AddItemInput {
	pub group_id: Uuid,
	pub item_type: ItemType,
}
