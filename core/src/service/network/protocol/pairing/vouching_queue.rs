use std::path::Path;

use chrono::{DateTime, Utc};
use sea_orm::{ConnectionTrait, Database, DatabaseConnection, DbBackend, Statement};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::service::network::{
	device::{DeviceInfo, SessionKeys},
	NetworkingError, Result,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum VouchQueueStatus {
	Queued,
	Waiting,
}

impl VouchQueueStatus {
	fn as_str(&self) -> &'static str {
		match self {
			Self::Queued => "queued",
			Self::Waiting => "waiting",
		}
	}

	fn from_str(value: &str) -> Self {
		match value {
			"waiting" => Self::Waiting,
			_ => Self::Queued,
		}
	}
}

#[derive(Debug, Clone)]
pub struct VouchingQueueEntry {
	pub session_id: Uuid,
	pub target_device_id: Uuid,
	pub voucher_device_id: Uuid,
	pub vouchee_device_id: Uuid,
	pub vouchee_device_info: DeviceInfo,
	pub vouchee_public_key: Vec<u8>,
	pub voucher_signature: Vec<u8>,
	pub proxied_session_keys: SessionKeys,
	pub created_at: DateTime<Utc>,
	pub expires_at: DateTime<Utc>,
	pub status: VouchQueueStatus,
	pub retry_count: u32,
	pub last_attempt_at: Option<DateTime<Utc>>,
}

pub struct VouchingQueue {
	conn: DatabaseConnection,
}

impl VouchingQueue {
	pub async fn open(data_dir: impl AsRef<Path>) -> Result<Self> {
		let db_path = data_dir
			.as_ref()
			.join("networking")
			.join("vouching_queue.db");
		let database_url = format!("sqlite://{}?mode=rwc", db_path.display());
		let conn = Database::connect(&database_url).await.map_err(|e| {
			NetworkingError::Protocol(format!("Failed to open vouching queue: {}", e))
		})?;

		Self::init_table(&conn).await?;

		Ok(Self { conn })
	}

	fn serialize<T: Serialize>(value: &T) -> Result<String> {
		serde_json::to_string(value).map_err(NetworkingError::Serialization)
	}

	fn deserialize<T: for<'de> Deserialize<'de>>(value: &str) -> Result<T> {
		serde_json::from_str(value).map_err(NetworkingError::Serialization)
	}

	async fn init_table(conn: &DatabaseConnection) -> Result<()> {
		conn.execute(Statement::from_string(
			DbBackend::Sqlite,
			r#"
			CREATE TABLE IF NOT EXISTS vouching_queue (
				id INTEGER PRIMARY KEY AUTOINCREMENT,
				session_id TEXT NOT NULL,
				target_device_id TEXT NOT NULL,
				voucher_device_id TEXT NOT NULL,
				vouchee_device_id TEXT NOT NULL,
				vouchee_device_info TEXT NOT NULL,
				vouchee_public_key BLOB NOT NULL,
				voucher_signature BLOB NOT NULL,
				proxied_session_keys TEXT NOT NULL,
				created_at TEXT NOT NULL,
				expires_at TEXT NOT NULL,
				status TEXT NOT NULL,
				retry_count INTEGER DEFAULT 0,
				last_attempt_at TEXT,

				UNIQUE(session_id, target_device_id)
			)
			"#
			.to_string(),
		))
		.await
		.map_err(|e| {
			NetworkingError::Protocol(format!("Failed to create vouching queue: {}", e))
		})?;

		conn.execute(Statement::from_string(
			DbBackend::Sqlite,
			"CREATE INDEX IF NOT EXISTS idx_vouching_queue_target ON vouching_queue(target_device_id)"
				.to_string(),
		))
		.await
		.map_err(|e| NetworkingError::Protocol(format!("Failed to index vouching queue: {}", e)))?;

		conn.execute(Statement::from_string(
			DbBackend::Sqlite,
			"CREATE INDEX IF NOT EXISTS idx_vouching_queue_expires ON vouching_queue(expires_at)"
				.to_string(),
		))
		.await
		.map_err(|e| NetworkingError::Protocol(format!("Failed to index vouching queue: {}", e)))?;

		Ok(())
	}

	pub async fn upsert_entry(&self, entry: &VouchingQueueEntry) -> Result<()> {
		self.conn
			.execute(Statement::from_sql_and_values(
				DbBackend::Sqlite,
				r#"
				INSERT INTO vouching_queue (
					session_id,
					target_device_id,
					voucher_device_id,
					vouchee_device_id,
					vouchee_device_info,
					vouchee_public_key,
					voucher_signature,
					proxied_session_keys,
					created_at,
					expires_at,
					status,
					retry_count,
					last_attempt_at
				)
				VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
				ON CONFLICT(session_id, target_device_id) DO UPDATE SET
					voucher_device_id = excluded.voucher_device_id,
					vouchee_device_id = excluded.vouchee_device_id,
					vouchee_device_info = excluded.vouchee_device_info,
					vouchee_public_key = excluded.vouchee_public_key,
					voucher_signature = excluded.voucher_signature,
					proxied_session_keys = excluded.proxied_session_keys,
					created_at = excluded.created_at,
					expires_at = excluded.expires_at,
					status = excluded.status,
					retry_count = excluded.retry_count,
					last_attempt_at = excluded.last_attempt_at
				"#,
				vec![
					entry.session_id.to_string().into(),
					entry.target_device_id.to_string().into(),
					entry.voucher_device_id.to_string().into(),
					entry.vouchee_device_id.to_string().into(),
					Self::serialize(&entry.vouchee_device_info)?.into(),
					entry.vouchee_public_key.clone().into(),
					entry.voucher_signature.clone().into(),
					Self::serialize(&entry.proxied_session_keys)?.into(),
					entry.created_at.to_rfc3339().into(),
					entry.expires_at.to_rfc3339().into(),
					entry.status.as_str().into(),
					(entry.retry_count as i64).into(),
					entry
						.last_attempt_at
						.map(|ts| ts.to_rfc3339())
						.unwrap_or_default()
						.into(),
				],
			))
			.await
			.map_err(|e| NetworkingError::Protocol(format!("Failed to upsert vouch: {}", e)))?;

		Ok(())
	}

	pub async fn list_entries(&self) -> Result<Vec<VouchingQueueEntry>> {
		let rows = self
			.conn
			.query_all(Statement::from_string(
				DbBackend::Sqlite,
				r#"
				SELECT session_id, target_device_id, voucher_device_id, vouchee_device_id,
					vouchee_device_info, vouchee_public_key, voucher_signature,
					proxied_session_keys, created_at, expires_at, status,
					retry_count, last_attempt_at
				FROM vouching_queue
				"#
				.to_string(),
			))
			.await
			.map_err(|e| NetworkingError::Protocol(format!("Failed to list vouches: {}", e)))?;

		let mut entries = Vec::new();
		for row in rows {
			let session_id: String = row.try_get("", "session_id").map_err(|e| {
				NetworkingError::Protocol(format!("Failed to read session_id: {}", e))
			})?;
			let target_device_id: String = row.try_get("", "target_device_id").map_err(|e| {
				NetworkingError::Protocol(format!("Failed to read target_device_id: {}", e))
			})?;
			let voucher_device_id: String = row.try_get("", "voucher_device_id").map_err(|e| {
				NetworkingError::Protocol(format!("Failed to read voucher_device_id: {}", e))
			})?;
			let vouchee_device_id: String = row.try_get("", "vouchee_device_id").map_err(|e| {
				NetworkingError::Protocol(format!("Failed to read vouchee_device_id: {}", e))
			})?;
			let vouchee_device_info: String =
				row.try_get("", "vouchee_device_info").map_err(|e| {
					NetworkingError::Protocol(format!("Failed to read vouchee_device_info: {}", e))
				})?;
			let vouchee_public_key: Vec<u8> =
				row.try_get("", "vouchee_public_key").map_err(|e| {
					NetworkingError::Protocol(format!("Failed to read vouchee_public_key: {}", e))
				})?;
			let voucher_signature: Vec<u8> = row.try_get("", "voucher_signature").map_err(|e| {
				NetworkingError::Protocol(format!("Failed to read voucher_signature: {}", e))
			})?;
			let proxied_session_keys: String =
				row.try_get("", "proxied_session_keys").map_err(|e| {
					NetworkingError::Protocol(format!("Failed to read proxied_session_keys: {}", e))
				})?;
			let created_at: String = row.try_get("", "created_at").map_err(|e| {
				NetworkingError::Protocol(format!("Failed to read created_at: {}", e))
			})?;
			let expires_at: String = row.try_get("", "expires_at").map_err(|e| {
				NetworkingError::Protocol(format!("Failed to read expires_at: {}", e))
			})?;
			let status: String = row
				.try_get("", "status")
				.map_err(|e| NetworkingError::Protocol(format!("Failed to read status: {}", e)))?;
			let retry_count: i64 = row.try_get("", "retry_count").map_err(|e| {
				NetworkingError::Protocol(format!("Failed to read retry_count: {}", e))
			})?;
			let last_attempt_at: Option<String> = row.try_get("", "last_attempt_at").ok();

			let entry = VouchingQueueEntry {
				session_id: Uuid::parse_str(&session_id)
					.map_err(|e| NetworkingError::Protocol(format!("Invalid session_id: {}", e)))?,
				target_device_id: Uuid::parse_str(&target_device_id).map_err(|e| {
					NetworkingError::Protocol(format!("Invalid target_device_id: {}", e))
				})?,
				voucher_device_id: Uuid::parse_str(&voucher_device_id).map_err(|e| {
					NetworkingError::Protocol(format!("Invalid voucher_device_id: {}", e))
				})?,
				vouchee_device_id: Uuid::parse_str(&vouchee_device_id).map_err(|e| {
					NetworkingError::Protocol(format!("Invalid vouchee_device_id: {}", e))
				})?,
				vouchee_device_info: Self::deserialize(&vouchee_device_info)?,
				vouchee_public_key,
				voucher_signature,
				proxied_session_keys: Self::deserialize(&proxied_session_keys)?,
				created_at: DateTime::parse_from_rfc3339(&created_at)
					.map_err(|e| NetworkingError::Protocol(format!("Invalid created_at: {}", e)))?
					.with_timezone(&Utc),
				expires_at: DateTime::parse_from_rfc3339(&expires_at)
					.map_err(|e| NetworkingError::Protocol(format!("Invalid expires_at: {}", e)))?
					.with_timezone(&Utc),
				status: VouchQueueStatus::from_str(&status),
				retry_count: retry_count as u32,
				last_attempt_at: last_attempt_at
					.and_then(|ts| DateTime::parse_from_rfc3339(&ts).ok())
					.map(|ts| ts.with_timezone(&Utc)),
			};

			entries.push(entry);
		}

		Ok(entries)
	}

	pub async fn update_status(
		&self,
		session_id: Uuid,
		target_device_id: Uuid,
		status: VouchQueueStatus,
		retry_count: u32,
		last_attempt_at: Option<DateTime<Utc>>,
	) -> Result<()> {
		self.conn
			.execute(Statement::from_sql_and_values(
				DbBackend::Sqlite,
				r#"
				UPDATE vouching_queue
				SET status = ?, retry_count = ?, last_attempt_at = ?
				WHERE session_id = ? AND target_device_id = ?
				"#,
				vec![
					status.as_str().into(),
					(retry_count as i64).into(),
					last_attempt_at
						.map(|ts| ts.to_rfc3339())
						.unwrap_or_default()
						.into(),
					session_id.to_string().into(),
					target_device_id.to_string().into(),
				],
			))
			.await
			.map_err(|e| NetworkingError::Protocol(format!("Failed to update vouch: {}", e)))?;

		Ok(())
	}

	pub async fn remove_entry(&self, session_id: Uuid, target_device_id: Uuid) -> Result<()> {
		self.conn
			.execute(Statement::from_sql_and_values(
				DbBackend::Sqlite,
				"DELETE FROM vouching_queue WHERE session_id = ? AND target_device_id = ?",
				vec![
					session_id.to_string().into(),
					target_device_id.to_string().into(),
				],
			))
			.await
			.map_err(|e| NetworkingError::Protocol(format!("Failed to delete vouch: {}", e)))?;

		Ok(())
	}

	pub async fn remove_expired(&self, now: DateTime<Utc>) -> Result<u64> {
		let result = self
			.conn
			.execute(Statement::from_sql_and_values(
				DbBackend::Sqlite,
				"DELETE FROM vouching_queue WHERE expires_at <= ?",
				vec![now.to_rfc3339().into()],
			))
			.await
			.map_err(|e| {
				NetworkingError::Protocol(format!("Failed to delete expired vouches: {}", e))
			})?;

		Ok(result.rows_affected)
	}
}
