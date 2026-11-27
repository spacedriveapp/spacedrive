//! # Space event log
//!
//! Every change inside a space, with who made it: an agent through its grant,
//! a person through the app, or something outside Spacedrive that the watcher
//! saw. One file per space at `<store dir>/spaces/<space id>/events.db`, so the
//! log travels with its space and replicates by sequence number. It is kept
//! apart from `grant_audit`, which stays the bounded request log for security
//! review. `docs/core/design/spaces.md` has the model.
//!
//! Rows are kept for a retention period, 30 days by default, and pruned by the
//! writer. The log is metadata only; no file contents are stored here.

use std::path::Path;

use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions};
use sqlx::SqlitePool;
use uuid::Uuid;

use crate::error::Result;

/// The file name of a space's event log inside its directory.
pub const SPACE_EVENTS_FILE: &str = "events.db";

const SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS space_event (
    seq INTEGER PRIMARY KEY AUTOINCREMENT,
    space_id BLOB NOT NULL,
    at TEXT NOT NULL,
    actor_id BLOB NOT NULL,
    grant_id BLOB,
    origin TEXT NOT NULL,
    op TEXT NOT NULL,
    path TEXT NOT NULL,
    to_path TEXT,
    before_hash TEXT,
    after_hash TEXT,
    size INTEGER,
    undo_id BLOB,
    authority_epoch INTEGER NOT NULL DEFAULT 0
);
CREATE INDEX IF NOT EXISTS idx_space_event_at ON space_event(at);
CREATE INDEX IF NOT EXISTS idx_space_event_path ON space_event(path, seq);
"#;

/// One change inside a space.
#[derive(Debug, Clone, PartialEq, sqlx::FromRow)]
pub struct SpaceEvent {
	/// Assigned by the log on append; ignored when appending.
	pub seq: i64,
	pub space_id: Uuid,
	/// RFC 3339.
	pub at: String,
	pub actor_id: Uuid,
	pub grant_id: Option<Uuid>,
	/// `agent_write`, `app_op` or `watcher`.
	pub origin: String,
	/// `create`, `mkdir`, `modify`, `move` or `delete`.
	pub op: String,
	/// Relative to the space root, `/`-separated.
	pub path: String,
	pub to_path: Option<String>,
	/// BLAKE3, hex, when the writer knew it.
	pub before_hash: Option<String>,
	pub after_hash: Option<String>,
	pub size: Option<i64>,
	/// The `files.undo` job id that reverses the change.
	pub undo_id: Option<Uuid>,
	pub authority_epoch: i64,
}

/// A space's event log, open for reads and appends.
#[derive(Debug, Clone)]
pub struct SpaceEventLog {
	pool: SqlitePool,
}

impl SpaceEventLog {
	/// Open the log in `dir`, creating the directory and the file.
	pub async fn open(dir: &Path) -> Result<Self> {
		tokio::fs::create_dir_all(dir).await?;
		let options = SqliteConnectOptions::new()
			.filename(dir.join(SPACE_EVENTS_FILE))
			.create_if_missing(true)
			.journal_mode(SqliteJournalMode::Wal)
			.busy_timeout(std::time::Duration::from_secs(5));
		let pool = SqlitePoolOptions::new()
			.max_connections(2)
			.connect_with(options)
			.await?;
		sqlx::raw_sql(SCHEMA).execute(&pool).await?;
		Ok(Self { pool })
	}

	/// Append one event and answer its sequence number.
	pub async fn append(&self, event: &SpaceEvent) -> Result<i64> {
		let result = sqlx::query(
			"INSERT INTO space_event
				(space_id, at, actor_id, grant_id, origin, op, path, to_path,
				 before_hash, after_hash, size, undo_id, authority_epoch)
			 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
		)
		.bind(event.space_id)
		.bind(&event.at)
		.bind(event.actor_id)
		.bind(event.grant_id)
		.bind(&event.origin)
		.bind(&event.op)
		.bind(&event.path)
		.bind(&event.to_path)
		.bind(&event.before_hash)
		.bind(&event.after_hash)
		.bind(event.size)
		.bind(event.undo_id)
		.bind(event.authority_epoch)
		.execute(&self.pool)
		.await?;
		Ok(result.last_insert_rowid())
	}

	/// Events after `seq`, oldest first, at most `limit` of them.
	pub async fn since(&self, seq: i64, limit: u32) -> Result<Vec<SpaceEvent>> {
		Ok(sqlx::query_as(
			"SELECT seq, space_id, at, actor_id, grant_id, origin, op, path, to_path,
				before_hash, after_hash, size, undo_id, authority_epoch
			 FROM space_event WHERE seq > ? ORDER BY seq LIMIT ?",
		)
		.bind(seq)
		.bind(limit)
		.fetch_all(&self.pool)
		.await?)
	}

	/// The newest event that touched `path`, as its source or its target.
	pub async fn last_for_path(&self, path: &str) -> Result<Option<SpaceEvent>> {
		Ok(sqlx::query_as(
			"SELECT seq, space_id, at, actor_id, grant_id, origin, op, path, to_path,
				before_hash, after_hash, size, undo_id, authority_epoch
			 FROM space_event WHERE path = ? OR to_path = ? ORDER BY seq DESC LIMIT 1",
		)
		.bind(path)
		.bind(path)
		.fetch_optional(&self.pool)
		.await?)
	}

	/// Drop events older than `cutoff` (RFC 3339). Answers how many went.
	pub async fn prune_before(&self, cutoff: &str) -> Result<u64> {
		Ok(sqlx::query("DELETE FROM space_event WHERE at < ?")
			.bind(cutoff)
			.execute(&self.pool)
			.await?
			.rows_affected())
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	fn event(at: &str, path: &str) -> SpaceEvent {
		SpaceEvent {
			seq: 0,
			space_id: Uuid::from_u128(1),
			at: at.into(),
			actor_id: Uuid::from_u128(2),
			grant_id: None,
			origin: "watcher".into(),
			op: "modify".into(),
			path: path.into(),
			to_path: None,
			before_hash: None,
			after_hash: None,
			size: Some(3),
			undo_id: None,
			authority_epoch: 0,
		}
	}

	#[tokio::test]
	async fn events_read_back_in_order_and_old_ones_are_pruned() {
		let dir = tempfile::tempdir().unwrap();
		let log = SpaceEventLog::open(&dir.path().join("spaces/x"))
			.await
			.unwrap();
		let first = log
			.append(&event("2026-10-01T00:00:00Z", "a.md"))
			.await
			.unwrap();
		let second = log
			.append(&event("2026-11-27T00:00:00Z", "b.md"))
			.await
			.unwrap();
		assert!(second > first);
		let all = log.since(0, 10).await.unwrap();
		assert_eq!(
			all.iter().map(|e| e.path.as_str()).collect::<Vec<_>>(),
			["a.md", "b.md"]
		);
		assert_eq!(log.since(first, 10).await.unwrap().len(), 1);
		assert_eq!(log.last_for_path("a.md").await.unwrap().unwrap().seq, first);

		assert_eq!(log.prune_before("2026-10-28T00:00:00Z").await.unwrap(), 1);
		assert_eq!(log.since(0, 10).await.unwrap().len(), 1);
	}
}
