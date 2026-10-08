//! # Store schema version
//!
//! A source store carries its schema version in SQLite's `PRAGMA
//! user_version`. The number names the shape of the generation tables, not
//! the data type's facet columns, which [`crate::db::SourceDb::ensure_facet_columns`]
//! adds on their own.
//!
//! Only a writer migrates. It runs every migration between the store's version
//! and [`SCHEMA_VERSION`], in order, each in its own transaction that also
//! writes the new version, before the record DDL and before the revision
//! triggers are installed. A fresh store skips straight to the current version
//! because the DDL that follows creates the current shape.
//!
//! A replica is a `VACUUM INTO` copy of the owner's store and so carries the
//! owner's version. Readers of a replica cannot migrate it and must read
//! whichever shape its version names until the owner upgrades.

use std::collections::HashMap;

use sqlx::{Connection, SqlitePool};
use uuid::Uuid;

use crate::content::uuid_for;
use crate::error::{Error, Result};

/// The version a writer brings a store to.
///
/// 1. `content` rebuilt so a row's integrity hash belongs to every record on
///    it: candidate rows keyed by sampled hash, confirmed rows by integrity
///    hash, `candidate_uuid` beside `uuid`.
/// 2. `content.kind_name` for the extension kind that identified the bytes,
///    nullable and never backfilled, and an index over the lowercase file
///    extension so a reidentification pass by extension does not walk the
///    facet table.
pub const SCHEMA_VERSION: i64 = 2;

/// The version a store carries. `0` for a store written before versions
/// existed.
pub async fn version(pool: &SqlitePool) -> Result<i64> {
	Ok(sqlx::query_scalar("PRAGMA user_version")
		.fetch_one(pool)
		.await?)
}

/// Bring a store to [`SCHEMA_VERSION`].
///
/// Refuses a store written by a newer Spacedrive, because downgrading would
/// read rows through a shape they do not have.
pub(crate) async fn run(pool: &SqlitePool) -> Result<()> {
	let current = version(pool).await?;
	if current > SCHEMA_VERSION {
		return Err(Error::UnsupportedGeneration(format!(
			"store schema version {current} is newer than this build's {SCHEMA_VERSION}"
		)));
	}
	if current == SCHEMA_VERSION {
		return Ok(());
	}

	let has_record: Option<i64> =
		sqlx::query_scalar("SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'record'")
			.fetch_optional(pool)
			.await?;
	if has_record.is_none() {
		sqlx::query(&format!("PRAGMA user_version = {SCHEMA_VERSION}"))
			.execute(pool)
			.await?;
		return Ok(());
	}

	for next in (current + 1)..=SCHEMA_VERSION {
		apply(pool, next).await?;
		tracing::info!(version = next, "migrated source store");
	}

	Ok(())
}

/// One migration, from `next - 1` to `next`, committed with its version.
async fn apply(pool: &SqlitePool, next: i64) -> Result<()> {
	match next {
		1 => content_rows_own_their_hash(pool).await,
		2 => content_rows_name_their_kind(pool).await,
		_ => Err(Error::Other(format!(
			"no migration defined for schema version {next}"
		))),
	}
}

/// The `content` table at schema version 1, as the migration creates it.
/// [`crate::record::RECORD_SCHEMA`] creates the same shape on a fresh store.
pub const CONTENT_V1: &str = "\
CREATE TABLE content (
    id INTEGER PRIMARY KEY,
    uuid BLOB NOT NULL,
    candidate_uuid BLOB,
    sampled_hash TEXT,
    integrity_hash TEXT UNIQUE,
    size INTEGER,
    kind INTEGER
)";

const CONTENT_V1_INDEXES: [&str; 4] = [
	"CREATE UNIQUE INDEX idx_content_candidate ON content(sampled_hash) \
	 WHERE integrity_hash IS NULL",
	"CREATE INDEX idx_content_sampled ON content(sampled_hash)",
	"CREATE INDEX idx_content_uuid ON content(uuid)",
	"CREATE INDEX idx_content_candidate_uuid ON content(candidate_uuid)",
];

/// The index a reidentification pass by extension needs. Lowercase because
/// the walk writes the extension as the name carries it and every lookup
/// compares lowercase. Created here for a store that predates it and by the
/// open path for a fresh one; `IF NOT EXISTS` makes both idempotent.
pub const FACET_FILE_EXTENSION_INDEX: &str =
	"CREATE INDEX IF NOT EXISTS idx_facet_file_extension ON facet_file(lower(extension))";

/// Migration 2: `content.kind_name`, nullable, with no backfill. Before this
/// no row carried a kind at all; the kind was derived from the file name on
/// every read. Rows identified from here on get `kind` and `kind_name` at
/// the content identity phase, and an extension that loads with kinds runs
/// one reidentification update by extension over the rows it can name.
async fn content_rows_name_their_kind(pool: &SqlitePool) -> Result<()> {
	let mut conn = pool.acquire().await?;
	let mut tx = conn.begin().await?;
	let present: Option<i64> =
		sqlx::query_scalar("SELECT 1 FROM pragma_table_info('content') WHERE name = 'kind_name'")
			.fetch_optional(&mut *tx)
			.await?;
	if present.is_none() {
		sqlx::query("ALTER TABLE content ADD COLUMN kind_name TEXT")
			.execute(&mut *tx)
			.await?;
	}
	let has_facet_file: Option<i64> = sqlx::query_scalar(
		"SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'facet_file'",
	)
	.fetch_optional(&mut *tx)
	.await?;
	if has_facet_file.is_some() {
		sqlx::query(FACET_FILE_EXTENSION_INDEX)
			.execute(&mut *tx)
			.await?;
	}
	sqlx::query("PRAGMA user_version = 2")
		.execute(&mut *tx)
		.await?;
	tx.commit().await?;
	Ok(())
}

/// Migration 1: rebuild `content` so that no record carries an integrity
/// hash its own bytes were never read for.
///
/// Before this, `sampled_hash` was unique, so every record with one sampled
/// hash shared one row, and an integrity hash read for one of them stood for
/// all of them. The store cannot tell which of those hashes are wrong, so
/// every row holding both hashes is demoted to a candidate: its integrity
/// hash is cleared and its uuid returns to the sampled derivation. A row with
/// an integrity hash and no sampled hash was only ever reached by the record
/// whose bytes produced it, so it stays confirmed; two such rows with one
/// hash merge. Rows no record points at are dropped, and a row with neither
/// hash goes with its records' `content_id` cleared, which re-queues them.
///
/// Every kept row keeps its `id`, so `record.content_id` still resolves. Tag
/// and overlay assertions anchored on a record whose row was demoted have
/// their content key rewritten to the candidate uuid, since the confirmed
/// uuid no longer names any row. That is the only change to assertion rows.
///
/// Runs with foreign keys off because `record.content_id` references the
/// table being dropped; the revision triggers on `content` go with the table
/// and are reinstalled by the open that runs this.
async fn content_rows_own_their_hash(pool: &SqlitePool) -> Result<()> {
	let mut conn = pool.acquire().await?;
	sqlx::query("PRAGMA foreign_keys = OFF")
		.execute(&mut *conn)
		.await?;
	let outcome = rebuild_content(&mut conn).await;
	sqlx::query("PRAGMA foreign_keys = ON")
		.execute(&mut *conn)
		.await?;
	outcome
}

/// Rows per insert statement: seven binds each, well under SQLite's limit.
const INSERT_BATCH: usize = 1000;

struct NewContent {
	id: i64,
	uuid: Uuid,
	candidate_uuid: Option<Uuid>,
	sampled_hash: Option<String>,
	integrity_hash: Option<String>,
	size: Option<i64>,
	kind: Option<i64>,
}

async fn insert_batch(tx: &mut sqlx::SqliteConnection, batch: &[NewContent]) -> Result<()> {
	if batch.is_empty() {
		return Ok(());
	}
	let sql = format!(
		"INSERT INTO content_new (id, uuid, candidate_uuid, sampled_hash, integrity_hash, size, kind) VALUES {}",
		vec!["(?, ?, ?, ?, ?, ?, ?)"; batch.len()].join(", ")
	);
	let mut query = sqlx::query(&sql);
	for row in batch {
		query = query
			.bind(row.id)
			.bind(row.uuid)
			.bind(row.candidate_uuid)
			.bind(&row.sampled_hash)
			.bind(&row.integrity_hash)
			.bind(row.size)
			.bind(row.kind);
	}
	query.execute(tx).await?;
	Ok(())
}

#[derive(sqlx::FromRow)]
struct OldContent {
	id: i64,
	uuid: Uuid,
	sampled_hash: Option<String>,
	integrity_hash: Option<String>,
	size: Option<i64>,
	kind: Option<i64>,
}

async fn rebuild_content(conn: &mut sqlx::SqliteConnection) -> Result<()> {
	let mut tx = conn.begin().await?;

	let present: Option<i64> =
		sqlx::query_scalar("SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'content'")
			.fetch_optional(&mut *tx)
			.await?;
	if present.is_none() {
		sqlx::query("PRAGMA user_version = 1")
			.execute(&mut *tx)
			.await?;
		tx.commit().await?;
		return Ok(());
	}

	let rows: Vec<OldContent> = sqlx::query_as(
		"SELECT id, uuid, sampled_hash, integrity_hash, size, kind FROM content
		 WHERE id IN (SELECT content_id FROM record WHERE content_id IS NOT NULL)
		 ORDER BY id",
	)
	.fetch_all(&mut *tx)
	.await?;

	sqlx::query(&CONTENT_V1.replace("CREATE TABLE content", "CREATE TABLE content_new"))
		.execute(&mut *tx)
		.await?;

	// Rows are rewritten in batches of multi-row inserts: one statement per
	// row made a million-record store take about a minute, batched it takes
	// seconds. Merges and hashless rows are rare and handled one at a time.
	let mut confirmed: HashMap<String, i64> = HashMap::new();
	let mut batch: Vec<NewContent> = Vec::with_capacity(INSERT_BATCH);
	let mut kept = 0usize;
	let mut demoted = 0usize;
	let mut merged = 0usize;
	for row in &rows {
		let (uuid, candidate_uuid, integrity_hash) =
			match (row.sampled_hash.as_deref(), row.integrity_hash.as_deref()) {
				(Some(sampled), _) => (uuid_for(sampled), Some(uuid_for(sampled)), None),
				(None, Some(integrity)) => {
					if let Some(keep) = confirmed.get(integrity) {
						sqlx::query("UPDATE record SET content_id = ? WHERE content_id = ?")
							.bind(keep)
							.bind(row.id)
							.execute(&mut *tx)
							.await?;
						merged += 1;
						continue;
					}
					confirmed.insert(integrity.to_string(), row.id);
					(uuid_for(integrity), None, Some(integrity.to_string()))
				}
				(None, None) => {
					sqlx::query("UPDATE record SET content_id = NULL WHERE content_id = ?")
						.bind(row.id)
						.execute(&mut *tx)
						.await?;
					continue;
				}
			};
		kept += 1;
		if uuid != row.uuid {
			demoted += 1;
		}
		batch.push(NewContent {
			id: row.id,
			uuid,
			candidate_uuid,
			sampled_hash: row.sampled_hash.clone(),
			integrity_hash,
			size: row.size,
			kind: row.kind,
		});
		if batch.len() == INSERT_BATCH {
			insert_batch(&mut tx, &batch).await?;
			batch.clear();
		}
	}
	insert_batch(&mut tx, &batch).await?;

	// Assertions keyed by a uuid their record's row no longer carries take
	// the row's new uuid. Rows keep their ids, so old and new join on id.
	for table in ["tag_assertion", "record_overlay"] {
		sqlx::query(&format!(
			"UPDATE {table} SET content_uuid = (
				 SELECT cn.uuid FROM record r JOIN content_new cn ON cn.id = r.content_id
				 WHERE r.uuid = {table}.record_uuid)
			 WHERE content_uuid IS NOT NULL AND EXISTS (
				 SELECT 1 FROM record r
				 JOIN content c ON c.id = r.content_id
				 JOIN content_new cn ON cn.id = c.id
				 WHERE r.uuid = {table}.record_uuid
				 AND c.uuid = {table}.content_uuid AND cn.uuid IS NOT c.uuid)"
		))
		.execute(&mut *tx)
		.await?;
	}

	sqlx::query("DROP TABLE content").execute(&mut *tx).await?;
	sqlx::query("ALTER TABLE content_new RENAME TO content")
		.execute(&mut *tx)
		.await?;
	for sql in CONTENT_V1_INDEXES {
		sqlx::query(sql).execute(&mut *tx).await?;
	}
	sqlx::query("PRAGMA user_version = 1")
		.execute(&mut *tx)
		.await?;
	tx.commit().await?;

	tracing::info!(
		kept,
		demoted,
		merged,
		"rebuilt content rows to carry their own integrity hash"
	);
	Ok(())
}
