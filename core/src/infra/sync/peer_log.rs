//! Per-peer sync log for shared resource changes
//!
//! Each device maintains a small, prunable log of its own changes to shared resources.
//! This log is ordered by HLC and pruned once all peers have acknowledged receiving changes.

use super::hlc::HLC;
use sea_orm::{
	entity::prelude::*, ConnectionTrait, Database, DatabaseConnection, DbBackend, Statement,
};
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::Arc;
use uuid::Uuid;

/// Per-peer sync log
///
/// Manages a separate `sync.db` file per library that contains only
/// this device's changes to shared resources.
pub struct PeerLog {
	library_id: Uuid,
	device_id: Uuid,
	conn: DatabaseConnection,
}

impl PeerLog {
	/// Open or create peer sync log for a library
	pub async fn open(
		library_id: Uuid,
		device_id: Uuid,
		library_path: &Path,
	) -> Result<Self, PeerLogError> {
		let sync_db_path = library_path.join("sync.db");

		let database_url = format!("sqlite://{}?mode=rwc", sync_db_path.display());
		let conn = Database::connect(&database_url)
			.await
			.map_err(|e| PeerLogError::ConnectionError(e.to_string()))?;

		// Create tables if they don't exist
		Self::create_tables(&conn).await?;

		Ok(Self {
			library_id,
			device_id,
			conn,
		})
	}

	/// Create sync.db tables
	async fn create_tables(conn: &DatabaseConnection) -> Result<(), PeerLogError> {
		// shared_changes table
		conn.execute(Statement::from_string(
			DbBackend::Sqlite,
			r#"
			CREATE TABLE IF NOT EXISTS shared_changes (
				hlc TEXT PRIMARY KEY,
				model_type TEXT NOT NULL,
				record_uuid TEXT NOT NULL,
				change_type TEXT NOT NULL,
				data TEXT NOT NULL,
				created_at TEXT NOT NULL
			)
			"#
			.to_string(),
		))
		.await
		.map_err(|e| PeerLogError::QueryError(e.to_string()))?;

		// Indexes for efficient queries
		conn.execute(Statement::from_string(
			DbBackend::Sqlite,
			"CREATE INDEX IF NOT EXISTS idx_shared_changes_hlc ON shared_changes(hlc)".to_string(),
		))
		.await
		.map_err(|e| PeerLogError::QueryError(e.to_string()))?;

		conn.execute(Statement::from_string(
			DbBackend::Sqlite,
			"CREATE INDEX IF NOT EXISTS idx_shared_changes_model ON shared_changes(model_type)"
				.to_string(),
		))
		.await
		.map_err(|e| PeerLogError::QueryError(e.to_string()))?;

		// peer_acks table
		conn.execute(Statement::from_string(
			DbBackend::Sqlite,
			r#"
			CREATE TABLE IF NOT EXISTS peer_acks (
				peer_device_id TEXT PRIMARY KEY,
				last_acked_hlc TEXT NOT NULL,
				acked_at TEXT NOT NULL
			)
			"#
			.to_string(),
		))
		.await
		.map_err(|e| PeerLogError::QueryError(e.to_string()))?;

		// device_resource_watermarks table (per-resource sync tracking)
		super::watermarks::ResourceWatermarkStore::init_table(conn)
			.await
			.map_err(|e| PeerLogError::QueryError(e.to_string()))?;

		// peer_received_watermarks table (shared resource incremental sync)
		super::peer_watermarks::PeerWatermarkStore::init_table(conn)
			.await
			.map_err(|e| PeerLogError::QueryError(e.to_string()))?;

		// backfill_checkpoints table (resumable backfill)
		super::checkpoints::BackfillCheckpointStore::init_table(conn)
			.await
			.map_err(|e| PeerLogError::QueryError(e.to_string()))?;

		Ok(())
	}

	/// Append a shared change entry to the log
	pub async fn append(&self, entry: SharedChangeEntry) -> Result<(), PeerLogError> {
		let hlc_str = entry.hlc.to_string();
		let change_type_str = entry.change_type.to_string();
		let data_json = serde_json::to_string(&entry.data)
			.map_err(|e| PeerLogError::SerializationError(e.to_string()))?;
		let created_at = chrono::Utc::now().to_rfc3339();

		self.conn
			.execute(Statement::from_sql_and_values(
				DbBackend::Sqlite,
				r#"
				INSERT INTO shared_changes (hlc, model_type, record_uuid, change_type, data, created_at)
				VALUES (?, ?, ?, ?, ?, ?)
				"#,
				vec![
					hlc_str.into(),
					entry.model_type.into(),
					entry.record_uuid.to_string().into(),
					change_type_str.into(),
					data_json.into(),
					created_at.into(),
				],
			))
			.await
			.map_err(|e| PeerLogError::QueryError(e.to_string()))?;

		Ok(())
	}

	/// Get all changes since a given HLC
	pub async fn get_since(
		&self,
		since: Option<HLC>,
	) -> Result<Vec<SharedChangeEntry>, PeerLogError> {
		let query = match since {
			Some(hlc) => {
				let hlc_str = hlc.to_string();
				Statement::from_sql_and_values(
					DbBackend::Sqlite,
					"SELECT hlc, model_type, record_uuid, change_type, data FROM shared_changes WHERE hlc > ? ORDER BY hlc ASC",
					vec![hlc_str.into()],
				)
			}
			None => Statement::from_string(
				DbBackend::Sqlite,
				"SELECT hlc, model_type, record_uuid, change_type, data FROM shared_changes ORDER BY hlc ASC".to_string(),
			),
		};

		let rows = self
			.conn
			.query_all(query)
			.await
			.map_err(|e| PeerLogError::QueryError(e.to_string()))?;

		let mut entries = Vec::new();
		for row in rows {
			let hlc_str: String = row
				.try_get("", "hlc")
				.map_err(|e| PeerLogError::QueryError(e.to_string()))?;
			let hlc =
				HLC::from_string(&hlc_str).map_err(|e| PeerLogError::ParseError(e.to_string()))?;

			let model_type: String = row
				.try_get("", "model_type")
				.map_err(|e| PeerLogError::QueryError(e.to_string()))?;

			let record_uuid_str: String = row
				.try_get("", "record_uuid")
				.map_err(|e| PeerLogError::QueryError(e.to_string()))?;
			let record_uuid = Uuid::parse_str(&record_uuid_str)
				.map_err(|e| PeerLogError::ParseError(e.to_string()))?;

			let change_type_str: String = row
				.try_get("", "change_type")
				.map_err(|e| PeerLogError::QueryError(e.to_string()))?;
			let change_type = ChangeType::from_string(&change_type_str)?;

			let data_json: String = row
				.try_get("", "data")
				.map_err(|e| PeerLogError::QueryError(e.to_string()))?;
			let data: serde_json::Value = serde_json::from_str(&data_json)
				.map_err(|e| PeerLogError::SerializationError(e.to_string()))?;

			entries.push(SharedChangeEntry {
				hlc,
				model_type,
				record_uuid,
				change_type,
				data,
			});
		}

		Ok(entries)
	}

	/// Get the maximum HLC from the peer log
	///
	/// Returns the most recent HLC across all shared changes.
	/// This represents the current shared watermark for this device.
	pub async fn get_max_hlc(&self) -> Result<Option<HLC>, PeerLogError> {
		let result = self
			.conn
			.query_one(Statement::from_string(
				DbBackend::Sqlite,
				"SELECT MAX(hlc) as max_hlc FROM shared_changes".to_string(),
			))
			.await
			.map_err(|e| PeerLogError::QueryError(e.to_string()))?;

		match result {
			Some(row) => {
				let hlc_str: Option<String> = row.try_get("", "max_hlc").ok();

				if let Some(hlc_s) = hlc_str {
					let hlc = HLC::from_string(&hlc_s)
						.map_err(|e| PeerLogError::ParseError(e.to_string()))?;
					Ok(Some(hlc))
				} else {
					Ok(None)
				}
			}
			None => Ok(None),
		}
	}

	/// Record peer acknowledgment of changes up to an HLC
	pub async fn record_ack(&self, peer_id: Uuid, up_to_hlc: HLC) -> Result<(), PeerLogError> {
		let hlc_str = up_to_hlc.to_string();
		let acked_at = chrono::Utc::now().to_rfc3339();

		self.conn
			.execute(Statement::from_sql_and_values(
				DbBackend::Sqlite,
				r#"
				INSERT OR REPLACE INTO peer_acks (peer_device_id, last_acked_hlc, acked_at)
				VALUES (?, ?, ?)
				"#,
				vec![peer_id.to_string().into(), hlc_str.into(), acked_at.into()],
			))
			.await
			.map_err(|e| PeerLogError::QueryError(e.to_string()))?;

		Ok(())
	}

	/// Get the minimum HLC that all peers have acknowledged
	///
	/// Excludes self-ACKs (where peer_device_id == our device_id) from calculation.
	/// Self-ACKs should never exist, but filtering them defensively prevents stale
	/// self-ACKs from blocking pruning.
	async fn get_min_acked_hlc(&self) -> Result<Option<HLC>, PeerLogError> {
		let result = self
			.conn
			.query_one(Statement::from_sql_and_values(
				DbBackend::Sqlite,
				"SELECT MIN(last_acked_hlc) as min_hlc FROM peer_acks WHERE peer_device_id != ?",
				vec![self.device_id.to_string().into()],
			))
			.await
			.map_err(|e| PeerLogError::QueryError(e.to_string()))?;

		match result {
			Some(row) => {
				let hlc_str: Option<String> = row
					.try_get("", "min_hlc")
					.map_err(|e| PeerLogError::QueryError(e.to_string()))?;

				match hlc_str {
					Some(s) => Ok(Some(
						HLC::from_string(&s)
							.map_err(|e| PeerLogError::ParseError(e.to_string()))?,
					)),
					None => Ok(None),
				}
			}
			None => Ok(None),
		}
	}

	/// Prune entries that all peers have acknowledged
	pub async fn prune_acked(&self) -> Result<usize, PeerLogError> {
		let min_hlc = self.get_min_acked_hlc().await?;

		match min_hlc {
			Some(hlc) => {
				let hlc_str = hlc.to_string();
				let result = self
					.conn
					.execute(Statement::from_sql_and_values(
						DbBackend::Sqlite,
						"DELETE FROM shared_changes WHERE hlc <= ?",
						vec![hlc_str.into()],
					))
					.await
					.map_err(|e| PeerLogError::QueryError(e.to_string()))?;

				Ok(result.rows_affected() as usize)
			}
			None => Ok(0),
		}
	}

	/// Count total entries in log
	pub async fn count(&self) -> Result<usize, PeerLogError> {
		let result = self
			.conn
			.query_one(Statement::from_string(
				DbBackend::Sqlite,
				"SELECT COUNT(*) as count FROM shared_changes".to_string(),
			))
			.await
			.map_err(|e| PeerLogError::QueryError(e.to_string()))?;

		match result {
			Some(row) => {
				let count: i64 = row
					.try_get("", "count")
					.map_err(|e| PeerLogError::QueryError(e.to_string()))?;
				Ok(count as usize)
			}
			None => Ok(0),
		}
	}

	/// Get the latest HLC for a specific record UUID
	///
	/// Used for conflict resolution - returns the most recent HLC that was logged
	/// for this record, so we can compare incoming changes against what we've already seen.
	pub async fn get_latest_hlc_for_record(
		&self,
		record_uuid: Uuid,
	) -> Result<Option<HLC>, PeerLogError> {
		let result = self
			.conn
			.query_one(Statement::from_sql_and_values(
				DbBackend::Sqlite,
				"SELECT hlc FROM shared_changes WHERE record_uuid = ? ORDER BY hlc DESC LIMIT 1",
				vec![record_uuid.to_string().into()],
			))
			.await
			.map_err(|e| PeerLogError::QueryError(e.to_string()))?;

		match result {
			Some(row) => {
				let hlc_str: String = row
					.try_get("", "hlc")
					.map_err(|e| PeerLogError::QueryError(e.to_string()))?;
				let hlc = HLC::from_string(&hlc_str)
					.map_err(|e| PeerLogError::ParseError(e.to_string()))?;
				Ok(Some(hlc))
			}
			None => Ok(None),
		}
	}

	/// Get database connection (for advanced queries)
	pub fn conn(&self) -> &DatabaseConnection {
		&self.conn
	}
}

/// Entry in the shared changes log
#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub struct SharedChangeEntry {
	pub hlc: HLC,
	pub model_type: String,
	pub record_uuid: Uuid,
	pub change_type: ChangeType,
	pub data: serde_json::Value,
}

/// Type of database change
#[derive(Debug, Clone, Copy, Serialize, Deserialize, Eq, PartialEq)]
pub enum ChangeType {
	Insert,
	Update,
	Delete,
}

impl ChangeType {
	pub fn to_string(&self) -> String {
		match self {
			ChangeType::Insert => "insert".to_string(),
			ChangeType::Update => "update".to_string(),
			ChangeType::Delete => "delete".to_string(),
		}
	}

	pub fn from_string(s: &str) -> Result<Self, PeerLogError> {
		match s {
			"insert" => Ok(ChangeType::Insert),
			"update" => Ok(ChangeType::Update),
			"delete" => Ok(ChangeType::Delete),
			_ => Err(PeerLogError::ParseError(format!(
				"Invalid change type: {}",
				s
			))),
		}
	}
}

/// PeerLog errors
#[derive(Debug, thiserror::Error)]
pub enum PeerLogError {
	#[error("Database connection error: {0}")]
	ConnectionError(String),

	#[error("Database query error: {0}")]
	QueryError(String),

	#[error("Serialization error: {0}")]
	SerializationError(String),

	#[error("Parse error: {0}")]
	ParseError(String),
}

#[cfg(test)]
mod tests {
	use super::*;
	use tempfile::TempDir;

	async fn create_test_peer_log() -> (PeerLog, TempDir) {
		let temp_dir = TempDir::new().unwrap();
		let library_id = Uuid::new_v4();
		let device_id = Uuid::new_v4();

		let peer_log = PeerLog::open(library_id, device_id, temp_dir.path())
			.await
			.unwrap();

		(peer_log, temp_dir)
	}

	#[tokio::test]
	async fn test_append_and_retrieve() {
		let (peer_log, _temp) = create_test_peer_log().await;

		let entry = SharedChangeEntry {
			hlc: HLC::now(peer_log.device_id),
			model_type: "tag".to_string(),
			record_uuid: Uuid::new_v4(),
			change_type: ChangeType::Insert,
			data: serde_json::json!({"name": "test"}),
		};

		peer_log.append(entry.clone()).await.unwrap();

		let entries = peer_log.get_since(None).await.unwrap();
		assert_eq!(entries.len(), 1);
		assert_eq!(entries[0].model_type, "tag");
	}

	#[tokio::test]
	async fn test_ack_and_prune() {
		let (peer_log, _temp) = create_test_peer_log().await;

		// Add 3 entries
		for i in 0..3 {
			let entry = SharedChangeEntry {
				hlc: HLC::generate(None, peer_log.device_id),
				model_type: "tag".to_string(),
				record_uuid: Uuid::new_v4(),
				change_type: ChangeType::Insert,
				data: serde_json::json!({"name": format!("tag{}", i)}),
			};
			peer_log.append(entry).await.unwrap();
			tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
		}

		let entries = peer_log.get_since(None).await.unwrap();
		assert_eq!(entries.len(), 3);

		// Peer A acks first 2
		let peer_a = Uuid::new_v4();
		peer_log.record_ack(peer_a, entries[1].hlc).await.unwrap();

		// Peer B acks all 3
		let peer_b = Uuid::new_v4();
		peer_log.record_ack(peer_b, entries[2].hlc).await.unwrap();

		// Prune - should remove first 2 (min ack)
		let pruned = peer_log.prune_acked().await.unwrap();
		assert_eq!(pruned, 2);

		let remaining = peer_log.get_since(None).await.unwrap();
		assert_eq!(remaining.len(), 1);
	}
}
