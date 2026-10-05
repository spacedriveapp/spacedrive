//! Durable sync progress markers in sync.db
//!
//! Two facts decide whether a device may adopt state from a peer, and both
//! have to survive restarts. The first is whether this library copy has
//! finished its first backfill: a copy created by joining another device's
//! library has no state of its own and must pull a snapshot, while a library
//! created here, or one that already backfilled, must never adopt one. The
//! second is which peers this device has already caught up with, so a peer
//! with nothing to send is not caught up again every loop iteration.

use chrono::{DateTime, Utc};
use sea_orm::{ConnectionTrait, DbBackend, Statement};
use uuid::Uuid;

use super::watermarks::WatermarkError;

const INITIAL_BACKFILL_PENDING: &str = "initial_backfill_pending";

/// Reads and writes the sync progress markers of one device in a sync.db.
pub struct SyncStateStore {
	device_uuid: Uuid,
}

impl SyncStateStore {
	pub fn new(device_uuid: Uuid) -> Self {
		Self { device_uuid }
	}

	/// Create the marker tables. Safe to call on every open.
	pub async fn init_table<C: ConnectionTrait>(conn: &C) -> Result<(), WatermarkError> {
		conn.execute(Statement::from_string(
			DbBackend::Sqlite,
			r#"
			CREATE TABLE IF NOT EXISTS sync_state (
				key TEXT PRIMARY KEY,
				value TEXT NOT NULL,
				updated_at TEXT NOT NULL
			)
			"#
			.to_string(),
		))
		.await
		.map_err(|e| WatermarkError::QueryError(e.to_string()))?;

		conn.execute(Statement::from_string(
			DbBackend::Sqlite,
			r#"
			CREATE TABLE IF NOT EXISTS peer_catch_up (
				device_uuid TEXT NOT NULL,
				peer_device_uuid TEXT NOT NULL,
				completed_at TEXT NOT NULL,
				PRIMARY KEY (device_uuid, peer_device_uuid)
			)
			"#
			.to_string(),
		))
		.await
		.map_err(|e| WatermarkError::QueryError(e.to_string()))?;

		Ok(())
	}

	/// Mark this library copy as waiting for its first backfill.
	///
	/// Written when a library is created from another device's library, before
	/// the sync service first opens it. The library-level marker has no device
	/// column because the copy belongs to exactly one device.
	pub async fn mark_initial_backfill_pending<C: ConnectionTrait>(
		conn: &C,
	) -> Result<(), WatermarkError> {
		Self::init_table(conn).await?;
		conn.execute(Statement::from_sql_and_values(
			DbBackend::Sqlite,
			r#"
			INSERT INTO sync_state (key, value, updated_at) VALUES (?, '1', ?)
			ON CONFLICT (key) DO UPDATE SET value = '1', updated_at = excluded.updated_at
			"#,
			vec![
				INITIAL_BACKFILL_PENDING.into(),
				Utc::now().to_rfc3339().into(),
			],
		))
		.await
		.map_err(|e| WatermarkError::QueryError(e.to_string()))?;
		Ok(())
	}

	/// Clear the first-backfill marker once a snapshot has been applied.
	pub async fn mark_initial_backfill_complete<C: ConnectionTrait>(
		conn: &C,
	) -> Result<(), WatermarkError> {
		conn.execute(Statement::from_sql_and_values(
			DbBackend::Sqlite,
			"DELETE FROM sync_state WHERE key = ?",
			vec![INITIAL_BACKFILL_PENDING.into()],
		))
		.await
		.map_err(|e| WatermarkError::QueryError(e.to_string()))?;
		Ok(())
	}

	/// Whether this library copy still needs its first backfill.
	///
	/// A library without the marker is treated as complete. That covers every
	/// library created before the marker existed, and erring this way is the
	/// safe side: a copy that wrongly backfills overwrites rows, a copy that
	/// wrongly skips the backfill can be deleted and joined again.
	pub async fn initial_backfill_pending<C: ConnectionTrait>(
		conn: &C,
	) -> Result<bool, WatermarkError> {
		let row = conn
			.query_one(Statement::from_sql_and_values(
				DbBackend::Sqlite,
				"SELECT value FROM sync_state WHERE key = ?",
				vec![INITIAL_BACKFILL_PENDING.into()],
			))
			.await
			.map_err(|e| WatermarkError::QueryError(e.to_string()))?;
		Ok(row.is_some())
	}

	/// Record that a catch-up with `peer` finished, whether or not it carried
	/// any rows.
	pub async fn mark_peer_caught_up<C: ConnectionTrait>(
		&self,
		conn: &C,
		peer: Uuid,
	) -> Result<(), WatermarkError> {
		conn.execute(Statement::from_sql_and_values(
			DbBackend::Sqlite,
			r#"
			INSERT INTO peer_catch_up (device_uuid, peer_device_uuid, completed_at)
			VALUES (?, ?, ?)
			ON CONFLICT (device_uuid, peer_device_uuid)
			DO UPDATE SET completed_at = excluded.completed_at
			"#,
			vec![
				self.device_uuid.to_string().into(),
				peer.to_string().into(),
				Utc::now().to_rfc3339().into(),
			],
		))
		.await
		.map_err(|e| WatermarkError::QueryError(e.to_string()))?;
		Ok(())
	}

	/// When the last catch-up with `peer` finished, if one ever did.
	pub async fn peer_caught_up_at<C: ConnectionTrait>(
		&self,
		conn: &C,
		peer: Uuid,
	) -> Result<Option<DateTime<Utc>>, WatermarkError> {
		let row = conn
			.query_one(Statement::from_sql_and_values(
				DbBackend::Sqlite,
				"SELECT completed_at FROM peer_catch_up WHERE device_uuid = ? AND peer_device_uuid = ?",
				vec![
					self.device_uuid.to_string().into(),
					peer.to_string().into(),
				],
			))
			.await
			.map_err(|e| WatermarkError::QueryError(e.to_string()))?;

		match row {
			Some(row) => {
				let value: String = row
					.try_get("", "completed_at")
					.map_err(|e| WatermarkError::QueryError(e.to_string()))?;
				let at = DateTime::parse_from_rfc3339(&value)
					.map_err(|e| WatermarkError::ParseError(e.to_string()))?
					.with_timezone(&Utc);
				Ok(Some(at))
			}
			None => Ok(None),
		}
	}

	/// Forget a peer's catch-up record so the next loop iteration catches up
	/// with it again. Used when a watermark exchange shows the peer is ahead.
	pub async fn clear_peer_caught_up<C: ConnectionTrait>(
		&self,
		conn: &C,
		peer: Uuid,
	) -> Result<(), WatermarkError> {
		conn.execute(Statement::from_sql_and_values(
			DbBackend::Sqlite,
			"DELETE FROM peer_catch_up WHERE device_uuid = ? AND peer_device_uuid = ?",
			vec![self.device_uuid.to_string().into(), peer.to_string().into()],
		))
		.await
		.map_err(|e| WatermarkError::QueryError(e.to_string()))?;
		Ok(())
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use sea_orm::Database;
	use tempfile::TempDir;

	async fn create_test_db() -> (sea_orm::DatabaseConnection, TempDir) {
		let temp_dir = TempDir::new().unwrap();
		let db_path = temp_dir.path().join("sync.db");
		let conn = Database::connect(format!("sqlite://{}?mode=rwc", db_path.display()))
			.await
			.unwrap();
		SyncStateStore::init_table(&conn).await.unwrap();
		(conn, temp_dir)
	}

	#[tokio::test]
	async fn missing_marker_means_complete() {
		let (conn, _temp) = create_test_db().await;
		assert!(!SyncStateStore::initial_backfill_pending(&conn)
			.await
			.unwrap());

		SyncStateStore::mark_initial_backfill_pending(&conn)
			.await
			.unwrap();
		assert!(SyncStateStore::initial_backfill_pending(&conn)
			.await
			.unwrap());

		SyncStateStore::mark_initial_backfill_complete(&conn)
			.await
			.unwrap();
		assert!(!SyncStateStore::initial_backfill_pending(&conn)
			.await
			.unwrap());
	}

	#[tokio::test]
	async fn peer_catch_up_round_trip() {
		let (conn, _temp) = create_test_db().await;
		let store = SyncStateStore::new(Uuid::new_v4());
		let peer = Uuid::new_v4();

		assert!(store
			.peer_caught_up_at(&conn, peer)
			.await
			.unwrap()
			.is_none());
		store.mark_peer_caught_up(&conn, peer).await.unwrap();
		assert!(store
			.peer_caught_up_at(&conn, peer)
			.await
			.unwrap()
			.is_some());
		store.clear_peer_caught_up(&conn, peer).await.unwrap();
		assert!(store
			.peer_caught_up_at(&conn, peer)
			.await
			.unwrap()
			.is_none());
	}
}
