//! Action system - preview/execute pattern
//!
//! Stubs for type-checking. Implementation will call WASM host functions.

use crate::types::*;

/// Action execution context
pub struct ActionContext;

impl ActionContext {
	/// Access VDFS
	pub fn vdfs(&self) -> crate::vdfs::VdfsContext {
		crate::vdfs::VdfsContext
	}
}

/// Action preview (shown to user before execution)
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct ActionPreview {
	pub title: String,
	pub description: String,
	pub changes: Vec<Change>,
	pub reversible: bool,
}

/// A change that will be applied
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub enum Change {
	CreateModel {
		model_type: String,
		data: serde_json::Value,
	},
	UpdateModel {
		model_id: uuid::Uuid,
		field: String,
		operation: String,
		value: serde_json::Value,
	},
	UpdateCustomField {
		entry_id: uuid::Uuid,
		field: String,
		value: serde_json::Value,
	},
	AddTag {
		target: uuid::Uuid,
		tag: String,
	},
	CreateDirectory {
		name: String,
		parent: uuid::Uuid,
	},
	MoveEntry {
		entry: Entry,
		destination: String,
	},
}

/// Action execution result
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct ExecutionResult {
	pub success: bool,
	pub message: String,
}

/// Action result type
pub type ActionResult<T> = std::result::Result<T, crate::types::Error>;


