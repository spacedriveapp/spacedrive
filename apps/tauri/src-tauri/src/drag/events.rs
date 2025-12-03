use super::*;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DragBeganEvent {
	pub session_id: String,
	pub source_window: String,
	pub items: Vec<DragItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DragMoveEvent {
	pub session_id: String,
	pub x: f64,
	pub y: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DragWindowEvent {
	pub session_id: String,
	pub window_label: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DragEndEvent {
	pub session_id: String,
	pub result: DragResult,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum DragResult {
	Dropped {
		operation: DragOperation,
		target: Option<String>,
	},
	Cancelled,
	Failed {
		error: String,
	},
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DropEvent {
	pub window_label: String,
	pub items: Vec<DragItem>,
	pub position: (f64, f64),
}
