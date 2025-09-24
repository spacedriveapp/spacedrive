//! Location rescan operation output

use crate::infra::action::output::ActionOutputTrait;
use serde::{Deserialize, Serialize};
use specta::Type;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct LocationRescanOutput {
	pub location_id: Uuid,
	pub location_path: String,
	pub job_id: Uuid,
	pub full_rescan: bool,
}

impl ActionOutputTrait for LocationRescanOutput {
	fn to_json(&self) -> serde_json::Value {
		serde_json::to_value(self).unwrap_or(serde_json::Value::Null)
	}

	fn display_message(&self) -> String {
		let scan_type = if self.full_rescan { "Full" } else { "Quick" };
		format!(
			"{} rescan started for location {} (job: {})",
			scan_type, self.location_path, self.job_id
		)
	}

	fn output_type(&self) -> &'static str {
		"location.rescan.output"
	}
}
