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

/// Handle state-based sync messages (device-owned data)
pub struct StateSyncHandler {
	library_id: Uuid,
	db: Arc<Database>,
}

impl StateSyncHandler {
	pub fn new(library_id: Uuid, db: Arc<Database>) -> Self {
		Self { library_id, db }
	}

	/// Handle incoming StateChange message
	///
	/// Uses the Syncable registry for dynamic dispatch - models handle their own application.
	pub async fn handle_state_change(
		&self,
		model_type: String,
		record_uuid: Uuid,
		device_id: Uuid,
		data: serde_json::Value,
		timestamp: DateTime<Utc>,
	) -> Result<()> {
		debug!(
			model_type = %model_type,
			record_uuid = %record_uuid,
			device_id = %device_id,
			"Handling state change"
		);

		// Use registry to apply state change (device-owned models)
		// Each model handles its own upsert logic
		let db = Arc::new(self.db.conn().clone());
		crate::infra::sync::registry::apply_state_change(&model_type, data, db)
			.await
			.map_err(|e| anyhow::anyhow!("{}", e))?;

		Ok(())
	}

	/// Handle StateBatch message
	pub async fn handle_state_batch(
		&self,
		model_type: String,
		device_id: Uuid,
		records: Vec<StateRecord>,
	) -> Result<()> {
		info!(
			model_type = %model_type,
			device_id = %device_id,
			count = records.len(),
			"Handling state batch"
		);

		// Batch apply using registry (device-owned models)
		let db = Arc::new(self.db.conn().clone());
		for record in records {
			crate::infra::sync::registry::apply_state_change(&model_type, record.data, db.clone())
				.await
				.map_err(|e| anyhow::anyhow!("{}", e))?;
		}

		Ok(())
	}

	/// Handle StateRequest message
	pub async fn handle_state_request(
		&self,
		model_types: Vec<String>,
		device_id: Option<Uuid>,
		since: Option<DateTime<Utc>>,
		batch_size: usize,
	) -> Result<Vec<SyncMessage>> {
		let mut responses = Vec::new();

		for model_type in model_types {
			let records = self
				.query_state(&model_type, device_id, since, batch_size)
				.await?;

			if !records.is_empty() {
				responses.push(SyncMessage::StateResponse {
					library_id: self.library_id,
					model_type,
					device_id: device_id.unwrap_or(Uuid::nil()),
					records,
					checkpoint: None, // TODO: Implement checkpointing
					has_more: false,  // TODO: Implement pagination
				});
			}
		}

		Ok(responses)
	}

	/// Query state from database (generic, works for all models)
	async fn query_state(
		&self,
		model_type: &str,
		device_id: Option<Uuid>,
		since: Option<DateTime<Utc>>,
		limit: usize,
	) -> Result<Vec<StateRecord>> {
		// Get table name from registry (no hardcoding!)
		let table_name = crate::infra::sync::registry::get_table_name(model_type)
			.await
			.ok_or_else(|| anyhow::anyhow!("Unknown model type: {}", model_type))?;

		let mut query = format!("SELECT * FROM {} WHERE 1=1", table_name);

		if let Some(dev_id) = device_id {
			query.push_str(&format!(" AND device_id = '{}'", dev_id));
		}

		if let Some(ts) = since {
			query.push_str(&format!(" AND updated_at > '{}'", ts.to_rfc3339()));
		}

		query.push_str(&format!(" LIMIT {}", limit));

		let rows = self
			.db
			.conn()
			.query_all(Statement::from_string(DbBackend::Sqlite, query))
			.await?;

		let mut records = Vec::new();
		for row in rows {
			// TODO: Proper serialization per model type via registry
			let uuid_str: String = row.try_get("", "uuid")?;
			let uuid = Uuid::parse_str(&uuid_str)?;

			records.push(StateRecord {
				uuid,
				data: serde_json::json!({}), // TODO: Serialize row via Syncable trait
				timestamp: Utc::now(),
			});
		}

		Ok(records)
	}
}

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
