//! Device revoke operation output

use crate::infra::actions::output::ActionOutputTrait;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceRevokeOutput {
	pub device_id: Uuid,
	pub device_name: String,
	pub reason: Option<String>,
}

impl ActionOutputTrait for DeviceRevokeOutput {
	fn to_json(&self) -> serde_json::Value {
		serde_json::to_value(self).unwrap_or(serde_json::Value::Null)
	}

	fn display_message(&self) -> String {
		match &self.reason {
			Some(reason) => format!(
				"Revoked device '{}' ({}): {}",
				self.device_name, self.device_id, reason
			),
			None => format!("Revoked device '{}' ({})", self.device_name, self.device_id),
		}
	}

	fn output_type(&self) -> &'static str {
		"device.revoke.output"
	}
}
