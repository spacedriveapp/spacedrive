use crate::domain::SpaceItem;
use serde::{Deserialize, Serialize};
use specta::Type;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct AddItemOutput {
	pub item: SpaceItem,
}
