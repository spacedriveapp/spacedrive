//! Per-resource watermark tracking for incremental sync
//!
//! Instead of global watermarks (one per device), each resource type (location, entry, volume)
//! maintains independent sync progress per peer. This prevents the watermark advancing for one
//! resource type from filtering out other resource types with earlier timestamps.

use chrono::{DateTime, Utc};
use sea_orm::{ConnectionTrait, DbBackend, Statement};
use uuid::Uuid;

/// Resource watermark tracking for incremental sync
///
/// Manages per-resource watermarks in sync.db to ensure each resource type
/// can sync independently without cross-contamination.
pub struct ResourceWatermarkStore {
	device_uuid: Uuid,
}

impl ResourceWatermarkStore {
	/// Create a new watermark store for a device
	pub fn new(device_uuid: Uuid) -> Self {
		Self { device_uuid }
	}

	/// Initialize the watermarks table in sync.db
	pub async fn init_table<C: ConnectionTrait>(conn: &C) -> Result<(), WatermarkError> {
		// Create main watermarks table
		conn.execute(Statement::from_string(
			DbBackend::Sqlite,
			r#"
			CREATE TABLE IF NOT EXISTS device_resource_watermarks (
				device_uuid TEXT NOT NULL,
				peer_device_uuid TEXT NOT NULL,
				resource_type TEXT NOT NULL,
				last_watermark TEXT NOT NULL,
				updated_at TEXT NOT NULL,
				PRIMARY KEY (device_uuid, peer_device_uuid, resource_type)
			)
			"#
			.to_string(),
		))
		.await
		.map_err(|e| WatermarkError::QueryError(e.to_string()))?;

		// Create indexes for efficient queries
		conn.execute(Statement::from_string(
			DbBackend::Sqlite,
			r#"
			CREATE INDEX IF NOT EXISTS idx_resource_watermarks_peer
			ON device_resource_watermarks(peer_device_uuid, resource_type)
			"#
			.to_string(),
		))
		.await
		.map_err(|e| WatermarkError::QueryError(e.to_string()))?;

		conn.execute(Statement::from_string(
			DbBackend::Sqlite,
			r#"
			CREATE INDEX IF NOT EXISTS idx_resource_watermarks_resource
			ON device_resource_watermarks(resource_type)
			"#
			.to_string(),
		))
		.await
		.map_err(|e| WatermarkError::QueryError(e.to_string()))?;

		Ok(())
	}

	/// Get watermark for a specific resource type from a peer
	pub async fn get<C: ConnectionTrait>(
		&self,
		conn: &C,
		peer_device_uuid: Uuid,
		resource_type: &str,
	) -> Result<Option<DateTime<Utc>>, WatermarkError> {
		let result = conn
			.query_one(Statement::from_sql_and_values(
				DbBackend::Sqlite,
				r#"
				SELECT last_watermark FROM device_resource_watermarks
				WHERE device_uuid = ? AND peer_device_uuid = ? AND resource_type = ?
				"#,
				vec![
					self.device_uuid.to_string().into(),
					peer_device_uuid.to_string().into(),
					resource_type.into(),
				],
			))
			.await
			.map_err(|e| WatermarkError::QueryError(e.to_string()))?;

		match result {
			Some(row) => {
				let watermark_str: String = row
					.try_get("", "last_watermark")
					.map_err(|e| WatermarkError::QueryError(e.to_string()))?;

				let dt = DateTime::parse_from_rfc3339(&watermark_str)
					.map_err(|e| WatermarkError::ParseError(e.to_string()))?
					.with_timezone(&Utc);

				Ok(Some(dt))
			}
			None => Ok(None),
		}
	}

	/// Upsert a resource watermark (only if newer than existing)
	pub async fn upsert<C: ConnectionTrait>(
		&self,
		conn: &C,
		peer_device_uuid: Uuid,
		resource_type: &str,
		watermark: DateTime<Utc>,
	) -> Result<(), WatermarkError> {
		// Check if newer before updating
		let existing = self.get(conn, peer_device_uuid, resource_type).await?;

		if let Some(existing_ts) = existing {
			if watermark <= existing_ts {
				// Don't update if not newer
				return Ok(());
			}
		}

		// Upsert (insert or replace)
		conn.execute(Statement::from_sql_and_values(
			DbBackend::Sqlite,
			r#"
			INSERT INTO device_resource_watermarks
			(device_uuid, peer_device_uuid, resource_type, last_watermark, updated_at)
			VALUES (?, ?, ?, ?, ?)
			ON CONFLICT (device_uuid, peer_device_uuid, resource_type)
			DO UPDATE SET
				last_watermark = excluded.last_watermark,
				updated_at = excluded.updated_at
			"#,
			vec![
				self.device_uuid.to_string().into(),
				peer_device_uuid.to_string().into(),
				resource_type.into(),
				watermark.to_rfc3339().into(),
				Utc::now().to_rfc3339().into(),
			],
		))
		.await
		.map_err(|e| WatermarkError::QueryError(e.to_string()))?;

		Ok(())
	}

	/// Get all watermarks for a peer (for diagnostics/debugging)
	pub async fn get_all_for_peer<C: ConnectionTrait>(
		&self,
		conn: &C,
		peer_device_uuid: Uuid,
	) -> Result<Vec<(String, DateTime<Utc>)>, WatermarkError> {
		let rows = conn
			.query_all(Statement::from_sql_and_values(
				DbBackend::Sqlite,
				r#"
				SELECT resource_type, last_watermark FROM device_resource_watermarks
				WHERE device_uuid = ? AND peer_device_uuid = ?
				ORDER BY resource_type
				"#,
				vec![
					self.device_uuid.to_string().into(),
					peer_device_uuid.to_string().into(),
				],
			))
			.await
			.map_err(|e| WatermarkError::QueryError(e.to_string()))?;

		let mut results = Vec::new();
		for row in rows {
			let resource_type: String = row
				.try_get("", "resource_type")
				.map_err(|e| WatermarkError::QueryError(e.to_string()))?;

			let watermark_str: String = row
				.try_get("", "last_watermark")
				.map_err(|e| WatermarkError::QueryError(e.to_string()))?;

			let dt = DateTime::parse_from_rfc3339(&watermark_str)
				.map_err(|e| WatermarkError::ParseError(e.to_string()))?
				.with_timezone(&Utc);

			results.push((resource_type, dt));
		}

		Ok(results)
	}

	/// Get maximum watermark across all resource types for this device
	///
	/// Returns the most recent watermark across all resource types.
	/// This is useful for determining overall sync progress.
	pub async fn get_max_watermark<C: ConnectionTrait>(
		&self,
		conn: &C,
	) -> Result<Option<DateTime<Utc>>, WatermarkError> {
		let result = conn
			.query_one(Statement::from_sql_and_values(
				DbBackend::Sqlite,
				r#"
				SELECT MAX(last_watermark) as max_watermark FROM device_resource_watermarks
				WHERE device_uuid = ?
				"#,
				vec![self.device_uuid.to_string().into()],
			))
			.await
			.map_err(|e| WatermarkError::QueryError(e.to_string()))?;

		match result {
			Some(row) => {
				let watermark_str: Option<String> = row
					.try_get("", "max_watermark")
					.ok();

				if let Some(wm_str) = watermark_str {
					let dt = DateTime::parse_from_rfc3339(&wm_str)
						.map_err(|e| WatermarkError::ParseError(e.to_string()))?
						.with_timezone(&Utc);

					Ok(Some(dt))
				} else {
					Ok(None)
				}
			}
			None => Ok(None),
		}
	}

	/// Delete all watermarks for a peer (cleanup on peer removal)
	pub async fn delete_peer<C: ConnectionTrait>(
		&self,
		conn: &C,
		peer_device_uuid: Uuid,
	) -> Result<usize, WatermarkError> {
		let result = conn
			.execute(Statement::from_sql_and_values(
				DbBackend::Sqlite,
				r#"
				DELETE FROM device_resource_watermarks
				WHERE device_uuid = ? AND peer_device_uuid = ?
				"#,
				vec![
					self.device_uuid.to_string().into(),
					peer_device_uuid.to_string().into(),
				],
			))
			.await
			.map_err(|e| WatermarkError::QueryError(e.to_string()))?;

		Ok(result.rows_affected() as usize)
	}

	/// Get our latest watermark for each resource type (aggregated across all peers)
	///
	/// Returns a HashMap mapping resource_type -> max(last_watermark) across all peers.
	/// This represents what we've successfully received from our peers.
	pub async fn get_our_resource_watermarks<C: ConnectionTrait>(
		&self,
		conn: &C,
	) -> Result<std::collections::HashMap<String, DateTime<Utc>>, WatermarkError> {
		let rows = conn
			.query_all(Statement::from_sql_and_values(
				DbBackend::Sqlite,
				r#"
				SELECT resource_type, MAX(last_watermark) as max_watermark
				FROM device_resource_watermarks
				WHERE device_uuid = ?
				GROUP BY resource_type
				ORDER BY resource_type
				"#,
				vec![self.device_uuid.to_string().into()],
			))
			.await
			.map_err(|e| WatermarkError::QueryError(e.to_string()))?;

		let mut results = std::collections::HashMap::new();
		for row in rows {
			let resource_type: String = row
				.try_get("", "resource_type")
				.map_err(|e| WatermarkError::QueryError(e.to_string()))?;

			let watermark_str: String = row
				.try_get("", "max_watermark")
				.map_err(|e| WatermarkError::QueryError(e.to_string()))?;

			let dt = DateTime::parse_from_rfc3339(&watermark_str)
				.map_err(|e| WatermarkError::ParseError(e.to_string()))?
				.with_timezone(&Utc);

			results.insert(resource_type, dt);
		}

		Ok(results)
	}
}

/// Watermark errors
#[derive(Debug, thiserror::Error)]
pub enum WatermarkError {
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
		let db_path = temp_dir.path().join("test_watermarks.db");
		let database_url = format!("sqlite://{}?mode=rwc", db_path.display());
		let conn = Database::connect(&database_url).await.unwrap();

		// Initialize table
		ResourceWatermarkStore::init_table(&conn).await.unwrap();

		(conn, temp_dir)
	}

	#[tokio::test]
	async fn test_upsert_and_get() {
		let (conn, _temp) = create_test_db().await;

		let device_uuid = Uuid::new_v4();
		let peer_uuid = Uuid::new_v4();
		let store = ResourceWatermarkStore::new(device_uuid);

		// Insert initial watermark
		let timestamp1 = Utc::now();
		store
			.upsert(&conn, peer_uuid, "location", timestamp1)
			.await
			.unwrap();

		// Verify retrieval
		let retrieved = store.get(&conn, peer_uuid, "location").await.unwrap();
		assert!(retrieved.is_some());
		assert_eq!(
			retrieved.unwrap().timestamp(),
			timestamp1.timestamp()
		);

		// Update with newer timestamp
		let timestamp2 = timestamp1 + chrono::Duration::seconds(10);
		store
			.upsert(&conn, peer_uuid, "location", timestamp2)
			.await
			.unwrap();

		// Verify update
		let retrieved = store.get(&conn, peer_uuid, "location").await.unwrap();
		assert_eq!(
			retrieved.unwrap().timestamp(),
			timestamp2.timestamp()
		);

		// Attempt update with older timestamp (should be ignored)
		let timestamp0 = timestamp1 - chrono::Duration::seconds(10);
		store
			.upsert(&conn, peer_uuid, "location", timestamp0)
			.await
			.unwrap();

		// Verify still has timestamp2 (newer)
		let retrieved = store.get(&conn, peer_uuid, "location").await.unwrap();
		assert_eq!(
			retrieved.unwrap().timestamp(),
			timestamp2.timestamp()
		);
	}

	#[tokio::test]
	async fn test_independent_resource_types() {
		let (conn, _temp) = create_test_db().await;

		let device_uuid = Uuid::new_v4();
		let peer_uuid = Uuid::new_v4();
		let store = ResourceWatermarkStore::new(device_uuid);

		let base_time = Utc::now();

		// Store different watermarks for different resource types
		store
			.upsert(&conn, peer_uuid, "location", base_time)
			.await
			.unwrap();

		store
			.upsert(
				&conn,
				peer_uuid,
				"entry",
				base_time + chrono::Duration::seconds(100),
			)
			.await
			.unwrap();

		store
			.upsert(
				&conn,
				peer_uuid,
				"volume",
				base_time + chrono::Duration::seconds(200),
			)
			.await
			.unwrap();

		// Verify each is stored independently
		let loc_wm = store.get(&conn, peer_uuid, "location").await.unwrap();
		let entry_wm = store.get(&conn, peer_uuid, "entry").await.unwrap();
		let vol_wm = store.get(&conn, peer_uuid, "volume").await.unwrap();

		assert!(loc_wm.is_some());
		assert!(entry_wm.is_some());
		assert!(vol_wm.is_some());

		// Verify they're different
		assert_ne!(loc_wm.unwrap(), entry_wm.unwrap());
		assert_ne!(entry_wm.unwrap(), vol_wm.unwrap());

		// Get all for peer
		let all = store.get_all_for_peer(&conn, peer_uuid).await.unwrap();
		assert_eq!(all.len(), 3);
	}

	#[tokio::test]
	async fn test_delete_peer() {
		let (conn, _temp) = create_test_db().await;

		let device_uuid = Uuid::new_v4();
		let peer_uuid = Uuid::new_v4();
		let store = ResourceWatermarkStore::new(device_uuid);

		let base_time = Utc::now();

		// Store multiple resource types
		store
			.upsert(&conn, peer_uuid, "location", base_time)
			.await
			.unwrap();
		store
			.upsert(&conn, peer_uuid, "entry", base_time)
			.await
			.unwrap();

		// Verify they exist
		let all = store.get_all_for_peer(&conn, peer_uuid).await.unwrap();
		assert_eq!(all.len(), 2);

		// Delete all for peer
		let deleted = store.delete_peer(&conn, peer_uuid).await.unwrap();
		assert_eq!(deleted, 2);

		// Verify deletion
		let all = store.get_all_for_peer(&conn, peer_uuid).await.unwrap();
		assert_eq!(all.len(), 0);
	}
}

