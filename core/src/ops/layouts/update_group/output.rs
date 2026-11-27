use crate::domain::LayoutGroup;
use serde::{Deserialize, Serialize};
use specta::Type;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct UpdateGroupOutput {
	pub group: LayoutGroup,
}
