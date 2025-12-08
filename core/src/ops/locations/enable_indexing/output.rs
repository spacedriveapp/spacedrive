use serde::{Deserialize, Serialize};
use specta::Type;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct EnableIndexingOutput {
	/// UUID of the location that had indexing enabled
	pub location_id: Uuid,

	/// Job ID of the indexing job that was started
	pub job_id: String,
}

impl EnableIndexingOutput {
	pub fn new(location_id: Uuid, job_id: String) -> Self {
		Self {
			location_id,
			job_id,
		}
	}
}
