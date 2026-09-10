//! Location add operation output types

use crate::{domain::addressing::SdPath, infra::action::output::ActionOutputTrait};

use serde::{Deserialize, Serialize};
use specta::Type;
use uuid::Uuid;

/// Output from location add action dispatch
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct LocationAddOutput {
	pub location_id: Uuid,
	pub path: SdPath,
	pub name: Option<String>,
}

impl LocationAddOutput {
	pub fn new(location_id: Uuid, path: SdPath, name: Option<String>) -> Self {
		Self {
			location_id,
			path,
			name,
		}
	}
}

impl ActionOutputTrait for LocationAddOutput {
	fn to_json(&self) -> serde_json::Value {
		serde_json::to_value(self).unwrap_or(serde_json::Value::Null)
	}

	fn display_message(&self) -> String {
		match &self.name {
			Some(name) => format!(
				"Pinned '{}' with ID {} at {}",
				name, self.location_id, self.path
			),
			None => format!("Pinned location {} at {}", self.location_id, self.path),
		}
	}

	fn output_type(&self) -> &'static str {
		"location.add.completed"
	}
}
