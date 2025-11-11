use crate::domain::SpaceGroup;
use serde::{Deserialize, Serialize};
use specta::Type;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct AddGroupOutput {
	pub group: SpaceGroup,
}
