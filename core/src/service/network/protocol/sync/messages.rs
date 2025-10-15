//! Sync protocol messages (Leaderless Hybrid Model)
//!
//! Defines message types for peer-to-peer sync communication:
//! - State-based messages for device-owned data
//! - Log-based messages with HLC for shared resources

use crate::infra::sync::{SharedChangeEntry, HLC};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Sync protocol messages for leaderless hybrid sync
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SyncMessage {
	// === STATE-BASED MESSAGES (Device-Owned Data) ===
	/// Broadcast single state change (location, entry, volume)
	StateChange {
		library_id: Uuid,
		model_type: String,
		record_uuid: Uuid,
		device_id: Uuid, // Owner device
		data: serde_json::Value,
		timestamp: DateTime<Utc>,
	},

	/// Broadcast batch of state changes (efficiency)
	StateBatch {
		library_id: Uuid,
		model_type: String,
		device_id: Uuid,
		records: Vec<StateRecord>,
	},

	/// Request state from peer
	StateRequest {
		library_id: Uuid,
		model_types: Vec<String>,     // e.g., ["location", "entry"]
		device_id: Option<Uuid>,      // Specific device or all
		since: Option<DateTime<Utc>>, // Incremental sync
		checkpoint: Option<String>,   // For resumability
		batch_size: usize,
	},

	/// Response with state
	StateResponse {
		library_id: Uuid,
		model_type: String,
		device_id: Uuid,
		records: Vec<StateRecord>,
		checkpoint: Option<String>,
		has_more: bool,
	},

	// === LOG-BASED MESSAGES (Shared Resources) ===
	/// Broadcast shared resource change (with HLC)
	SharedChange {
		library_id: Uuid,
		entry: SharedChangeEntry,
	},

	/// Broadcast batch of shared changes
	SharedChangeBatch {
		library_id: Uuid,
		entries: Vec<SharedChangeEntry>,
	},

	/// Request shared changes since HLC
	SharedChangeRequest {
		library_id: Uuid,
		since_hlc: Option<HLC>,
		limit: usize,
	},

	/// Response with shared changes
	SharedChangeResponse {
		library_id: Uuid,
		entries: Vec<SharedChangeEntry>,
		current_state: Option<serde_json::Value>, // Fallback if logs pruned
		has_more: bool,
	},

	/// Acknowledge shared changes (for pruning)
	AckSharedChanges {
		library_id: Uuid,
		from_device: Uuid,
		up_to_hlc: HLC,
	},

	// === GENERAL ===
	/// Peer status heartbeat
	Heartbeat {
		library_id: Uuid,
		device_id: Uuid,
		timestamp: DateTime<Utc>,
		state_watermark: Option<DateTime<Utc>>, // Last state sync
		shared_watermark: Option<HLC>,          // Last shared change
	},

	/// Request peer's watermarks for reconnection sync
	WatermarkExchangeRequest {
		library_id: Uuid,
		device_id: Uuid, // Requesting device
		my_state_watermark: Option<DateTime<Utc>>,
		my_shared_watermark: Option<HLC>,
	},

	/// Response with peer's watermarks
	WatermarkExchangeResponse {
		library_id: Uuid,
		device_id: Uuid, // Responding device
		state_watermark: Option<DateTime<Utc>>,
		shared_watermark: Option<HLC>,
		needs_state_catchup: bool,   // If true, peer needs our state
		needs_shared_catchup: bool,  // If true, peer needs our shared changes
	},

	/// Error response
	Error { library_id: Uuid, message: String },
}

/// Single state record in batches
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateRecord {
	pub uuid: Uuid,
	pub data: serde_json::Value,
	pub timestamp: DateTime<Utc>,
}

impl SyncMessage {
	/// Get the library ID this message pertains to
	pub fn library_id(&self) -> Uuid {
		match self {
			SyncMessage::StateChange { library_id, .. }
			| SyncMessage::StateBatch { library_id, .. }
			| SyncMessage::StateRequest { library_id, .. }
			| SyncMessage::StateResponse { library_id, .. }
			| SyncMessage::SharedChange { library_id, .. }
			| SyncMessage::SharedChangeBatch { library_id, .. }
			| SyncMessage::SharedChangeRequest { library_id, .. }
			| SyncMessage::SharedChangeResponse { library_id, .. }
			| SyncMessage::AckSharedChanges { library_id, .. }
			| SyncMessage::Heartbeat { library_id, .. }
			| SyncMessage::WatermarkExchangeRequest { library_id, .. }
			| SyncMessage::WatermarkExchangeResponse { library_id, .. }
			| SyncMessage::Error { library_id, .. } => *library_id,
		}
	}

	/// Check if this is a request message (expects a response)
	pub fn is_request(&self) -> bool {
		matches!(
			self,
			SyncMessage::StateRequest { .. }
				| SyncMessage::SharedChangeRequest { .. }
				| SyncMessage::Heartbeat { .. }
				| SyncMessage::WatermarkExchangeRequest { .. }
		)
	}

	/// Check if this is a notification (no response expected)
	pub fn is_notification(&self) -> bool {
		matches!(
			self,
			SyncMessage::StateChange { .. }
				| SyncMessage::StateBatch { .. }
				| SyncMessage::SharedChange { .. }
				| SyncMessage::SharedChangeBatch { .. }
				| SyncMessage::AckSharedChanges { .. }
		)
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_sync_message_library_id() {
		let library_id = Uuid::new_v4();

		let msg = SyncMessage::StateChange {
			library_id,
			model_type: "location".to_string(),
			record_uuid: Uuid::new_v4(),
			device_id: Uuid::new_v4(),
			data: serde_json::json!({}),
			timestamp: Utc::now(),
		};

		assert_eq!(msg.library_id(), library_id);
	}

	#[test]
	fn test_sync_message_types() {
		let library_id = Uuid::new_v4();

		let request = SyncMessage::StateRequest {
			library_id,
			model_types: vec!["location".to_string()],
			device_id: None,
			since: None,
			checkpoint: None,
			batch_size: 1000,
		};
		assert!(request.is_request());
		assert!(!request.is_notification());

		let change = SyncMessage::StateChange {
			library_id,
			model_type: "location".to_string(),
			record_uuid: Uuid::new_v4(),
			device_id: Uuid::new_v4(),
			data: serde_json::json!({}),
			timestamp: Utc::now(),
		};
		assert!(!change.is_request());
		assert!(change.is_notification());
	}
}
