//! Output types for get_sync_event_log query

use serde::{Deserialize, Serialize};
use specta::Type;

use crate::infra::sync::SyncEventLog;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct GetSyncEventLogOutput {
	pub events: Vec<SyncEventLog>,
}
