//! The universal record table shared by every source index.
//!
//! Every indexed thing — a note, an email, a channel, a repository — is a
//! `record` row. Type-specific columns live in facet tables keyed by
//! `record.uuid`, generated from the data type's TOML models
//! ([`crate::schema::codegen`]). The record table is what makes cross-source search and
//! cross-source edges possible: they join on one shape, not per-data-type ones.
//!
//! A source index is disposable. Everything durable — user assertions, curated
//! groupings, cross-source edges — lives in the library ([`crate::library`]) and
//! rebinds by `(type, external_id)` when a source is re-added.

use crate::error::Result;

/// Applied to every per-source index on open. All statements are
/// `IF NOT EXISTS`, so re-applying to a populated index is a no-op.
pub const RECORD_SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS record (
    uuid TEXT PRIMARY KEY,
    external_id TEXT NOT NULL,
    type TEXT NOT NULL,
    title TEXT,
    created_at INTEGER,
    modified_at INTEGER,
    parent_uuid TEXT REFERENCES record(uuid) ON DELETE SET NULL,
    content_id INTEGER REFERENCES content(id),
    version TEXT,
    scan_epoch INTEGER,
    indexed_at TEXT NOT NULL DEFAULT (datetime('now')),
    _embedded_at TEXT,
    _safety_score INTEGER,
    _safety_verdict TEXT NOT NULL DEFAULT 'unscreened',
    _safety_version TEXT,
    UNIQUE (type, external_id)
);
CREATE INDEX IF NOT EXISTS idx_record_type ON record(type);
CREATE INDEX IF NOT EXISTS idx_record_parent ON record(parent_uuid);
CREATE INDEX IF NOT EXISTS idx_record_verdict ON record(_safety_verdict);

CREATE TABLE IF NOT EXISTS content (
    id INTEGER PRIMARY KEY,
    sampled_hash TEXT,
    integrity_hash TEXT,
    size INTEGER,
    kind INTEGER
);
CREATE INDEX IF NOT EXISTS idx_content_sampled ON content(sampled_hash);
CREATE INDEX IF NOT EXISTS idx_content_integrity ON content(integrity_hash);

CREATE TABLE IF NOT EXISTS edge (
    src_uuid TEXT NOT NULL REFERENCES record(uuid) ON DELETE CASCADE,
    dst_uuid TEXT NOT NULL REFERENCES record(uuid) ON DELETE CASCADE,
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

CREATE TABLE IF NOT EXISTS _schema (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    data_type_id TEXT NOT NULL,
    schema_hash TEXT NOT NULL,
    schema_toml TEXT NOT NULL,
    applied_at TEXT NOT NULL DEFAULT (datetime('now'))
);
"#;

/// A record row as written by the ingest path.
#[derive(Debug, Clone)]
pub struct Record {
	pub uuid: String,
	pub external_id: String,
	/// Open type key — the adapter's model name.
	pub type_: String,
	pub title: Option<String>,
	/// Unix milliseconds, parsed from the source's own timestamp fields.
	pub created_at: Option<i64>,
	pub modified_at: Option<i64>,
	pub parent_uuid: Option<String>,
	pub content_id: Option<i64>,
}

/// A row in the `content` table: the identity of the bytes a record points at.
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
