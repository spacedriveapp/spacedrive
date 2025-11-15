//! Transaction Manager - Sole gatekeeper for syncable database writes
//!
//! The TransactionManager ensures that all state-changing writes to sync-enabled
//! models are atomic, logged, and emit appropriate events.

use super::Syncable;
use crate::infra::event::{Event, EventBus};
use chrono::Utc;
use sea_orm::{ActiveModelTrait, DatabaseConnection, DbErr, TransactionTrait};
use std::sync::Arc;
use thiserror::Error;
use tokio::sync::Mutex;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

/// Errors related to transaction management
#[derive(Debug, Error)]
pub enum TxError {
	#[error("Database error: {0}")]
	Database(#[from] DbErr),

	#[error("Sync log error: {0}")]
	SyncLog(String),

	#[error("Serialization error: {0}")]
	Serialization(#[from] serde_json::Error),

	#[error("Invalid model: {0}")]
	InvalidModel(String),
}

pub type Result<T> = std::result::Result<T, TxError>;

/// Bulk operation metadata (for 1K+ item operations)
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct BulkOperationMetadata {
	/// Type of bulk operation
	pub operation: BulkOperation,

	/// Number of items affected
	pub affected_count: u64,

	/// Optional hints for followers (e.g., location path for indexing)
	pub hints: serde_json::Value,
}

/// Types of bulk operations
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum BulkOperation {
	/// Initial indexing of a location
	InitialIndex {
		location_id: Uuid,
		location_path: String,
	},
	/// Bulk tag application
	BulkTag { tag_id: Uuid, entry_count: u64 },
	/// Bulk deletion
	BulkDelete { model_type: String, count: u64 },
}

/// Transaction Manager
///
/// Coordinates atomic writes, sync log creation, and event emission.
/// In the leaderless architecture, all devices can write without role checks.
pub struct TransactionManager {
	/// Dedicated sync event bus for sync coordination events
	sync_events: Arc<crate::infra::sync::SyncEventBus>,

	/// General event bus for non-sync events (if needed)
	event_bus: Arc<EventBus>,

	/// Current sequence number per library (library_id -> sequence)
	/// TODO: Replace with HLC in leaderless architecture
	sync_sequence: Arc<Mutex<std::collections::HashMap<Uuid, u64>>>,
}

impl TransactionManager {
	/// Create a new transaction manager with both event buses
	///
	/// # Arguments
	/// * `sync_events` - Dedicated sync event bus (high priority, large capacity)
	/// * `event_bus` - General event bus (for non-sync events if needed)
	pub fn new(
		sync_events: Arc<crate::infra::sync::SyncEventBus>,
		event_bus: Arc<EventBus>,
	) -> Self {
		Self {
			sync_events,
			event_bus,
			sync_sequence: Arc::new(Mutex::new(std::collections::HashMap::new())),
		}
	}

	/// Get the general event bus
	pub fn event_bus(&self) -> &Arc<EventBus> {
		&self.event_bus
	}

	/// Get the sync event bus
	pub fn sync_events(&self) -> &Arc<crate::infra::sync::SyncEventBus> {
		&self.sync_events
	}

	/// Commit device-owned resource (state-based sync)
	///
	/// For locations, entries, volumes, audit logs - data owned by this device.
	/// Uses simple state broadcast (no log needed).
	///
	/// # Example
	/// ```rust,ignore
	/// let location = location::ActiveModel { ... };
	/// tm.commit_device_owned(library, model, data, device_id).await?;
	/// // → Broadcasts state to peers
	/// ```
	pub async fn commit_device_owned(
		&self,
		library_id: Uuid,
		model_type: &str,
		record_uuid: Uuid,
		device_id: Uuid,
		data: serde_json::Value,
	) -> Result<()> {
		debug!(
			library_id = %library_id,
			model_type = %model_type,
			record_uuid = %record_uuid,
			"Committing device-owned data"
		);

		// Note: Resource events are emitted by callers via ResourceManager
		// to ensure proper domain model structure (not raw DB JSON)

		// Emit to dedicated sync event bus for PeerSync to broadcast to peers
		self.sync_events.emit(crate::infra::sync::SyncEvent::StateChange {
			library_id,
			model_type: model_type.to_string(),
			record_uuid,
			device_id,
			data,
			timestamp: chrono::Utc::now(),
		});

		Ok(())
	}

	/// Commit shared resource (log-based sync with HLC)
	///
	/// For tags, albums, user_metadata - data shared across all devices.
	/// Uses HLC-ordered log for conflict resolution.
	///
	/// # Example
	/// ```rust,ignore
	/// let tag = tag::ActiveModel { ... };
	/// tm.commit_shared(library, model_type, record_uuid, change_type, data, peer_log, hlc_gen).await?;
	/// // → Generates HLC, writes to peer_log, broadcasts
	/// ```
	pub async fn commit_shared(
		&self,
		library_id: Uuid,
		model_type: &str,
		record_uuid: Uuid,
		change_type: crate::infra::sync::ChangeType,
		data: serde_json::Value,
		peer_log: &crate::infra::sync::PeerLog,
		hlc_generator: &mut crate::infra::sync::HLCGenerator,
	) -> Result<()> {
		debug!(
			library_id = %library_id,
			model_type = %model_type,
			record_uuid = %record_uuid,
			change_type = ?change_type,
			"Committing shared data"
		);

		// Generate HLC timestamp for ordering
		let hlc = hlc_generator.next();

		// Create entry for peer log
		let entry = crate::infra::sync::SharedChangeEntry {
			hlc,
			model_type: model_type.to_string(),
			record_uuid,
			change_type,
			data: data.clone(),
		};

		// Write to peer log (append-only, for sync and conflict resolution)
		peer_log
			.append(entry.clone())
			.await
			.map_err(|e| TxError::SyncLog(format!("Failed to append to peer log: {}", e)))?;

		info!(
			hlc = %hlc,
			model_type = %model_type,
			record_uuid = %record_uuid,
			"Shared change written to peer log"
		);

		// Note: Resource events are emitted by callers via ResourceManager
		// to ensure proper domain model structure (not raw DB JSON)

		// Emit to dedicated sync event bus for PeerSync to broadcast to peers
		self.sync_events
			.emit(crate::infra::sync::SyncEvent::SharedChange {
				library_id,
				entry,
			});

		Ok(())
	}

	// OLD METHODS (STUBBED - Will be replaced with HLC-based approach)

	/// Log a single change (DEPRECATED - Use PeerSync directly)
	///
	/// This method is stubbed out and will be removed.
	/// In the new architecture:
	/// - Device-owned data: No log, just broadcast state
	/// - Shared resources: Use PeerLog with HLC
	pub async fn log_change_stubbed(&self, library_id: Uuid) -> Result<u64> {
		warn!("log_change called but is deprecated in leaderless architecture");
		// Return dummy sequence for compatibility
		Ok(self.next_sequence(library_id).await?)
	}

	/// Log batch changes (DEPRECATED - Use PeerSync directly)
	pub async fn log_batch_stubbed(&self, library_id: Uuid, count: usize) -> Result<Vec<u64>> {
		warn!("log_batch called but is deprecated in leaderless architecture");
		let mut sequences = Vec::new();
		for _ in 0..count {
			sequences.push(self.next_sequence(library_id).await?);
		}
		Ok(sequences)
	}

	/// Log bulk operation (DEPRECATED - Use PeerSync directly)
	pub async fn log_bulk_stubbed(
		&self,
		library_id: Uuid,
		metadata: BulkOperationMetadata,
	) -> Result<u64> {
		info!(
			library_id = %library_id,
			operation = ?metadata.operation,
			affected_count = metadata.affected_count,
			"Bulk operation (leaderless - no sync log)"
		);

		// Emit event
		self.event_bus.emit(Event::Custom {
			event_type: "BulkOperationCommitted".to_string(),
			data: serde_json::to_value(&metadata).unwrap_or_default(),
		});

		Ok(self.next_sequence(library_id).await?)
	}

	/// Get the next sequence number for a library
	/// TODO: Replace with HLC in leaderless architecture
	async fn next_sequence(&self, library_id: Uuid) -> Result<u64> {
		let mut sequences = self.sync_sequence.lock().await;
		let seq = sequences.entry(library_id).or_insert(0);
		*seq += 1;
		Ok(*seq)
	}

	/// Emit a generic change event
	pub fn emit_change_event_simple(&self, library_id: Uuid, model_type: &str, record_id: Uuid) {
		self.event_bus.emit(Event::Custom {
			event_type: format!("{}_changed", model_type),
			data: serde_json::json!({
				"library_id": library_id,
				"record_id": record_id,
			}),
		});
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_transaction_manager_creation() {
		let sync_events = Arc::new(crate::infra::sync::SyncEventBus::default());
		let event_bus = Arc::new(EventBus::default());

		let _tm = TransactionManager::new(sync_events, event_bus);
	}
}
