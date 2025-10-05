//! Location remove operation output types

use crate::infra::action::output::ActionOutputTrait;
use serde::{Deserialize, Serialize};
use specta::Type;
use uuid::Uuid;

/// Output from location remove action dispatch
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct LocationRemoveOutput {
	pub location_id: Uuid,
	pub path: Option<String>,
}

impl LocationRemoveOutput {
	pub fn new(location_id: Uuid, path: Option<String>) -> Self {
		Self { location_id, path }
	}
}

impl ActionOutputTrait for LocationRemoveOutput {
	fn to_json(&self) -> serde_json::Value {
		serde_json::to_value(self).unwrap_or(serde_json::Value::Null)
	}

	fn display_message(&self) -> String {
		match &self.path {
			Some(path) => format!("Removed location with ID {} at {}", self.location_id, path),
			None => format!("Removed location with ID {}", self.location_id),
		}
	}

	fn output_type(&self) -> &'static str {
		"location.remove.completed"
	}
}
