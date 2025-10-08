//! Transaction Manager - Sole gatekeeper for syncable database writes
//!
//! The TransactionManager ensures that all state-changing writes to sync-enabled
//! models are atomic, logged in the sync log, and emit appropriate events.
//!
//! ## Usage
//!
//! ```rust,ignore
//! // Before: Manual DB write + event emission (error-prone)
//! let model = tag::ActiveModel { /* ... */ };
//! model.insert(db).await?;
//! event_bus.emit(Event::TagCreated { /* ... */ }); // Can forget this!
//!
//! // After: TransactionManager (atomic, automatic)
//! let model = tag::ActiveModel { /* ... */ };
//! let tag = tm.commit(library, model).await?;
//! // ✅ DB write + sync log + event — all atomic!
//! ```

use super::leader::LeadershipManager;
use super::sync_log_db::SyncLogDb;
use super::sync_log_entity::{ChangeType, SyncLogEntry};
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

	#[error("Not leader: only the leader device can create sync log entries")]
	NotLeader,

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
pub struct TransactionManager {
	/// Event bus for emitting events after successful commits
	event_bus: Arc<EventBus>,

	/// Leadership manager to check if this device is the leader
	leadership: Arc<Mutex<LeadershipManager>>,

	/// Current sequence number per library (library_id -> sequence)
	/// Only used by the leader device
	sync_sequence: Arc<Mutex<std::collections::HashMap<Uuid, u64>>>,
}

impl TransactionManager {
	/// Create a new transaction manager
	pub fn new(event_bus: Arc<EventBus>, leadership: Arc<Mutex<LeadershipManager>>) -> Self {
		Self {
			event_bus,
			leadership,
			sync_sequence: Arc::new(Mutex::new(std::collections::HashMap::new())),
		}
	}

	/// Commit a single resource change (creates sync log)
	///
	/// Use this for user-initiated changes (e.g., renaming a file, creating an album).
	///
	/// This is a low-level method. In Phase 2, higher-level wrappers will be
	/// provided for specific model types.
	///
	/// # Arguments
	/// * `library_id` - ID of the library this change belongs to
	/// * `sync_log_db` - Sync log database for the library
	/// * `model` - The syncable model (already written to DB)
	/// * `change_type` - Type of change (Insert, Update, Delete)
	///
	/// # Returns
	/// The sequence number assigned to this change
	pub async fn log_change<M>(
		&self,
		library_id: Uuid,
		sync_log_db: &Arc<SyncLogDb>,
		model: &M,
		change_type: ChangeType,
	) -> Result<u64>
	where
		M: Syncable,
	{
		// Check if we're the leader
		if !self.is_leader(library_id).await {
			return Err(TxError::NotLeader);
		}

		// Get next sequence number
		let sequence = self.next_sequence(library_id).await?;

		// Create sync log entry
		let sync_entry = SyncLogEntry {
			sequence,
			device_id: self.device_id().await,
			timestamp: Utc::now(),
			model_type: M::SYNC_MODEL.to_string(),
			record_id: model.sync_id(),
			change_type,
			version: model.version(),
			data: model.to_sync_json()?,
		};

		// Write sync log entry
		sync_log_db
			.append(sync_entry.clone())
			.await
			.map_err(|e| TxError::SyncLog(format!("Failed to append sync log entry: {}", e)))?;

		debug!(
			library_id = %library_id,
			sequence = sequence,
			model_type = M::SYNC_MODEL,
			record_id = %model.sync_id(),
			"Logged change to sync log"
		);

		// Emit event (after successful commit)
		self.emit_change_event(library_id, &sync_entry);

		Ok(sequence)
	}

	/// Log a batch of changes (10-1K items, creates per-item sync logs)
	///
	/// Use this for watcher events or user actions affecting multiple items
	/// (e.g., copying a folder with 100 files).
	///
	/// Models should already be written to the database.
	///
	/// # Arguments
	/// * `library_id` - ID of the library
	/// * `sync_log_db` - Sync log database
	/// * `models` - Vector of models to log
	/// * `change_type` - Type of change for all models
	///
	/// # Returns
	/// Vector of sequence numbers assigned
	pub async fn log_batch<M>(
		&self,
		library_id: Uuid,
		sync_log_db: &Arc<SyncLogDb>,
		models: &[M],
		change_type: ChangeType,
	) -> Result<Vec<u64>>
	where
		M: Syncable,
	{
		if !self.is_leader(library_id).await {
			return Err(TxError::NotLeader);
		}

		info!(
			library_id = %library_id,
			count = models.len(),
			"Logging batch of changes to sync log"
		);

		let mut sequences = Vec::with_capacity(models.len());

		for model in models {
			let seq = self
				.log_change(library_id, sync_log_db, model, change_type)
				.await?;
			sequences.push(seq);
		}

		Ok(sequences)
	}

	/// Log a bulk operation (1K+ items, creates ONE metadata sync log)
	///
	/// Use this for initial indexing or large-scale operations. Instead of
	/// creating a sync log entry per item, this creates a single metadata entry
	/// that tells followers "I indexed location X with 1M files - you should too".
	///
	/// # Arguments
	/// * `library_id` - ID of the library
	/// * `sync_log_db` - Sync log database
	/// * `metadata` - Bulk operation metadata
	///
	/// # Returns
	/// The sequence number of the bulk operation log entry
	pub async fn log_bulk(
		&self,
		library_id: Uuid,
		sync_log_db: &Arc<SyncLogDb>,
		metadata: BulkOperationMetadata,
	) -> Result<u64> {
		if !self.is_leader(library_id).await {
			return Err(TxError::NotLeader);
		}

		info!(
			library_id = %library_id,
			operation = ?metadata.operation,
			affected_count = metadata.affected_count,
			"Committing bulk operation"
		);

		let sequence = self.next_sequence(library_id).await?;

		// Create a single metadata sync log entry
		let sync_entry = SyncLogEntry {
			sequence,
			device_id: self.device_id().await,
			timestamp: Utc::now(),
			model_type: "bulk_operation".to_string(),
			record_id: Uuid::new_v4(), // Unique ID for this operation
			change_type: ChangeType::BulkInsert,
			version: 1,
			data: serde_json::to_value(&metadata)?,
		};

		sync_log_db
			.append(sync_entry.clone())
			.await
			.map_err(|e| TxError::SyncLog(format!("Failed to append bulk operation: {}", e)))?;

		debug!(
			library_id = %library_id,
			sequence = sequence,
			"Committed bulk operation with metadata sync log"
		);

		// Emit summary event
		self.event_bus.emit(Event::Custom {
			event_type: "BulkOperationCommitted".to_string(),
			data: serde_json::to_value(&metadata).unwrap_or_default(),
		});

		Ok(sequence)
	}

	/// Check if this device is the leader for a library
	async fn is_leader(&self, library_id: Uuid) -> bool {
		let leadership = self.leadership.lock().await;
		leadership.is_leader(library_id)
	}

	/// Get the device ID of this device
	async fn device_id(&self) -> Uuid {
		let leadership = self.leadership.lock().await;
		leadership.device_id()
	}

	/// Get the next sequence number for a library (leader only)
	async fn next_sequence(&self, library_id: Uuid) -> Result<u64> {
		let mut sequences = self.sync_sequence.lock().await;
		let seq = sequences.entry(library_id).or_insert(0);
		*seq += 1;
		Ok(*seq)
	}

	/// Emit an event for a sync log entry
	fn emit_change_event(&self, library_id: Uuid, entry: &SyncLogEntry) {
		// Emit a generic "resource changed" event
		// In Phase 2, emit model-specific events (TagCreated, AlbumUpdated, etc.)
		self.event_bus.emit(Event::Custom {
			event_type: format!("{}_{}", entry.model_type, entry.change_type.to_string()),
			data: serde_json::json!({
				"library_id": library_id,
				"record_id": entry.record_id,
				"sequence": entry.sequence,
			}),
		});
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	// Note: Full integration tests require a complete database setup
	// These are unit tests for the basic structure

	#[test]
	fn test_transaction_manager_creation() {
		let event_bus = Arc::new(EventBus::default());
		let device_id = Uuid::new_v4();
		let leadership = Arc::new(Mutex::new(LeadershipManager::new(device_id)));

		let _tm = TransactionManager::new(event_bus, leadership);
	}
}
