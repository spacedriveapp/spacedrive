//! Input for getting sync metrics

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use specta::Type;
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize, Type)]
pub struct GetSyncMetricsInput {
	/// Filter metrics since this time
	pub since: Option<DateTime<Utc>>,
	
	/// Filter metrics for specific peer device
	pub peer_id: Option<Uuid>,
	
	/// Filter metrics for specific model type
	pub model_type: Option<String>,
	
	/// Show only state metrics
	pub state_only: Option<bool>,
	
	/// Show only operation metrics
	pub operations_only: Option<bool>,
	
	/// Show only error metrics
	pub errors_only: Option<bool>,
}