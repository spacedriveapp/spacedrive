//! Sync protocol messages
//!
//! Defines the message types for push-based sync communication between
//! leader and follower devices.

use crate::infra::sync::{SyncLogEntry, SyncRole};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Sync protocol messages
///
/// These messages enable push-based sync:
/// - Leader pushes NewEntries when changes occur
/// - Follower requests entries via FetchEntries
/// - Leader responds with EntriesResponse
/// - Follower acknowledges with Acknowledge
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SyncMessage {
	/// Leader → Follower: New entries available
	///
	/// Sent immediately when the leader commits changes to the sync log.
	/// Follower should respond with FetchEntries to retrieve the actual data.
	NewEntries {
		library_id: Uuid,
		from_sequence: u64,
		to_sequence: u64,
		entry_count: usize,
	},

	/// Follower → Leader: Request entries
	///
	/// Sent by follower to retrieve sync log entries after receiving NewEntries,
	/// or during catch-up sync.
	FetchEntries {
		library_id: Uuid,
		since_sequence: u64,
		limit: usize, // Max 1000
	},

	/// Leader → Follower: Response with entries
	///
	/// Contains the actual sync log entries requested by FetchEntries.
	EntriesResponse {
		library_id: Uuid,
		entries: Vec<SyncLogEntry>,
		latest_sequence: u64,
		has_more: bool,
	},

	/// Follower → Leader: Acknowledge received
	///
	/// Sent after successfully applying sync entries.
	/// Helps leader track follower progress.
	Acknowledge {
		library_id: Uuid,
		up_to_sequence: u64,
		applied_count: usize,
	},

	/// Bi-directional: Heartbeat
	///
	/// Sent periodically (every 30s) to maintain connection and sync state.
	/// Leader uses this to track follower health.
	/// Follower uses this to detect leader timeout.
	Heartbeat {
		library_id: Uuid,
		current_sequence: u64,
		role: SyncRole,
		timestamp: chrono::DateTime<chrono::Utc>,
	},

	/// Leader → Follower: You're behind, full sync needed
	///
	/// Sent when follower's sequence is too far behind or there's a gap.
	/// Follower should trigger a full sync job.
	SyncRequired {
		library_id: Uuid,
		reason: String,
		leader_sequence: u64,
		follower_sequence: u64,
	},

	/// Error response for any request
	Error { library_id: Uuid, message: String },
}

impl SyncMessage {
	/// Get the library ID this message pertains to
	pub fn library_id(&self) -> Uuid {
		match self {
			SyncMessage::NewEntries { library_id, .. }
			| SyncMessage::FetchEntries { library_id, .. }
			| SyncMessage::EntriesResponse { library_id, .. }
			| SyncMessage::Acknowledge { library_id, .. }
			| SyncMessage::Heartbeat { library_id, .. }
			| SyncMessage::SyncRequired { library_id, .. }
			| SyncMessage::Error { library_id, .. } => *library_id,
		}
	}

	/// Check if this is a request message (expects a response)
	pub fn is_request(&self) -> bool {
		matches!(
			self,
			SyncMessage::FetchEntries { .. } | SyncMessage::Heartbeat { .. }
		)
	}

	/// Check if this is a notification (no response expected)
	pub fn is_notification(&self) -> bool {
		matches!(
			self,
			SyncMessage::NewEntries { .. }
				| SyncMessage::Acknowledge { .. }
				| SyncMessage::SyncRequired { .. }
		)
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_sync_message_library_id() {
		let library_id = Uuid::new_v4();

		let msg = SyncMessage::NewEntries {
			library_id,
			from_sequence: 1,
			to_sequence: 10,
			entry_count: 10,
		};

		assert_eq!(msg.library_id(), library_id);
	}

	#[test]
	fn test_sync_message_types() {
		let library_id = Uuid::new_v4();

		let fetch = SyncMessage::FetchEntries {
			library_id,
			since_sequence: 0,
			limit: 100,
		};
		assert!(fetch.is_request());
		assert!(!fetch.is_notification());

		let new_entries = SyncMessage::NewEntries {
			library_id,
			from_sequence: 1,
			to_sequence: 10,
			entry_count: 10,
		};
		assert!(!new_entries.is_request());
		assert!(new_entries.is_notification());
	}
}
