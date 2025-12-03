//! Protocol handlers for state-based and log-based sync
//!
//! Uses the Syncable trait registry for polymorphic dispatch - NO SWITCH STATEMENTS!

use crate::{
	infra::{
		db::Database,
		sync::{ChangeType, SharedChangeEntry, HLC},
	},
	service::network::protocol::sync::messages::{StateRecord, SyncMessage},
};
use anyhow::Result;
use chrono::{DateTime, Utc};
use sea_orm::{ConnectionTrait, DatabaseConnection, DbBackend, Statement};
use std::sync::Arc;
use tracing::{debug, info, warn};
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

		// For initial sync (no watermark), always include current state
		// This ensures shared resources like content_identities are available
		// even if they weren't recorded in peer_log (e.g., created before sync was enabled)
		let current_state = if since_hlc.is_none() {
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
		// Query all shared models via registry
		let db = Arc::new(self.db.conn().clone());
		let results = crate::infra::sync::registry::query_all_shared_models(
			None,    // No watermark - get everything
			100_000, // Large batch to get all records
			db,
		)
		.await
		.map_err(|e| anyhow::anyhow!("Failed to query shared models: {}", e))?;

		// Convert to JSON format expected by backfill
		// Format: { "model_type": [{ "uuid": "...", "data": {...} }, ...] }
		let mut json_map = serde_json::Map::new();

		for (model_type, records) in results {
			let records_json: Vec<serde_json::Value> = records
				.into_iter()
				.map(|(uuid, data, _timestamp)| {
					serde_json::json!({
						"uuid": uuid.to_string(),
						"data": data,
					})
				})
				.collect();

			json_map.insert(model_type, serde_json::Value::Array(records_json));
		}

		Ok(serde_json::Value::Object(json_map))
	}
}
