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
		checkpoint: Option<String>,
		batch_size: usize,
	) -> Result<Vec<SyncMessage>> {
		let mut responses = Vec::new();

		for model_type in model_types {
			// Parse checkpoint to get cursor (timestamp + uuid tie-breaker)
			let cursor = checkpoint.as_ref().and_then(|chk| {
				let parts: Vec<&str> = chk.split('|').collect();
				if parts.len() == 2 {
					let ts = DateTime::parse_from_rfc3339(parts[0])
						.ok()?
						.with_timezone(&chrono::Utc);
					let uuid = Uuid::parse_str(parts[1]).ok()?;
					Some((ts, uuid))
				} else {
					None
				}
			});

			let records = self
				.query_state(&model_type, device_id, since, cursor, batch_size)
				.await?;

			// Query tombstones if this is an incremental sync
			let deleted_uuids = if let Some(since_time) = since {
				self.query_tombstones(&model_type, device_id, since_time)
					.await?
			} else {
				vec![] // Full sync doesn't need tombstones
			};

			if !records.is_empty() || !deleted_uuids.is_empty() {
				// If we got exactly batch_size records, there may be more
				let has_more = records.len() >= batch_size;

				// Create checkpoint: "timestamp|uuid" format
				let next_checkpoint = if has_more {
					records.last().map(|r| {
						format!("{}|{}", r.timestamp.to_rfc3339(), r.uuid)
					})
				} else {
					None
				};

				responses.push(SyncMessage::StateResponse {
					library_id: self.library_id,
					model_type,
					device_id: device_id.unwrap_or(Uuid::nil()),
					records,
					deleted_uuids,
					checkpoint: next_checkpoint,
					has_more,
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
		cursor: Option<(DateTime<Utc>, Uuid)>,
		limit: usize,
	) -> Result<Vec<StateRecord>> {
		// Get table name from registry (no hardcoding!)
		let table_name = crate::infra::sync::registry::get_table_name(model_type)
			.await
			.ok_or_else(|| anyhow::anyhow!("Unknown model type: {}", model_type))?;

		// Build parameterized query to prevent SQL injection
		let mut values: Vec<sea_orm::Value> = Vec::new();
		let mut conditions = Vec::new();

		if let Some(dev_id) = device_id {
			conditions.push("device_id = ?");
			values.push(dev_id.to_string().into());
		}

		if let Some(ts) = since {
			conditions.push("updated_at > ?");
			values.push(ts.to_rfc3339().into());
		}

		let where_clause = if conditions.is_empty() {
			String::new()
		} else {
			format!(" WHERE {}", conditions.join(" AND "))
		};

		// Cursor-based pagination with tie-breaker
		// Handles batches with identical timestamps (common during indexing)
		let pagination_clause = if let Some((cursor_ts, cursor_uuid)) = cursor {
			values.push(cursor_ts.to_rfc3339().into());
			values.push(cursor_uuid.to_string().into());
			" AND ((updated_at > ?) OR (updated_at = ? AND uuid > ?))"
		} else {
			""
		};

		// Repeat cursor_ts value for the equality check
		if cursor.is_some() {
			let (cursor_ts, _) = cursor.unwrap();
			values.push(cursor_ts.to_rfc3339().into());
		}

		// Order by updated_at for logical ordering, uuid as tiebreaker for determinism
		let query = format!(
			"SELECT * FROM {}{}{} ORDER BY updated_at ASC, uuid ASC LIMIT ?",
			table_name, where_clause, pagination_clause
		);
		values.push((limit as i64).into());

		let rows = self
			.db
			.conn()
			.query_all(Statement::from_sql_and_values(
				DbBackend::Sqlite,
				&query,
				values,
			))
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

	/// Query deletion tombstones for incremental sync
	async fn query_tombstones(
		&self,
		model_type: &str,
		device_id: Option<Uuid>,
		since: DateTime<Utc>,
	) -> Result<Vec<Uuid>> {
		use crate::infra::db::entities::device_state_tombstone;
		use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};

		let mut query = device_state_tombstone::Entity::find()
			.filter(device_state_tombstone::Column::ModelType.eq(model_type))
			.filter(device_state_tombstone::Column::DeletedAt.gte(since));

		// Filter by device if specified
		if let Some(dev_id) = device_id {
			// Map device UUID to local ID
			if let Some(device) = crate::infra::db::entities::device::Entity::find()
				.filter(crate::infra::db::entities::device::Column::Uuid.eq(dev_id))
				.one(self.db.conn())
				.await?
			{
				query = query.filter(device_state_tombstone::Column::DeviceId.eq(device.id));
			} else {
				// Device not found, no tombstones
				return Ok(vec![]);
			}
		}

		let tombstones = query.all(self.db.conn()).await?;

		Ok(tombstones.into_iter().map(|t| t.record_uuid).collect())
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
			None,     // No watermark - get everything
			100_000,  // Large batch to get all records
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
