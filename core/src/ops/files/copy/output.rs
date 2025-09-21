//! File copy operation output types

use crate::infra::action::output::ActionOutputTrait;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Output from file copy action dispatch
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct FileCopyActionOutput {
	pub job_id: Uuid,
	pub sources_count: usize,
	pub destination: String,
}

impl FileCopyActionOutput {
	pub fn new(job_id: Uuid, sources_count: usize, destination: String) -> Self {
		Self {
			job_id,
			sources_count,
			destination,
		}
	}
}

impl ActionOutputTrait for FileCopyActionOutput {
	fn to_json(&self) -> serde_json::Value {
		serde_json::to_value(self).unwrap_or(serde_json::Value::Null)
	}

	fn display_message(&self) -> String {
		format!(
			"Dispatched file copy job {} for {} source(s) to {}",
			self.job_id, self.sources_count, self.destination
		)
	}

	fn output_type(&self) -> &'static str {
		"file.copy.dispatched"
	}
}
