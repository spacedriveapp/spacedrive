//! Protocol handler for log-based sync
//!
//! Uses the Syncable trait registry for polymorphic dispatch - NO SWITCH STATEMENTS!

use crate::{
	infra::{
		db::Database,
		sync::{SharedChangeEntry, HLC},
	},
	service::network::protocol::sync::messages::SyncMessage,
};
use anyhow::Result;
use std::sync::Arc;
use tracing::{debug, info};
use uuid::Uuid;

use super::peer::PeerSync;

/// Handle log-based sync messages (shared resources)
pub struct LogSyncHandler {
	library_id: Uuid,
	db: Arc<Database>,
	peer_sync: Arc<PeerSync>,
}

impl LogSyncHandler {
	pub fn new(library_id: Uuid, db: Arc<Database>, peer_sync: Arc<PeerSync>) -> Self {
		Self {
			library_id,
			db,
			peer_sync,
		}
	}

	/// Handle incoming SharedChange message
	///
	/// Uses the Syncable registry with conflict resolution strategies.
	pub async fn handle_shared_change(&self, entry: SharedChangeEntry) -> Result<()> {
		debug!(
			hlc = %entry.hlc,
			model_type = %entry.model_type,
			record_uuid = %entry.record_uuid,
			change_type = ?entry.change_type,
			"Handling shared change"
		);

		// Use registry to apply with conflict resolution (shared models)
		// Models implement their own merge strategies (union, LWW, etc.)

		// Extract HLC info before moving entry
		let hlc_device_id = entry.hlc.device_id;
		let hlc = entry.hlc;

		let db = Arc::new(self.peer_sync.db().as_ref().clone());
		crate::infra::sync::registry::apply_shared_change(entry, db)
			.await
			.map_err(|e| anyhow::anyhow!("{}", e))?;

		// Send ACK to sender
		self.peer_sync.on_ack_received(hlc_device_id, hlc).await?;

		Ok(())
	}

	/// Handle SharedChangeBatch message
	pub async fn handle_shared_batch(&self, entries: Vec<SharedChangeEntry>) -> Result<()> {
		info!(count = entries.len(), "Handling shared change batch");

		// Sort by HLC (apply in order)
		let mut sorted = entries;
		sorted.sort_by_key(|e| e.hlc);

		for entry in sorted {
			self.handle_shared_change(entry).await?;
		}

		Ok(())
	}

	/// Handle SharedChangeRequest message
	pub async fn handle_shared_request(
		&self,
		since_hlc: Option<HLC>,
		limit: usize,
	) -> Result<SyncMessage> {
		// Get changes from our peer log
		let entries = self.peer_sync.peer_log.get_since(since_hlc).await?;

		let has_more = entries.len() >= limit;
		let limited: Vec<_> = entries.into_iter().take(limit).collect();

		// If logs were pruned and no since_hlc, include current state as fallback
		let current_state = if since_hlc.is_none() && limited.is_empty() {
			Some(self.get_current_shared_state().await?)
		} else {
			None
		};

		Ok(SyncMessage::SharedChangeResponse {
			library_id: self.library_id,
			entries: limited,
			current_state,
			has_more,
		})
	}

	/// Get current state of all shared resources (fallback when logs pruned)
	async fn get_current_shared_state(&self) -> Result<serde_json::Value> {
		// TODO: Query via registry instead of hardcoding
		// registry.get_all_models_of_type("shared") -> serialize
		Ok(serde_json::json!({
			"tags": [],
			"albums": [],
			"user_metadata": [],
		}))
	}
}
