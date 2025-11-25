//! Get sync activity output

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use specta::Type;
use std::collections::HashMap;
use uuid::Uuid;

use crate::service::sync::state::DeviceSyncState;

/// Sync activity summary for the UI
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct GetSyncActivityOutput {
	pub current_state: DeviceSyncState,
	pub peers: Vec<PeerActivity>,
	pub error_count: u64,
}

/// Per-peer activity information
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct PeerActivity {
	pub device_id: Uuid,
	pub device_name: String,
	pub is_online: bool,
	pub last_seen: DateTime<Utc>,
	pub entries_received: u64,
	pub bytes_received: u64,
	pub bytes_sent: u64,
	pub watermark_lag_ms: Option<u64>,
}
