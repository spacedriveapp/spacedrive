//! Peer received watermark tracking for shared resource incremental sync
//!
//! Tracks the maximum HLC received from each peer for shared resources (tags, metadata, etc).
//! This enables incremental catch-up by requesting only entries newer than the watermark.

use crate::infra::sync::hlc::HLC;
use sea_orm::{ConnectionTrait, DbBackend, Statement};
use std::collections::HashMap;
use uuid::Uuid;

/// Peer watermark tracking for shared resource incremental sync
///
/// Manages per-peer watermarks in sync.db to track the maximum HLC received
/// from each peer. This prevents re-syncing shared resources that have already
/// been received.
pub struct PeerWatermarkStore {
	device_uuid: Uuid,
}

impl PeerWatermarkStore {
	/// Create a new peer watermark store for a device
	pub fn new(device_uuid: Uuid) -> Self {
		Self { device_uuid }
	}

	/// Initialize the peer_received_watermarks table in sync.db
	pub async fn init_table<C: ConnectionTrait>(conn: &C) -> Result<(), WatermarkError> {
		// Create main table
		conn.execute(Statement::from_string(
			DbBackend::Sqlite,
			r#"
			CREATE TABLE IF NOT EXISTS peer_received_watermarks (
				device_uuid TEXT NOT NULL,
				peer_device_uuid TEXT NOT NULL,
				max_received_hlc TEXT NOT NULL,
				updated_at TEXT NOT NULL,
				PRIMARY KEY (device_uuid, peer_device_uuid)
			)
			"#
			.to_string(),
		))
		.await
		.map_err(|e| WatermarkError::QueryError(e.to_string()))?;

		// Create index for efficient peer queries
		conn.execute(Statement::from_string(
			DbBackend::Sqlite,
			"CREATE INDEX IF NOT EXISTS idx_peer_received_watermarks_peer
			 ON peer_received_watermarks(peer_device_uuid)"
				.to_string(),
		))
		.await
		.map_err(|e| WatermarkError::QueryError(e.to_string()))?;

		Ok(())
	}

	/// Get max HLC received from a specific peer
	pub async fn get<C: ConnectionTrait>(
		&self,
		conn: &C,
		peer_device_uuid: Uuid,
	) -> Result<Option<HLC>, WatermarkError> {
		let row = conn
			.query_one(Statement::from_sql_and_values(
				DbBackend::Sqlite,
				"SELECT max_received_hlc FROM peer_received_watermarks
				 WHERE device_uuid = ? AND peer_device_uuid = ?",
				vec![
					self.device_uuid.to_string().into(),
					peer_device_uuid.to_string().into(),
				],
			))
			.await
			.map_err(|e| WatermarkError::QueryError(e.to_string()))?;

		match row {
			Some(row) => {
				let hlc_str: String = row
					.try_get("", "max_received_hlc")
					.map_err(|e| WatermarkError::QueryError(e.to_string()))?;

				let hlc = hlc_str
					.parse()
					.map_err(|e: crate::infra::sync::hlc::HLCError| {
						WatermarkError::ParseError(e.to_string())
					})?;

				Ok(Some(hlc))
			}
			None => Ok(None),
		}
	}

	/// Update max HLC received from peer (only if newer)
	pub async fn upsert<C: ConnectionTrait>(
		&self,
		conn: &C,
		peer_device_uuid: Uuid,
		received_hlc: HLC,
	) -> Result<(), WatermarkError> {
		// Prevent self-watermarks
		if peer_device_uuid == self.device_uuid {
			tracing::warn!(
				device_uuid = %self.device_uuid,
				peer_device_uuid = %peer_device_uuid,
				"Attempted to track received HLC from self - skipping"
			);
			return Ok(());
		}

		// Check if newer before updating
		let existing = self.get(conn, peer_device_uuid).await?;

		if let Some(existing_hlc) = existing {
			if received_hlc <= existing_hlc {
				return Ok(());
			}
		}

		// Upsert
		conn.execute(Statement::from_sql_and_values(
			DbBackend::Sqlite,
			r#"
			INSERT INTO peer_received_watermarks
			(device_uuid, peer_device_uuid, max_received_hlc, updated_at)
			VALUES (?, ?, ?, ?)
			ON CONFLICT (device_uuid, peer_device_uuid)
			DO UPDATE SET
				max_received_hlc = excluded.max_received_hlc,
				updated_at = excluded.updated_at
			"#,
			vec![
				self.device_uuid.to_string().into(),
				peer_device_uuid.to_string().into(),
				received_hlc.to_string().into(),
				chrono::Utc::now().to_rfc3339().into(),
			],
		))
		.await
		.map_err(|e| WatermarkError::QueryError(e.to_string()))?;

		Ok(())
	}

	/// Get all received watermarks (for diagnostics)
	pub async fn get_all<C: ConnectionTrait>(
		&self,
		conn: &C,
	) -> Result<HashMap<Uuid, HLC>, WatermarkError> {
		let rows = conn
			.query_all(Statement::from_sql_and_values(
				DbBackend::Sqlite,
				"SELECT peer_device_uuid, max_received_hlc FROM peer_received_watermarks
				 WHERE device_uuid = ?",
				vec![self.device_uuid.to_string().into()],
			))
			.await
			.map_err(|e| WatermarkError::QueryError(e.to_string()))?;

		let mut results = HashMap::new();
		for row in rows {
			let peer_str: String = row
				.try_get("", "peer_device_uuid")
				.map_err(|e| WatermarkError::QueryError(e.to_string()))?;

			let hlc_str: String = row
				.try_get("", "max_received_hlc")
				.map_err(|e| WatermarkError::QueryError(e.to_string()))?;

			if let (Ok(peer_uuid), Ok(hlc)) = (Uuid::parse_str(&peer_str), hlc_str.parse()) {
				results.insert(peer_uuid, hlc);
			}
		}

		Ok(results)
	}

	/// Get the maximum HLC received from any peer (for catch-up decisions)
	pub async fn get_max_across_all_peers<C: ConnectionTrait>(
		&self,
		conn: &C,
	) -> Result<Option<HLC>, WatermarkError> {
		let row = conn
			.query_one(Statement::from_sql_and_values(
				DbBackend::Sqlite,
				"SELECT MAX(max_received_hlc) as max_hlc FROM peer_received_watermarks
				 WHERE device_uuid = ?",
				vec![self.device_uuid.to_string().into()],
			))
			.await
			.map_err(|e| WatermarkError::QueryError(e.to_string()))?;

		match row {
			Some(row) => {
				let hlc_str: Option<String> = row.try_get("", "max_hlc").ok();

				match hlc_str {
					Some(s) => {
						let hlc = s.parse().map_err(|e: crate::infra::sync::hlc::HLCError| {
							WatermarkError::ParseError(e.to_string())
						})?;
						Ok(Some(hlc))
					}
					None => Ok(None),
				}
			}
			None => Ok(None),
		}
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
	use crate::infra::sync::time_source::SystemTimeSource;
	use sea_orm::Database;
	use tempfile::TempDir;

	async fn create_test_db() -> (sea_orm::DatabaseConnection, TempDir) {
		let temp_dir = TempDir::new().unwrap();
		let db_path = temp_dir.path().join("test_peer_watermarks.db");
		let database_url = format!("sqlite://{}?mode=rwc", db_path.display());
		let conn = Database::connect(&database_url).await.unwrap();

		PeerWatermarkStore::init_table(&conn).await.unwrap();

		(conn, temp_dir)
	}

	#[tokio::test]
	async fn test_peer_watermark_persistence() {
		let (conn, _temp) = create_test_db().await;

		let device_uuid = Uuid::new_v4();
		let peer = Uuid::new_v4();
		let store = PeerWatermarkStore::new(device_uuid);
		let time = SystemTimeSource;

		let hlc = HLC::now(peer, &time);

		// Store watermark
		store.upsert(&conn, peer, hlc).await.unwrap();

		// Query back
		let retrieved = store.get(&conn, peer).await.unwrap();
		assert_eq!(retrieved, Some(hlc));
	}

	#[tokio::test]
	async fn test_prevent_self_watermark() {
		let (conn, _temp) = create_test_db().await;

		let device_uuid = Uuid::new_v4();
		let store = PeerWatermarkStore::new(device_uuid);
		let time = SystemTimeSource;

		let hlc = HLC::now(device_uuid, &time);

		// Attempt to track self
		let result = store.upsert(&conn, device_uuid, hlc).await;
		assert!(result.is_ok());

		// Verify not stored
		let retrieved = store.get(&conn, device_uuid).await.unwrap();
		assert_eq!(retrieved, None);
	}

	#[tokio::test]
	async fn test_watermark_only_advances() {
		let (conn, _temp) = create_test_db().await;

		let device_uuid = Uuid::new_v4();
		let peer = Uuid::new_v4();
		let store = PeerWatermarkStore::new(device_uuid);

		// Store HLC(1000)
		let hlc1 = HLC {
			timestamp: 1000,
			counter: 0,
			device_id: peer,
		};
		store.upsert(&conn, peer, hlc1).await.unwrap();

		// Try to store older HLC(500)
		let hlc2 = HLC {
			timestamp: 500,
			counter: 0,
			device_id: peer,
		};
		store.upsert(&conn, peer, hlc2).await.unwrap();

		// Should still be HLC(1000)
		let retrieved = store.get(&conn, peer).await.unwrap();
		assert_eq!(retrieved.unwrap().timestamp, 1000);
	}

	#[tokio::test]
	async fn test_get_max_across_all_peers() {
		let (conn, _temp) = create_test_db().await;

		let device_uuid = Uuid::new_v4();
		let store = PeerWatermarkStore::new(device_uuid);

		let peer1 = Uuid::new_v4();
		let peer2 = Uuid::new_v4();
		let peer3 = Uuid::new_v4();

		// Store different HLCs from different peers
		let hlc1 = HLC {
			timestamp: 1000,
			counter: 0,
			device_id: peer1,
		};
		let hlc2 = HLC {
			timestamp: 2000,
			counter: 0,
			device_id: peer2,
		};
		let hlc3 = HLC {
			timestamp: 1500,
			counter: 0,
			device_id: peer3,
		};

		store.upsert(&conn, peer1, hlc1).await.unwrap();
		store.upsert(&conn, peer2, hlc2).await.unwrap();
		store.upsert(&conn, peer3, hlc3).await.unwrap();

		// Get max should return peer2's HLC(2000)
		let max = store.get_max_across_all_peers(&conn).await.unwrap();
		assert!(max.is_some());
		assert_eq!(max.unwrap().timestamp, 2000);
	}

	#[tokio::test]
	async fn test_get_all() {
		let (conn, _temp) = create_test_db().await;

		let device_uuid = Uuid::new_v4();
		let store = PeerWatermarkStore::new(device_uuid);
		let time = SystemTimeSource;

		let peer1 = Uuid::new_v4();
		let peer2 = Uuid::new_v4();

		let hlc1 = HLC::now(peer1, &time);
		let hlc2 = HLC::now(peer2, &time);

		store.upsert(&conn, peer1, hlc1).await.unwrap();
		store.upsert(&conn, peer2, hlc2).await.unwrap();

		let all = store.get_all(&conn).await.unwrap();
		assert_eq!(all.len(), 2);
		assert_eq!(all.get(&peer1), Some(&hlc1));
		assert_eq!(all.get(&peer2), Some(&hlc2));
	}
}
