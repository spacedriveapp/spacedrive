//! Transaction Manager - Sole gatekeeper for syncable database writes
//!
//! The TransactionManager ensures that all state-changing writes to sync-enabled
//! models are atomic, logged, and emit appropriate events.
//!
//! ## Leaderless Architecture (NEW)
//!
//! In the new leaderless model, this will be simplified to:
//! - Device-owned data: Just emit events (state-based sync)
//! - Shared resources: Use HLC + PeerLog (log-based sync)
//!
//! ## Current Status
//!
//! This file is in transition. The old sync log methods are stubbed out
//! and will be replaced with HLC-based methods.

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
	/// Event bus for emitting events after successful commits
	event_bus: Arc<EventBus>,

	/// Current sequence number per library (library_id -> sequence)
	/// TODO: Replace with HLC in leaderless architecture
	sync_sequence: Arc<Mutex<std::collections::HashMap<Uuid, u64>>>,
}

impl TransactionManager {
	/// Create a new transaction manager
	pub fn new(event_bus: Arc<EventBus>) -> Self {
		Self {
			event_bus,
			sync_sequence: Arc::new(Mutex::new(std::collections::HashMap::new())),
		}
	}

	/// Get the event bus
	pub fn event_bus(&self) -> &Arc<EventBus> {
		&self.event_bus
	}

	/// Commit device-owned resource (state-based sync)
	///
	/// For locations, entries, volumes, audit logs - data owned by this device.
	/// Uses simple state broadcast (no log needed).
	///
	/// # Example
	/// ```rust,ignore
	/// let location = location::ActiveModel { ... };
	/// tm.commit_device_owned(library, location).await?;
	/// // → Broadcasts state to peers
	/// ```
	pub async fn commit_device_owned<M>(&self, library_id: Uuid, model: M) -> Result<()>
	where
		M: Syncable,
	{
		// TODO: Implement
		// 1. Verify model.is_device_owned() == true
		// 2. Emit event for state broadcast
		// 3. SyncService will pick up event and broadcast

		self.emit_change_event_simple(library_id, M::SYNC_MODEL, model.sync_id());
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
	/// tm.commit_shared(library, tag).await?;
	/// // → Generates HLC, writes to peer_log, broadcasts
	/// ```
	pub async fn commit_shared<M>(&self, library_id: Uuid, model: M) -> Result<()>
	where
		M: Syncable,
	{
		// TODO: Implement
		// 1. Verify model.is_device_owned() == false
		// 2. Generate HLC
		// 3. Write to peer_log
		// 4. Emit event for broadcast

		self.emit_change_event_simple(library_id, M::SYNC_MODEL, model.sync_id());
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
		let event_bus = Arc::new(EventBus::default());

		let _tm = TransactionManager::new(event_bus);
	}
}
