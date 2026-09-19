//! # Store revision
//!
//! A store's revision counts the changes committed to what it holds. A
//! delivered copy is labeled with the revision it was taken at, so an owner
//! and a replica agree the copy is current exactly when their revisions match.
//!
//! The count has to live in the store, because the database files are not a
//! change signal. SQLite deletes the WAL when a pool's last connection closes
//! and recreates it on the next read, so the files' sizes and modification
//! times move while no row does.
//!
//! Triggers keep the count, so every path that writes is counted without
//! having to remember to. Each tracked table gets an insert, an update and a
//! delete trigger. The update trigger compares old and new values and ignores
//! the columns a writer restamps on rows it did not change, so an adapter
//! re-putting unchanged records or a walk stamping its epoch leaves the
//! revision where it was. `_sync_state` and `_schema` are bookkeeping and the
//! search index is derived from tracked rows, so none of them are tracked.
//!
//! The count is paired with a store id drawn when a writer first opens the
//! store, so a store recreated from scratch cannot repeat a revision that a
//! copy of its predecessor carries.

use sqlx::SqlitePool;
use uuid::Uuid;

use crate::error::{Error, Result};
use crate::record::facet_table;
use crate::schema::DataTypeSchema;

/// Tables whose rows a delivered copy carries, beside the data type's facet
/// tables.
const TRACKED_TABLES: [&str; 7] = [
	"record",
	"directory_path",
	"content",
	"edge",
	"record_overlay",
	"tag_definition",
	"tag_assertion",
];

/// `(table, column)` pairs a writer restamps on rows it did not otherwise
/// change.
const BOOKKEEPING_COLUMNS: [(&str, &str); 2] = [("record", "scan_epoch"), ("record", "indexed_at")];

const REVISION_TABLE: &str = "\
CREATE TABLE IF NOT EXISTS _revision (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    store_id BLOB NOT NULL,
    value INTEGER NOT NULL
)";

/// Which store, and how many changes it has committed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Revision {
	pub store_id: Uuid,
	pub value: i64,
}

impl Revision {
	/// How a store reads before a writer has opened it. Every write path opens
	/// the store as a writer first, which draws a real store id, so nothing
	/// can change the store while it reads this way.
	pub const UNTRACKED: Self = Self {
		store_id: Uuid::nil(),
		value: 0,
	};
}

/// Create the revision row and bring every tracked table's triggers in line
/// with its current columns.
///
/// Runs on every writable open, after facet columns are added. A trigger is
/// replaced only when the SQL it would be created with differs from what the
/// store holds, so reopening an unchanged store writes nothing, and a facet
/// column added since the last open is compared from this open on.
pub(crate) async fn install(pool: &SqlitePool, schema: &DataTypeSchema) -> Result<()> {
	let mut tx = pool.begin().await?;
	sqlx::query(REVISION_TABLE).execute(&mut *tx).await?;
	sqlx::query("INSERT OR IGNORE INTO _revision (id, store_id, value) VALUES (1, ?, 0)")
		.bind(Uuid::now_v7())
		.execute(&mut *tx)
		.await?;
	tx.commit().await?;

	let facets = schema.models.keys().map(|model| facet_table(model));
	for table in TRACKED_TABLES.map(str::to_string).into_iter().chain(facets) {
		let columns: Vec<String> = sqlx::query_scalar("SELECT name FROM pragma_table_info(?)")
			.bind(&table)
			.fetch_all(pool)
			.await?;
		if columns.is_empty() {
			return Err(Error::Other(format!(
				"tracked table {table} is missing from the store"
			)));
		}

		for (name, sql) in triggers(&table, &columns) {
			let current: Option<String> = sqlx::query_scalar(
				"SELECT sql FROM sqlite_master WHERE type = 'trigger' AND name = ?",
			)
			.bind(&name)
			.fetch_optional(pool)
			.await?;
			if current.as_deref() == Some(sql.as_str()) {
				continue;
			}

			let mut tx = pool.begin().await?;
			sqlx::query(&format!("DROP TRIGGER IF EXISTS \"{name}\""))
				.execute(&mut *tx)
				.await?;
			sqlx::query(&sql).execute(&mut *tx).await?;
			tx.commit().await?;
		}
	}

	Ok(())
}

/// Read a store's revision. Works on read-only handles, a delivered replica
/// included.
pub(crate) async fn read(pool: &SqlitePool) -> Result<Revision> {
	let tracked: Option<i64> = sqlx::query_scalar(
		"SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = '_revision'",
	)
	.fetch_optional(pool)
	.await?;
	if tracked.is_none() {
		return Ok(Revision::UNTRACKED);
	}

	let row: Option<(Uuid, i64)> =
		sqlx::query_as("SELECT store_id, value FROM _revision WHERE id = 1")
			.fetch_optional(pool)
			.await?;
	Ok(
		row.map_or(Revision::UNTRACKED, |(store_id, value)| Revision {
			store_id,
			value,
		}),
	)
}

/// The insert, update and delete triggers for one table, as `(name, sql)`.
///
/// The SQL is written the way SQLite stores it in `sqlite_master`, so
/// [`install`] can compare the two directly.
fn triggers(table: &str, columns: &[String]) -> [(String, String); 3] {
	let changed = columns
		.iter()
		.filter(|column| {
			!BOOKKEEPING_COLUMNS
				.iter()
				.any(|&(owner, bookkeeping)| owner == table && bookkeeping == column.as_str())
		})
		.map(|column| format!("OLD.\"{column}\" IS NOT NEW.\"{column}\""))
		.collect::<Vec<_>>()
		.join(" OR ");

	[
		("insert", "INSERT", String::new()),
		("update", "UPDATE", format!(" WHEN {changed}")),
		("delete", "DELETE", String::new()),
	]
	.map(|(suffix, event, when)| {
		let name = format!("{table}_revision_{suffix}");
		let sql = format!(
			"CREATE TRIGGER \"{name}\" AFTER {event} ON \"{table}\"{when} \
			 BEGIN UPDATE _revision SET value = value + 1 WHERE id = 1; END"
		);
		(name, sql)
	})
}
