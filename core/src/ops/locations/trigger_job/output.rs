//! Location trigger job output

use super::action::JobType;
use serde::{Deserialize, Serialize};
use specta::Type;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct LocationTriggerJobOutput {
	/// UUID of the dispatched job
	pub job_id: Uuid,

	/// Type of job that was triggered
	pub job_type: JobType,

	/// UUID of the location the job is running on
	pub location_id: Uuid,
}
