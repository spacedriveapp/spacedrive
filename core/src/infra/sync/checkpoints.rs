//! Backfill checkpoint persistence for crash recovery
//!
//! When a daemon crashes during backfill, checkpoints allow resuming from the last
//! saved state instead of restarting from scratch. Checkpoints are stored in sync.db
//! per peer and per resource type.

use chrono::{DateTime, Utc};
use sea_orm::{ConnectionTrait, DbBackend, Statement};
use uuid::Uuid;

/// Backfill checkpoint store for crash recovery
///
/// Manages checkpoint persistence in sync.db to enable resumable backfill operations.
pub struct BackfillCheckpointStore;

impl BackfillCheckpointStore {
	/// Initialize the checkpoints table in sync.db
	pub async fn init_table<C: ConnectionTrait>(conn: &C) -> Result<(), CheckpointError> {
		// Create backfill checkpoints table
		conn.execute(Statement::from_string(
			DbBackend::Sqlite,
			r#"
			CREATE TABLE IF NOT EXISTS backfill_checkpoints (
				device_uuid TEXT NOT NULL,
				peer_device_uuid TEXT NOT NULL,
				resource_type TEXT NOT NULL,
				resume_token TEXT,
				last_watermark TEXT,
				records_synced INTEGER NOT NULL DEFAULT 0,
				started_at TEXT NOT NULL,
				updated_at TEXT NOT NULL,
				PRIMARY KEY (device_uuid, peer_device_uuid, resource_type)
			)
			"#
			.to_string(),
		))
		.await
		.map_err(|e| CheckpointError::QueryError(e.to_string()))?;

		// Create index for active checkpoints
		conn.execute(Statement::from_string(
			DbBackend::Sqlite,
			r#"
			CREATE INDEX IF NOT EXISTS idx_backfill_checkpoints_peer
			ON backfill_checkpoints(peer_device_uuid)
			"#
			.to_string(),
		))
		.await
		.map_err(|e| CheckpointError::QueryError(e.to_string()))?;

		Ok(())
	}

	/// Save or update a checkpoint
	pub async fn save<C: ConnectionTrait>(
		conn: &C,
		checkpoint: BackfillCheckpoint,
	) -> Result<(), CheckpointError> {
		let now = Utc::now().to_rfc3339();

		conn.execute(Statement::from_sql_and_values(
			DbBackend::Sqlite,
			r#"
			INSERT INTO backfill_checkpoints
			(device_uuid, peer_device_uuid, resource_type, resume_token, last_watermark, records_synced, started_at, updated_at)
			VALUES (?, ?, ?, ?, ?, ?, ?, ?)
			ON CONFLICT (device_uuid, peer_device_uuid, resource_type)
			DO UPDATE SET
				resume_token = excluded.resume_token,
				last_watermark = excluded.last_watermark,
				records_synced = excluded.records_synced,
				updated_at = excluded.updated_at
			"#,
			vec![
				checkpoint.device_uuid.to_string().into(),
				checkpoint.peer_device_uuid.to_string().into(),
				checkpoint.resource_type.into(),
				checkpoint.resume_token.into(),
				checkpoint.last_watermark.map(|ts| ts.to_rfc3339()).into(),
				(checkpoint.records_synced as i64).into(),
				checkpoint.started_at.to_rfc3339().into(),
				now.into(),
			],
		))
		.await
		.map_err(|e| CheckpointError::QueryError(e.to_string()))?;

		Ok(())
	}

	/// Load a checkpoint for a specific peer and resource type
	pub async fn load<C: ConnectionTrait>(
		conn: &C,
		device_uuid: Uuid,
		peer_device_uuid: Uuid,
		resource_type: &str,
	) -> Result<Option<BackfillCheckpoint>, CheckpointError> {
		let result = conn
			.query_one(Statement::from_sql_and_values(
				DbBackend::Sqlite,
				r#"
				SELECT device_uuid, peer_device_uuid, resource_type, resume_token, last_watermark, records_synced, started_at, updated_at
				FROM backfill_checkpoints
				WHERE device_uuid = ? AND peer_device_uuid = ? AND resource_type = ?
				"#,
				vec![
					device_uuid.to_string().into(),
					peer_device_uuid.to_string().into(),
					resource_type.into(),
				],
			))
			.await
			.map_err(|e| CheckpointError::QueryError(e.to_string()))?;

		match result {
			Some(row) => {
				let device_uuid_str: String = row
					.try_get("", "device_uuid")
					.map_err(|e| CheckpointError::QueryError(e.to_string()))?;
				let device_uuid = Uuid::parse_str(&device_uuid_str)
					.map_err(|e| CheckpointError::ParseError(e.to_string()))?;

				let peer_device_uuid_str: String = row
					.try_get("", "peer_device_uuid")
					.map_err(|e| CheckpointError::QueryError(e.to_string()))?;
				let peer_device_uuid = Uuid::parse_str(&peer_device_uuid_str)
					.map_err(|e| CheckpointError::ParseError(e.to_string()))?;

				let resource_type: String = row
					.try_get("", "resource_type")
					.map_err(|e| CheckpointError::QueryError(e.to_string()))?;

				let resume_token: Option<String> = row
					.try_get("", "resume_token")
					.map_err(|e| CheckpointError::QueryError(e.to_string()))?;

				let last_watermark: Option<String> = row
					.try_get("", "last_watermark")
					.map_err(|e| CheckpointError::QueryError(e.to_string()))?;

				let last_watermark = last_watermark
					.map(|s| {
						DateTime::parse_from_rfc3339(&s)
							.map(|dt| dt.with_timezone(&Utc))
							.map_err(|e| CheckpointError::ParseError(e.to_string()))
					})
					.transpose()?;

				let records_synced: i64 = row
					.try_get("", "records_synced")
					.map_err(|e| CheckpointError::QueryError(e.to_string()))?;

				let started_at_str: String = row
					.try_get("", "started_at")
					.map_err(|e| CheckpointError::QueryError(e.to_string()))?;
				let started_at = DateTime::parse_from_rfc3339(&started_at_str)
					.map_err(|e| CheckpointError::ParseError(e.to_string()))?
					.with_timezone(&Utc);

				Ok(Some(BackfillCheckpoint {
					device_uuid,
					peer_device_uuid,
					resource_type,
					resume_token,
					last_watermark,
					records_synced: records_synced as usize,
					started_at,
				}))
			}
			None => Ok(None),
		}
	}

	/// Delete a checkpoint after successful completion
	pub async fn delete<C: ConnectionTrait>(
		conn: &C,
		device_uuid: Uuid,
		peer_device_uuid: Uuid,
		resource_type: &str,
	) -> Result<(), CheckpointError> {
		conn.execute(Statement::from_sql_and_values(
			DbBackend::Sqlite,
			r#"
			DELETE FROM backfill_checkpoints
			WHERE device_uuid = ? AND peer_device_uuid = ? AND resource_type = ?
			"#,
			vec![
				device_uuid.to_string().into(),
				peer_device_uuid.to_string().into(),
				resource_type.into(),
			],
		))
		.await
		.map_err(|e| CheckpointError::QueryError(e.to_string()))?;

		Ok(())
	}

	/// Delete all checkpoints for a peer
	pub async fn delete_peer<C: ConnectionTrait>(
		conn: &C,
		device_uuid: Uuid,
		peer_device_uuid: Uuid,
	) -> Result<usize, CheckpointError> {
		let result = conn
			.execute(Statement::from_sql_and_values(
				DbBackend::Sqlite,
				r#"
				DELETE FROM backfill_checkpoints
				WHERE device_uuid = ? AND peer_device_uuid = ?
				"#,
				vec![
					device_uuid.to_string().into(),
					peer_device_uuid.to_string().into(),
				],
			))
			.await
			.map_err(|e| CheckpointError::QueryError(e.to_string()))?;

		Ok(result.rows_affected() as usize)
	}

	/// List all checkpoints for a device (for diagnostics)
	pub async fn list_all<C: ConnectionTrait>(
		conn: &C,
		device_uuid: Uuid,
	) -> Result<Vec<BackfillCheckpoint>, CheckpointError> {
		let rows = conn
			.query_all(Statement::from_sql_and_values(
				DbBackend::Sqlite,
				r#"
				SELECT device_uuid, peer_device_uuid, resource_type, resume_token, last_watermark, records_synced, started_at, updated_at
				FROM backfill_checkpoints
				WHERE device_uuid = ?
				ORDER BY peer_device_uuid, resource_type
				"#,
				vec![device_uuid.to_string().into()],
			))
			.await
			.map_err(|e| CheckpointError::QueryError(e.to_string()))?;

		let mut checkpoints = Vec::new();
		for row in rows {
			let device_uuid_str: String = row
				.try_get("", "device_uuid")
				.map_err(|e| CheckpointError::QueryError(e.to_string()))?;
			let device_uuid = Uuid::parse_str(&device_uuid_str)
				.map_err(|e| CheckpointError::ParseError(e.to_string()))?;

			let peer_device_uuid_str: String = row
				.try_get("", "peer_device_uuid")
				.map_err(|e| CheckpointError::QueryError(e.to_string()))?;
			let peer_device_uuid = Uuid::parse_str(&peer_device_uuid_str)
				.map_err(|e| CheckpointError::ParseError(e.to_string()))?;

			let resource_type: String = row
				.try_get("", "resource_type")
				.map_err(|e| CheckpointError::QueryError(e.to_string()))?;

			let resume_token: Option<String> = row
				.try_get("", "resume_token")
				.map_err(|e| CheckpointError::QueryError(e.to_string()))?;

			let last_watermark: Option<String> = row
				.try_get("", "last_watermark")
				.map_err(|e| CheckpointError::QueryError(e.to_string()))?;

			let last_watermark = last_watermark
				.map(|s| {
					DateTime::parse_from_rfc3339(&s)
						.map(|dt| dt.with_timezone(&Utc))
						.map_err(|e| CheckpointError::ParseError(e.to_string()))
				})
				.transpose()?;

			let records_synced: i64 = row
				.try_get("", "records_synced")
				.map_err(|e| CheckpointError::QueryError(e.to_string()))?;

			let started_at_str: String = row
				.try_get("", "started_at")
				.map_err(|e| CheckpointError::QueryError(e.to_string()))?;
			let started_at = DateTime::parse_from_rfc3339(&started_at_str)
				.map_err(|e| CheckpointError::ParseError(e.to_string()))?
				.with_timezone(&Utc);

			checkpoints.push(BackfillCheckpoint {
				device_uuid,
				peer_device_uuid,
				resource_type,
				resume_token,
				last_watermark,
				records_synced: records_synced as usize,
				started_at,
			});
		}

		Ok(checkpoints)
	}
}

/// Backfill checkpoint for resumable operations
#[derive(Debug, Clone)]
pub struct BackfillCheckpoint {
	pub device_uuid: Uuid,
	pub peer_device_uuid: Uuid,
	pub resource_type: String,
	pub resume_token: Option<String>,
	pub last_watermark: Option<DateTime<Utc>>,
	pub records_synced: usize,
	pub started_at: DateTime<Utc>,
}

/// Checkpoint errors
#[derive(Debug, thiserror::Error)]
pub enum CheckpointError {
	#[error("Database query error: {0}")]
	QueryError(String),

	#[error("Parse error: {0}")]
	ParseError(String),
}

#[cfg(test)]
mod tests {
	use super::*;
	use sea_orm::Database;
	use tempfile::TempDir;

	async fn create_test_db() -> (sea_orm::DatabaseConnection, TempDir) {
		let temp_dir = TempDir::new().unwrap();
		let db_path = temp_dir.path().join("test_checkpoints.db");
		let database_url = format!("sqlite://{}?mode=rwc", db_path.display());
		let conn = Database::connect(&database_url).await.unwrap();

		// Initialize table
		BackfillCheckpointStore::init_table(&conn).await.unwrap();

		(conn, temp_dir)
	}

	#[tokio::test]
	async fn test_save_and_load_checkpoint() {
		let (conn, _temp) = create_test_db().await;

		let device_uuid = Uuid::new_v4();
		let peer_uuid = Uuid::new_v4();

		let checkpoint = BackfillCheckpoint {
			device_uuid,
			peer_device_uuid: peer_uuid,
			resource_type: "location".to_string(),
			resume_token: Some("token123".to_string()),
			last_watermark: Some(Utc::now()),
			records_synced: 500,
			started_at: Utc::now(),
		};

		// Save checkpoint
		BackfillCheckpointStore::save(&conn, checkpoint.clone())
			.await
			.unwrap();

		// Load checkpoint
		let loaded = BackfillCheckpointStore::load(&conn, device_uuid, peer_uuid, "location")
			.await
			.unwrap();

		assert!(loaded.is_some());
		let loaded = loaded.unwrap();
		assert_eq!(loaded.device_uuid, device_uuid);
		assert_eq!(loaded.peer_device_uuid, peer_uuid);
		assert_eq!(loaded.resource_type, "location");
		assert_eq!(loaded.resume_token, Some("token123".to_string()));
		assert_eq!(loaded.records_synced, 500);
	}

	#[tokio::test]
	async fn test_update_checkpoint() {
		let (conn, _temp) = create_test_db().await;

		let device_uuid = Uuid::new_v4();
		let peer_uuid = Uuid::new_v4();

		// Save initial checkpoint
		let checkpoint1 = BackfillCheckpoint {
			device_uuid,
			peer_device_uuid: peer_uuid,
			resource_type: "entry".to_string(),
			resume_token: Some("token1".to_string()),
			last_watermark: Some(Utc::now()),
			records_synced: 100,
			started_at: Utc::now(),
		};

		BackfillCheckpointStore::save(&conn, checkpoint1).await.unwrap();

		// Update checkpoint (different resume token and records)
		let checkpoint2 = BackfillCheckpoint {
			device_uuid,
			peer_device_uuid: peer_uuid,
			resource_type: "entry".to_string(),
			resume_token: Some("token2".to_string()),
			last_watermark: Some(Utc::now() + chrono::Duration::seconds(10)),
			records_synced: 250,
			started_at: Utc::now(),
		};

		BackfillCheckpointStore::save(&conn, checkpoint2).await.unwrap();

		// Load and verify update
		let loaded = BackfillCheckpointStore::load(&conn, device_uuid, peer_uuid, "entry")
			.await
			.unwrap()
			.unwrap();

		assert_eq!(loaded.resume_token, Some("token2".to_string()));
		assert_eq!(loaded.records_synced, 250);
	}

	#[tokio::test]
	async fn test_delete_checkpoint() {
		let (conn, _temp) = create_test_db().await;

		let device_uuid = Uuid::new_v4();
		let peer_uuid = Uuid::new_v4();

		let checkpoint = BackfillCheckpoint {
			device_uuid,
			peer_device_uuid: peer_uuid,
			resource_type: "volume".to_string(),
			resume_token: None,
			last_watermark: None,
			records_synced: 0,
			started_at: Utc::now(),
		};

		BackfillCheckpointStore::save(&conn, checkpoint).await.unwrap();

		// Verify it exists
		let loaded = BackfillCheckpointStore::load(&conn, device_uuid, peer_uuid, "volume")
			.await
			.unwrap();
		assert!(loaded.is_some());

		// Delete it
		BackfillCheckpointStore::delete(&conn, device_uuid, peer_uuid, "volume")
			.await
			.unwrap();

		// Verify deletion
		let loaded = BackfillCheckpointStore::load(&conn, device_uuid, peer_uuid, "volume")
			.await
			.unwrap();
		assert!(loaded.is_none());
	}

	#[tokio::test]
	async fn test_delete_peer_checkpoints() {
		let (conn, _temp) = create_test_db().await;

		let device_uuid = Uuid::new_v4();
		let peer_uuid = Uuid::new_v4();

		// Save multiple checkpoints for the same peer
		for resource_type in &["location", "entry", "volume"] {
			let checkpoint = BackfillCheckpoint {
				device_uuid,
				peer_device_uuid: peer_uuid,
				resource_type: resource_type.to_string(),
				resume_token: None,
				last_watermark: None,
				records_synced: 0,
				started_at: Utc::now(),
			};
			BackfillCheckpointStore::save(&conn, checkpoint).await.unwrap();
		}

		// Verify all exist
		let all = BackfillCheckpointStore::list_all(&conn, device_uuid)
			.await
			.unwrap();
		assert_eq!(all.len(), 3);

		// Delete all for peer
		let deleted = BackfillCheckpointStore::delete_peer(&conn, device_uuid, peer_uuid)
			.await
			.unwrap();
		assert_eq!(deleted, 3);

		// Verify deletion
		let all = BackfillCheckpointStore::list_all(&conn, device_uuid)
			.await
			.unwrap();
		assert_eq!(all.len(), 0);
	}
}

