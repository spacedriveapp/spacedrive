//! The universal record table shared by every source index.
//!
//! Every indexed thing — a note, an email, a channel, a repository — is a
//! `record` row. Type-specific columns live in facet tables keyed by
//! `record.uuid`, generated from the data type's TOML models
//! ([`crate::schema::codegen`]). The record table is what makes cross-source
//! search possible: every source is queried through one shape, not a
//! per-data-type one. `edge` relates records inside a single source.
//!
//! `record_overlay` sits beside them, holding what no ingest produced: the
//! scalar assertions a person makes about a record. It keys on the record uuid,
//! which survives a rename, and carries `(type, external_id, content_uuid)` as
//! rebind evidence, which is what a copy of this store arriving on another device has
//! to work from. `hlc` and `device_uuid` order the merge when two devices have
//! both written. `docs/core/design/source-durability.md` carries the reasoning.
//!
//! `record_overlay` deliberately has no foreign key to `record`. Assertions
//! outlive the generation: dropping every record row to re-index must leave
//! them standing, and a cascade would delete exactly the rows nothing can
//! rebuild.

use crate::error::Result;
use uuid::Uuid;

/// Applied to every per-source index on open. All statements are
/// `IF NOT EXISTS`, so re-applying to a populated index is a no-op.
///
/// ## How a record is addressed
///
/// `external_id` is the source's own key and is nullable, because not every
/// source has one for every record. An adapter record has an opaque id from
/// the API it came from. A filesystem *directory* carries its path. A
/// filesystem *file* carries nothing: it is addressed by
/// `(parent_uuid, title)`, and its path is produced by joining its parent's
/// `directory_path` row to its name.
///
/// Storing the full path on every record cost more than half the database on a
/// two million record source, once in the table and again in the unique index,
/// and bought a path lookup that `directory_path` answers in the same two
/// probes. Resolving `a/b/c/d.png` is one probe into `directory_path` for
/// `a/b/c` and one into `(parent_uuid, title)`; producing a file's path is the
/// same in reverse. Neither walks the tree.
///
/// It also makes a rename proportional to the directories under it rather than
/// to every record under it, which is the difference between rewriting a few
/// hundred rows and a few hundred thousand when someone drags a folder.
///
/// `UNIQUE (parent_uuid, title)` is the constraint that was always true and
/// never stated: two entries cannot share a name in one directory. NULLs are
/// distinct in SQLite, so an adapter record with no parent is unconstrained by
/// it, and `UNIQUE (type, external_id)` still catches two records claiming one
/// key.
pub const RECORD_SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS record (
    uuid BLOB PRIMARY KEY,
    external_id TEXT,
    type TEXT NOT NULL,
    title TEXT,
    created_at INTEGER,
    modified_at INTEGER,
    parent_uuid BLOB REFERENCES record(uuid) ON DELETE SET NULL,
    content_id INTEGER REFERENCES content(id),
    version TEXT,
    scan_epoch INTEGER,
    indexed_at TEXT NOT NULL DEFAULT (datetime('now')),
    UNIQUE (type, external_id)
);
CREATE INDEX IF NOT EXISTS idx_record_type ON record(type);
CREATE UNIQUE INDEX IF NOT EXISTS idx_record_sibling ON record(parent_uuid, title);

CREATE TABLE IF NOT EXISTS directory_path (
    record_uuid BLOB PRIMARY KEY REFERENCES record(uuid) ON DELETE CASCADE,
    path TEXT NOT NULL UNIQUE
);

CREATE TABLE IF NOT EXISTS content (
    id INTEGER PRIMARY KEY,
    uuid BLOB NOT NULL,
    sampled_hash TEXT UNIQUE,
    integrity_hash TEXT,
    size INTEGER,
    kind INTEGER
);
CREATE INDEX IF NOT EXISTS idx_content_uuid ON content(uuid);
CREATE INDEX IF NOT EXISTS idx_content_integrity ON content(integrity_hash);

CREATE TABLE IF NOT EXISTS edge (
    src_uuid BLOB NOT NULL REFERENCES record(uuid) ON DELETE CASCADE,
    dst_uuid BLOB NOT NULL REFERENCES record(uuid) ON DELETE CASCADE,
    type TEXT NOT NULL,
    ord REAL,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    PRIMARY KEY (src_uuid, dst_uuid, type)
);
CREATE INDEX IF NOT EXISTS idx_edge_dst ON edge(dst_uuid, type);

CREATE TABLE IF NOT EXISTS _sync_state (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL,
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS record_overlay (
    record_uuid BLOB PRIMARY KEY,
    type TEXT NOT NULL,
    external_id TEXT NOT NULL,
    content_uuid BLOB,
    fields TEXT NOT NULL DEFAULT '{}',
    hlc TEXT NOT NULL,
    device_uuid BLOB NOT NULL,
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX IF NOT EXISTS idx_overlay_external ON record_overlay(type, external_id);
CREATE INDEX IF NOT EXISTS idx_overlay_content ON record_overlay(content_uuid);

CREATE TABLE IF NOT EXISTS _schema (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    data_type_id TEXT NOT NULL,
    schema_hash TEXT NOT NULL,
    schema_toml TEXT NOT NULL,
    applied_at TEXT NOT NULL DEFAULT (datetime('now'))
);
"#;

/// The record-table write, shared by both ingests: the adapter path writes one
/// at a time through [`crate::db::SourceDb::upsert`], the walker writes
/// thousands inside one transaction. Two writers, one statement.
///
/// The conflict target is the uuid rather than `(type, external_id)`, because
/// the walker resolves identity before it writes and a moved file is the same
/// record at a new key. `UNIQUE (type, external_id)` still stands, so two
/// records claiming one path fails loudly instead of quietly.
pub(crate) const INSERT_RECORD: &str = "\
INSERT INTO record
	(uuid, external_id, type, title, created_at, modified_at,
	 parent_uuid, content_id, scan_epoch, indexed_at)
 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, datetime('now'))
 ON CONFLICT (uuid) DO UPDATE SET
	external_id = excluded.external_id,
	type = excluded.type,
	title = excluded.title,
	created_at = excluded.created_at,
	modified_at = excluded.modified_at,
	parent_uuid = excluded.parent_uuid,
	scan_epoch = excluded.scan_epoch,
	indexed_at = excluded.indexed_at";

/// [`INSERT_RECORD`] with a record bound to it, ready for a pool or a
/// transaction.
pub(crate) fn insert_record_query<'q>(
	record: &'q Record,
	epoch: i64,
) -> sqlx::query::Query<'q, sqlx::Sqlite, sqlx::sqlite::SqliteArguments<'q>> {
	sqlx::query(INSERT_RECORD)
		.bind(record.uuid)
		.bind(&record.external_id)
		.bind(&record.type_)
		.bind(&record.title)
		.bind(record.created_at)
		.bind(record.modified_at)
		.bind(record.parent_uuid)
		.bind(record.content_id)
		.bind(epoch)
}

/// A record row as written by the ingest path.
#[derive(Debug, Clone)]
pub struct Record {
	pub uuid: Uuid,
	/// The source's own key, where it has one. `None` for a filesystem file,
	/// which is addressed by its parent and its name.
	pub external_id: Option<String>,
	/// Open type key: the adapter's model name.
	pub type_: String,
	pub title: Option<String>,
	/// Unix milliseconds, parsed from the source's own timestamp fields.
	pub created_at: Option<i64>,
	pub modified_at: Option<i64>,
	pub parent_uuid: Option<Uuid>,
	/// The local `content` row. Stable across the hash ladder, unlike
	/// `content.uuid`, which is re-derived when the integrity hash lands.
	pub content_id: Option<i64>,
}

/// A row in the `content` table: the identity of the bytes a record points at.
/// The stored `uuid` is derived from whichever hash is present
/// ([`crate::content`]), so it is the same on every machine that sees the same
/// bytes.
#[derive(Debug, Clone, Default)]
pub struct ContentIdentity {
	/// Cheap tier — hash over sampled regions.
	pub sampled_hash: Option<String>,
	/// Full tier — hash over every byte.
	pub integrity_hash: Option<String>,
	pub size: Option<i64>,
	pub kind: Option<i64>,
}

/// Facet table name for a model. Namespaced so a model can never collide with a
/// record table.
pub fn facet_table(model: &str) -> String {
	format!("facet_{model}")
}

/// The next scan epoch: one past the highest stamped so far, 1 on a fresh index.
/// Every record written during a sync carries that run's epoch, which records
/// when a record was last seen at the source.
///
/// Removal is driven by the adapter's explicit `delete` operations, not by a
/// stale-epoch sweep: the JSONL protocol is a delta stream resumed from a
/// cursor, so an incremental run legitimately touches only a handful of records
/// and the untouched remainder is still present at the source.
pub async fn next_scan_epoch(pool: &sqlx::SqlitePool) -> Result<i64> {
	let max: Option<i64> = sqlx::query_scalar("SELECT MAX(scan_epoch) FROM record")
		.fetch_one(pool)
		.await?;
	Ok(max.unwrap_or(0) + 1)
}
