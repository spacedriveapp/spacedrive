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
CREATE INDEX IF NOT EXISTS idx_space_event_to_path ON space_event(to_path, seq);
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

/// Which events [`SpaceEventLog::read`] returns.
#[derive(Debug, Clone, Default)]
pub struct EventQuery {
	/// Oldest first, the events after this seq; with `newest`, newest first,
	/// the events before it. None starts at the oldest or the newest.
	pub cursor: Option<i64>,
	pub newest: bool,
	/// Only events whose path or move target is this path or inside it.
	pub path: Option<String>,
	pub limit: u32,
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
	/// Close the log, waiting for writes in flight, so its folder can be
	/// deleted.
	pub async fn close(&self) {
		self.pool.close().await;
	}

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

	/// Events in sequence order, a page at a time.
	pub async fn read(&self, query: &EventQuery) -> Result<Vec<SpaceEvent>> {
		let mut sql = String::from(
			"SELECT seq, space_id, at, actor_id, grant_id, origin, op, path, to_path,
				before_hash, after_hash, size, undo_id, authority_epoch
			 FROM space_event WHERE ",
		);
		sql.push_str(if query.newest { "seq < ?1" } else { "seq > ?1" });
		let prefix = query
			.path
			.as_deref()
			.map(|path| path.trim_matches('/'))
			.filter(|path| !path.is_empty());
		if prefix.is_some() {
			// Every path below a folder sorts between "folder/" and "folder0",
			// since '0' is the byte after '/', so both path indexes serve it.
			sql.push_str(
				" AND (path = ?3 OR (path > ?4 AND path < ?5)
				  OR to_path = ?3 OR (to_path > ?4 AND to_path < ?5))",
			);
		}
		sql.push_str(if query.newest {
			" ORDER BY seq DESC LIMIT ?2"
		} else {
			" ORDER BY seq LIMIT ?2"
		});
		let cursor = query
			.cursor
			.unwrap_or(if query.newest { i64::MAX } else { 0 });
		let mut statement = sqlx::query_as(&sql).bind(cursor).bind(query.limit);
		if let Some(prefix) = prefix {
			statement = statement
				.bind(prefix.to_string())
				.bind(format!("{prefix}/"))
				.bind(format!("{prefix}0"));
		}
		Ok(statement.fetch_all(&self.pool).await?)
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

	fn page(cursor: Option<i64>, newest: bool, path: Option<&str>) -> EventQuery {
		EventQuery {
			cursor,
			newest,
			path: path.map(str::to_string),
			limit: 10,
		}
	}

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
		let all = log.read(&page(None, false, None)).await.unwrap();
		assert_eq!(
			all.iter().map(|e| e.path.as_str()).collect::<Vec<_>>(),
			["a.md", "b.md"]
		);
		assert_eq!(
			log.read(&page(Some(first), false, None))
				.await
				.unwrap()
				.len(),
			1
		);
		assert_eq!(log.last_for_path("a.md").await.unwrap().unwrap().seq, first);

		let third = log
			.append(&SpaceEvent {
				op: "move".into(),
				to_path: Some("docs/a.md".into()),
				..event("2026-11-27T00:00:01Z", "a.md")
			})
			.await
			.unwrap();
		log.append(&event("2026-11-27T00:00:02Z", "docsx/c.md"))
			.await
			.unwrap();
		let newest = log.read(&page(None, true, None)).await.unwrap();
		assert_eq!(newest.first().unwrap().path, "docsx/c.md");
		assert_eq!(newest.last().unwrap().seq, first);
		let before = log.read(&page(Some(third), true, None)).await.unwrap();
		assert_eq!(
			before.iter().map(|e| e.seq).collect::<Vec<_>>(),
			[second, first]
		);
		let in_docs = log.read(&page(None, true, Some("docs"))).await.unwrap();
		assert_eq!(
			in_docs.iter().map(|e| e.seq).collect::<Vec<_>>(),
			[third],
			"a move into the folder counts, a sibling with the same prefix does not"
		);
		let one_file = log.read(&page(None, true, Some("a.md"))).await.unwrap();
		assert_eq!(one_file.first().unwrap().seq, third);
		assert_eq!(one_file.len(), 2);

		assert_eq!(log.prune_before("2026-10-28T00:00:00Z").await.unwrap(), 1);
		assert_eq!(log.read(&page(None, false, None)).await.unwrap().len(), 3);
	}
}
