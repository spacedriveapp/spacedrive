use serde::{Deserialize, Serialize};
use specta::Type;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct LayoutUpdateInput {
	pub layout_id: Uuid,
	pub name: Option<String>,
	pub icon: Option<String>,
	pub color: Option<String>,
}
