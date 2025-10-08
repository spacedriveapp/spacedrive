//! Sync log database wrapper
//!
//! The sync log lives in a separate database (`sync.db`) per library for better
//! performance, easier maintenance, and cleaner separation of concerns.

use super::sync_log_entity::{
	ActiveModel, ChangeType, Column, Entity, Model, SyncLogEntry, SyncLogModel,
};
use super::sync_log_migration::SyncLogMigrator;
use chrono::{DateTime, Utc};
use sea_orm::{
	ActiveModelTrait, ColumnTrait, ConnectOptions, Database as SeaDatabase, DatabaseConnection,
	DbErr, EntityTrait, PaginatorTrait, QueryFilter, QueryOrder, QuerySelect,
};
use sea_orm_migration::MigratorTrait;
use std::path::Path;
use std::time::Duration;
use thiserror::Error;
use tracing::{debug, info, warn};
use uuid::Uuid;

/// Errors related to sync log database operations
#[derive(Debug, Error)]
pub enum SyncLogError {
	#[error("Database error: {0}")]
	Database(#[from] DbErr),

	#[error("Serialization error: {0}")]
	Serialization(#[from] serde_json::Error),

	#[error("IO error: {0}")]
	Io(#[from] std::io::Error),

	#[error("Not leader: only the leader device can append to sync log")]
	NotLeader,

	#[error("Invalid sequence: expected {expected}, got {actual}")]
	InvalidSequence { expected: u64, actual: u64 },
}

pub type Result<T> = std::result::Result<T, SyncLogError>;

/// Sync log database wrapper
///
/// Manages a separate SQLite database for sync log entries.
/// Each library has its own sync log database located at:
/// `~/.spacedrive/libraries/{library_uuid}/sync.db`
pub struct SyncLogDb {
	library_id: Uuid,
	conn: DatabaseConnection,
}

impl SyncLogDb {
	/// Open or create sync log database for a library
	///
	/// Creates the database if it doesn't exist and runs migrations.
	///
	/// # Arguments
	/// * `library_id` - UUID of the library
	/// * `library_path` - Path to the library directory (e.g., ~/.spacedrive/libraries/{uuid})
	pub async fn open(library_id: Uuid, library_path: &Path) -> Result<Self> {
		info!(
			"Opening sync log database for library {} at {:?}",
			library_id, library_path
		);

		// Ensure library directory exists
		if !library_path.exists() {
			std::fs::create_dir_all(library_path)?;
		}

		let db_path = library_path.join("sync.db");
		let db_url = format!("sqlite://{}?mode=rwc", db_path.display());

		let mut opt = ConnectOptions::new(db_url);
		opt.max_connections(3)
			.min_connections(1)
			.connect_timeout(Duration::from_secs(8))
			.idle_timeout(Duration::from_secs(8))
			.max_lifetime(Duration::from_secs(8))
			.sqlx_logging(false);

		let conn = SeaDatabase::connect(opt).await?;

		// Apply SQLite optimizations for append-only workload
		use sea_orm::{ConnectionTrait, Statement};
		let _ = conn
			.execute(Statement::from_string(
				sea_orm::DatabaseBackend::Sqlite,
				"PRAGMA journal_mode=WAL",
			))
			.await;
		let _ = conn
			.execute(Statement::from_string(
				sea_orm::DatabaseBackend::Sqlite,
				"PRAGMA synchronous=NORMAL",
			))
			.await;
		let _ = conn
			.execute(Statement::from_string(
				sea_orm::DatabaseBackend::Sqlite,
				"PRAGMA temp_store=MEMORY",
			))
			.await;

		// Run migrations
		SyncLogMigrator::up(&conn, None).await?;

		info!(
			"Sync log database opened successfully for library {}",
			library_id
		);

		Ok(Self { library_id, conn })
	}

	/// Append a new entry to the sync log (leader only)
	///
	/// # Arguments
	/// * `entry` - The sync log entry to append
	///
	/// # Returns
	/// The sequence number of the appended entry
	pub async fn append(&self, entry: SyncLogEntry) -> Result<u64> {
		debug!(
			library_id = %self.library_id,
			sequence = entry.sequence,
			model_type = %entry.model_type,
			"Appending entry to sync log"
		);

		let active_model = entry.to_active_model();
		let result = active_model.insert(&self.conn).await?;

		Ok(result.sequence as u64)
	}

	/// Fetch sync log entries since a given sequence number
	///
	/// # Arguments
	/// * `since_sequence` - Fetch entries with sequence > this value
	/// * `limit` - Maximum number of entries to fetch (default: 100)
	///
	/// # Returns
	/// Vector of sync log entries ordered by sequence
	pub async fn fetch_since(
		&self,
		since_sequence: u64,
		limit: Option<usize>,
	) -> Result<Vec<SyncLogEntry>> {
		let limit = limit.unwrap_or(100).min(1000); // Cap at 1000

		debug!(
			library_id = %self.library_id,
			since_sequence = since_sequence,
			limit = limit,
			"Fetching sync log entries"
		);

		let models = Entity::find()
			.filter(Column::Sequence.gt(since_sequence as i64))
			.order_by_asc(Column::Sequence)
			.limit(limit as u64)
			.all(&self.conn)
			.await?;

		let mut entries = Vec::new();
		for model in models {
			entries.push(SyncLogEntry::from_model(model)?);
		}

		Ok(entries)
	}

	/// Fetch a specific range of sync log entries
	///
	/// # Arguments
	/// * `from_sequence` - Start sequence (inclusive)
	/// * `to_sequence` - End sequence (inclusive)
	///
	/// # Returns
	/// Vector of sync log entries in the range
	pub async fn fetch_range(
		&self,
		from_sequence: u64,
		to_sequence: u64,
	) -> Result<Vec<SyncLogEntry>> {
		debug!(
			library_id = %self.library_id,
			from_sequence = from_sequence,
			to_sequence = to_sequence,
			"Fetching sync log entry range"
		);

		let models = Entity::find()
			.filter(Column::Sequence.gte(from_sequence as i64))
			.filter(Column::Sequence.lte(to_sequence as i64))
			.order_by_asc(Column::Sequence)
			.all(&self.conn)
			.await?;

		let mut entries = Vec::new();
		for model in models {
			entries.push(SyncLogEntry::from_model(model)?);
		}

		Ok(entries)
	}

	/// Get the latest sequence number in the sync log
	///
	/// Returns 0 if the sync log is empty.
	pub async fn latest_sequence(&self) -> Result<u64> {
		let result = Entity::find()
			.order_by_desc(Column::Sequence)
			.one(&self.conn)
			.await?;

		Ok(result.map(|m| m.sequence as u64).unwrap_or(0))
	}

	/// Get the total count of entries in the sync log
	pub async fn count(&self) -> Result<u64> {
		let count = Entity::find().count(&self.conn).await?;
		Ok(count)
	}

	/// Vacuum old entries from the sync log
	///
	/// Removes entries older than the specified date. This should be called
	/// periodically (e.g., after successful sync) to keep the database size manageable.
	///
	/// # Arguments
	/// * `before` - Delete entries with timestamp before this date
	///
	/// # Returns
	/// Number of entries deleted
	pub async fn vacuum_old_entries(&self, before: DateTime<Utc>) -> Result<usize> {
		info!(
			library_id = %self.library_id,
			before = %before,
			"Vacuuming old sync log entries"
		);

		let result = Entity::delete_many()
			.filter(Column::Timestamp.lt(before))
			.exec(&self.conn)
			.await?;

		info!(
			library_id = %self.library_id,
			deleted_count = result.rows_affected,
			"Vacuumed old sync log entries"
		);

		Ok(result.rows_affected as usize)
	}

	/// Get entries for a specific record
	///
	/// Useful for debugging or conflict resolution.
	pub async fn get_record_history(
		&self,
		model_type: &str,
		record_id: Uuid,
	) -> Result<Vec<SyncLogEntry>> {
		let models = Entity::find()
			.filter(Column::ModelType.eq(model_type))
			.filter(Column::RecordId.eq(record_id))
			.order_by_asc(Column::Sequence)
			.all(&self.conn)
			.await?;

		let mut entries = Vec::new();
		for model in models {
			entries.push(SyncLogEntry::from_model(model)?);
		}

		Ok(entries)
	}

	/// Get the library ID this sync log belongs to
	pub fn library_id(&self) -> Uuid {
		self.library_id
	}

	/// Get direct access to the database connection
	///
	/// Use with caution - prefer the higher-level methods.
	pub fn connection(&self) -> &DatabaseConnection {
		&self.conn
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use tempfile::tempdir;

	#[tokio::test]
	async fn test_sync_log_db_lifecycle() {
		let temp_dir = tempdir().unwrap();
		let library_id = Uuid::new_v4();

		// Open database
		let sync_db = SyncLogDb::open(library_id, temp_dir.path())
			.await
			.expect("Failed to open sync log db");

		// Verify empty
		let count = sync_db.count().await.unwrap();
		assert_eq!(count, 0);

		let latest = sync_db.latest_sequence().await.unwrap();
		assert_eq!(latest, 0);
	}

	#[tokio::test]
	async fn test_append_and_fetch() {
		let temp_dir = tempdir().unwrap();
		let library_id = Uuid::new_v4();
		let device_id = Uuid::new_v4();

		let sync_db = SyncLogDb::open(library_id, temp_dir.path())
			.await
			.expect("Failed to open sync log db");

		// Create test entry
		let entry = SyncLogEntry {
			sequence: 1,
			device_id,
			timestamp: Utc::now(),
			model_type: "album".to_string(),
			record_id: Uuid::new_v4(),
			change_type: ChangeType::Insert,
			version: 1,
			data: serde_json::json!({"name": "Test Album"}),
		};

		// Append entry
		let seq = sync_db.append(entry.clone()).await.unwrap();
		assert_eq!(seq, 1);

		// Fetch entries
		let entries = sync_db.fetch_since(0, None).await.unwrap();
		assert_eq!(entries.len(), 1);
		assert_eq!(entries[0].sequence, 1);
		assert_eq!(entries[0].model_type, "album");

		// Check latest sequence
		let latest = sync_db.latest_sequence().await.unwrap();
		assert_eq!(latest, 1);
	}
}
