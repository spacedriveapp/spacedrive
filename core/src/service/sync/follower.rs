//! Follower sync handler
//!
//! Handles follower-side sync: listening for NewEntries and applying changes locally.

use super::SyncApplier;
use crate::infra::sync::{SyncLogDb, SyncLogEntry};
use crate::library::Library;
use anyhow::Result;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
use tokio::time;
use tracing::{debug, info, warn};
use uuid::Uuid;

/// Follower sync handler
///
/// Listens for push notifications from the leader and applies changes locally.
pub struct FollowerSync {
	library_id: Uuid,
	sync_log_db: Arc<SyncLogDb>,
	last_synced_sequence: Arc<Mutex<u64>>,
	applier: Arc<SyncApplier>,
}

impl FollowerSync {
	/// Create a new follower sync handler
	pub async fn new_with_deps(
		library_id: Uuid,
		sync_log_db: Arc<SyncLogDb>,
		db: Arc<crate::infra::db::Database>,
	) -> Result<Self> {
		info!(library_id = %library_id, "Creating follower sync handler");

		// Get last synced sequence from sync log
		let last_synced = sync_log_db.latest_sequence().await.unwrap_or(0);

		// Create applier
		let applier = Arc::new(SyncApplier::new_with_deps(library_id, db));

		Ok(Self {
			library_id,
			sync_log_db,
			last_synced_sequence: Arc::new(Mutex::new(last_synced)),
			applier,
		})
	}

	/// Run the follower sync loop
	///
	/// For now, this is a placeholder. In Phase 2.5, this will:
	/// 1. Listen for NewEntries push notifications via SyncProtocolHandler
	/// 2. Request entries from leader
	/// 3. Apply entries locally
	/// 4. Send acknowledge
	pub async fn run(&self) {
		info!(library_id = %self.library_id, "Starting follower sync loop");

		// Heartbeat loop (sends heartbeat every 30s)
		let mut interval = time::interval(Duration::from_secs(30));

		loop {
			interval.tick().await;

			// Send heartbeat to leader
			self.send_heartbeat().await;

			// TODO: In Phase 2.5, also listen for incoming NewEntries notifications
			// For now, just maintain heartbeat
		}
	}

	/// Send heartbeat to leader
	async fn send_heartbeat(&self) {
		let current_sequence = *self.last_synced_sequence.lock().await;

		debug!(
			library_id = %self.library_id,
			sequence = current_sequence,
			"Sending heartbeat to leader"
		);

		// TODO: Send via SyncProtocolHandler when networking integration is complete
		// let heartbeat = SyncMessage::Heartbeat {
		//     library_id,
		//     current_sequence,
		//     role: SyncRole::Follower,
		//     timestamp: Utc::now(),
		// };
		// protocol_handler.send_message(leader_device_id, heartbeat).await;
	}

	/// Apply sync entries received from leader
	pub async fn apply_entries(&self, entries: Vec<SyncLogEntry>) -> Result<()> {
		info!(
			library_id = %self.library_id,
			entry_count = entries.len(),
			"Applying sync entries from leader"
		);

		for entry in entries {
			// Apply entry
			self.applier.apply_entry(&entry).await?;

			// Update last synced sequence
			*self.last_synced_sequence.lock().await = entry.sequence;

			debug!(
				library_id = %self.library_id,
				sequence = entry.sequence,
				model_type = %entry.model_type,
				"Applied sync entry"
			);
		}

		Ok(())
	}

	/// Get the last synced sequence
	pub async fn last_synced_sequence(&self) -> u64 {
		*self.last_synced_sequence.lock().await
	}
}
