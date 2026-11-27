use crate::domain::Layout;
use serde::{Deserialize, Serialize};
use specta::Type;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct LayoutsListOutput {
	pub layouts: Vec<Layout>,
}
