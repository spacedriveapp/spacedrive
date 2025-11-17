//! Output for getting sync metrics

use crate::service::sync::metrics::snapshot::SyncMetricsSnapshot;
use serde::{Deserialize, Serialize};
use specta::Type;

#[derive(Debug, Serialize, Deserialize, Type)]
pub struct GetSyncMetricsOutput {
	/// The metrics snapshot
	pub metrics: SyncMetricsSnapshot,
}
