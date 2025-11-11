use crate::domain::ItemType;
use serde::{Deserialize, Serialize};
use specta::Type;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct AddItemInput {
	pub space_id: Uuid,
	pub group_id: Option<Uuid>, // None = space-level item
	pub item_type: ItemType,
}
