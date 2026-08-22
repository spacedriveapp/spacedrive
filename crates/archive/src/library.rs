//! What no ingest produced: user and agent assertions, addressed so they
//! outlive the source store they describe.
//!
//! Removing a source and adding it back mints fresh record uuids, so nothing
//! here keys on one. `(source_id, type, external_id)` is the address instead,
//! and the assertions rebind on the way back in.
//!
//! These tables live in `registry.db` alongside the source registry. Moving
//! them into each source's own file is P1 of
//! `docs/plans/2026-08-22-source-convergence.md`; `lib_edge` is the open
//! question there, being cross-source by definition.

use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

use crate::error::Result;

/// Durable tables, applied idempotently when the registry opens.
pub const LIBRARY_SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS record_overlay (
    source_id TEXT NOT NULL,
    type TEXT NOT NULL,
    external_id TEXT NOT NULL,
    fields TEXT NOT NULL DEFAULT '{}',
    updated_at TEXT NOT NULL DEFAULT (datetime('now')),
    PRIMARY KEY (source_id, type, external_id)
);

CREATE TABLE IF NOT EXISTS grouping (
    uuid TEXT PRIMARY KEY,
    type TEXT NOT NULL,
    title TEXT,
    predicate TEXT,
    custom_data TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX IF NOT EXISTS idx_grouping_type ON grouping(type);

CREATE TABLE IF NOT EXISTS lib_edge (
    src_source TEXT NOT NULL,
    src_type TEXT NOT NULL,
    src_external TEXT NOT NULL,
    dst_source TEXT NOT NULL,
    dst_type TEXT NOT NULL,
    dst_external TEXT NOT NULL,
    type TEXT NOT NULL,
    ord REAL,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    PRIMARY KEY (src_source, src_type, src_external,
                 dst_source, dst_type, dst_external, type)
);
CREATE INDEX IF NOT EXISTS idx_lib_edge_dst
    ON lib_edge(dst_source, dst_type, dst_external, type);
"#;

/// Addresses a record durably, independent of the record table uuid a given index
/// happens to have minted for it.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct RecordKey {
	pub source_id: String,
	pub type_: String,
	pub external_id: String,
}

impl RecordKey {
	pub fn new(
		source_id: impl Into<String>,
		type_: impl Into<String>,
		external_id: impl Into<String>,
	) -> Self {
		Self {
			source_id: source_id.into(),
			type_: type_.into(),
			external_id: external_id.into(),
		}
	}
}

/// A curated collection: an album, a person, a tag, a saved search. Manual
/// membership is expressed as edges pointing at it; a `predicate` makes the
/// same primitive a smart list.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Grouping {
	pub uuid: String,
	pub type_: String,
	pub title: Option<String>,
	pub predicate: Option<String>,
	pub custom_data: Option<serde_json::Value>,
	pub created_at: String,
}

/// A durable cross-record edge.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LibEdge {
	pub src: RecordKey,
	pub dst: RecordKey,
	pub type_: String,
	pub ord: Option<f64>,
}

/// Handle to the durable knowledge tables.
pub struct Library {
	pool: SqlitePool,
}

impl Library {
	/// Apply the schema and return a handle. Shares the registry's pool.
	pub async fn new(pool: SqlitePool) -> Result<Self> {
		sqlx::raw_sql(LIBRARY_SCHEMA).execute(&pool).await?;
		Ok(Self { pool })
	}

	/// Merge fields onto a record's overlay. A field set to JSON `null` is
	/// removed; the rest are shallow-merged over what is already stored.
	pub async fn set_overlay(
		&self,
		key: &RecordKey,
		fields: &serde_json::Value,
	) -> Result<serde_json::Value> {
		let incoming = fields.as_object().ok_or_else(|| {
			crate::error::Error::Other("overlay fields must be a JSON object".to_string())
		})?;

		let mut merged = match self.get_overlay(key).await? {
			serde_json::Value::Object(map) => map,
			_ => serde_json::Map::new(),
		};

		for (k, v) in incoming {
			if v.is_null() {
				merged.remove(k);
			} else {
				merged.insert(k.clone(), v.clone());
			}
		}

		let encoded = serde_json::to_string(&merged)?;
		sqlx::query(
			"INSERT INTO record_overlay (source_id, type, external_id, fields, updated_at)
			 VALUES (?, ?, ?, ?, datetime('now'))
			 ON CONFLICT (source_id, type, external_id) DO UPDATE SET
				fields = excluded.fields, updated_at = excluded.updated_at",
		)
		.bind(&key.source_id)
		.bind(&key.type_)
		.bind(&key.external_id)
		.bind(&encoded)
		.execute(&self.pool)
		.await?;

		Ok(serde_json::Value::Object(merged))
	}

	/// Read one record's overlay. Returns an empty object when none is stored.
	pub async fn get_overlay(&self, key: &RecordKey) -> Result<serde_json::Value> {
		let row: Option<(String,)> = sqlx::query_as(
			"SELECT fields FROM record_overlay
			 WHERE source_id = ? AND type = ? AND external_id = ?",
		)
		.bind(&key.source_id)
		.bind(&key.type_)
		.bind(&key.external_id)
		.fetch_optional(&self.pool)
		.await?;

		Ok(row
			.map(|(fields,)| serde_json::from_str(&fields).unwrap_or_else(|_| empty_object()))
			.unwrap_or_else(empty_object))
	}

	/// Read overlays for many records of one source and type in a single query.
	/// Records without an overlay are absent from the map.
	pub async fn overlays_for(
		&self,
		source_id: &str,
		type_: &str,
		external_ids: &[String],
	) -> Result<std::collections::HashMap<String, serde_json::Value>> {
		if external_ids.is_empty() {
			return Ok(std::collections::HashMap::new());
		}

		let placeholders = vec!["?"; external_ids.len()].join(", ");
		let sql = format!(
			"SELECT external_id, fields FROM record_overlay
			 WHERE source_id = ? AND type = ? AND external_id IN ({placeholders})"
		);

		let mut query = sqlx::query_as::<_, (String, String)>(&sql)
			.bind(source_id)
			.bind(type_);
		for id in external_ids {
			query = query.bind(id);
		}

		let rows = query.fetch_all(&self.pool).await?;
		Ok(rows
			.into_iter()
			.map(|(external_id, fields)| {
				let parsed = serde_json::from_str(&fields).unwrap_or_else(|_| empty_object());
				(external_id, parsed)
			})
			.collect())
	}

	/// Drop every overlay belonging to a source. Only for a caller that means to
	/// discard the assertions, not for ordinary source removal.
	pub async fn clear_source_overlays(&self, source_id: &str) -> Result<u64> {
		let result = sqlx::query("DELETE FROM record_overlay WHERE source_id = ?")
			.bind(source_id)
			.execute(&self.pool)
			.await?;
		Ok(result.rows_affected())
	}

	/// Create a durable edge between two records.
	pub async fn link(&self, src: &RecordKey, dst: &RecordKey, type_: &str) -> Result<()> {
		sqlx::query(
			"INSERT INTO lib_edge
				(src_source, src_type, src_external, dst_source, dst_type, dst_external, type)
			 VALUES (?, ?, ?, ?, ?, ?, ?)
			 ON CONFLICT DO NOTHING",
		)
		.bind(&src.source_id)
		.bind(&src.type_)
		.bind(&src.external_id)
		.bind(&dst.source_id)
		.bind(&dst.type_)
		.bind(&dst.external_id)
		.bind(type_)
		.execute(&self.pool)
		.await?;
		Ok(())
	}

	/// Remove a durable edge.
	pub async fn unlink(&self, src: &RecordKey, dst: &RecordKey, type_: &str) -> Result<()> {
		sqlx::query(
			"DELETE FROM lib_edge
			 WHERE src_source = ? AND src_type = ? AND src_external = ?
			   AND dst_source = ? AND dst_type = ? AND dst_external = ? AND type = ?",
		)
		.bind(&src.source_id)
		.bind(&src.type_)
		.bind(&src.external_id)
		.bind(&dst.source_id)
		.bind(&dst.type_)
		.bind(&dst.external_id)
		.bind(type_)
		.execute(&self.pool)
		.await?;
		Ok(())
	}

	/// Edges touching a record, in either direction. The bool is `true` when the
	/// record is the edge's source.
	pub async fn neighbors(
		&self,
		key: &RecordKey,
		edge_type: Option<&str>,
	) -> Result<Vec<(LibEdge, bool)>> {
		let mut sql = String::from(
			"SELECT src_source, src_type, src_external,
					dst_source, dst_type, dst_external, type, ord,
					(src_source = ? AND src_type = ? AND src_external = ?) AS outgoing
			 FROM lib_edge
			 WHERE ((src_source = ? AND src_type = ? AND src_external = ?)
				 OR (dst_source = ? AND dst_type = ? AND dst_external = ?))",
		);
		if edge_type.is_some() {
			sql.push_str(" AND type = ?");
		}

		let mut query = sqlx::query_as::<_, LibEdgeRow>(&sql);
		for _ in 0..3 {
			query = query
				.bind(&key.source_id)
				.bind(&key.type_)
				.bind(&key.external_id);
		}
		if let Some(t) = edge_type {
			query = query.bind(t);
		}

		let rows = query.fetch_all(&self.pool).await?;
		Ok(rows.into_iter().map(LibEdgeRow::split).collect())
	}

	/// Create or replace a grouping.
	pub async fn upsert_grouping(&self, grouping: &Grouping) -> Result<()> {
		let custom = grouping
			.custom_data
			.as_ref()
			.map(serde_json::to_string)
			.transpose()?;

		sqlx::query(
			"INSERT INTO grouping (uuid, type, title, predicate, custom_data)
			 VALUES (?, ?, ?, ?, ?)
			 ON CONFLICT (uuid) DO UPDATE SET
				type = excluded.type, title = excluded.title,
				predicate = excluded.predicate, custom_data = excluded.custom_data",
		)
		.bind(&grouping.uuid)
		.bind(&grouping.type_)
		.bind(&grouping.title)
		.bind(&grouping.predicate)
		.bind(&custom)
		.execute(&self.pool)
		.await?;
		Ok(())
	}

	/// List groupings, optionally of one type.
	pub async fn list_groupings(&self, type_: Option<&str>) -> Result<Vec<Grouping>> {
		let rows = match type_ {
			Some(t) => {
				sqlx::query_as::<_, GroupingRow>(
					"SELECT uuid, type, title, predicate, custom_data, created_at
				 FROM grouping WHERE type = ? ORDER BY created_at DESC",
				)
				.bind(t)
				.fetch_all(&self.pool)
				.await?
			}
			None => {
				sqlx::query_as::<_, GroupingRow>(
					"SELECT uuid, type, title, predicate, custom_data, created_at
				 FROM grouping ORDER BY created_at DESC",
				)
				.fetch_all(&self.pool)
				.await?
			}
		};

		Ok(rows.into_iter().map(GroupingRow::into_grouping).collect())
	}

	/// Delete a grouping and every durable edge that points at it.
	pub async fn delete_grouping(&self, uuid: &str) -> Result<()> {
		sqlx::query("DELETE FROM lib_edge WHERE dst_external = ? OR src_external = ?")
			.bind(uuid)
			.bind(uuid)
			.execute(&self.pool)
			.await?;
		sqlx::query("DELETE FROM grouping WHERE uuid = ?")
			.bind(uuid)
			.execute(&self.pool)
			.await?;
		Ok(())
	}
}

fn empty_object() -> serde_json::Value {
	serde_json::Value::Object(serde_json::Map::new())
}

#[derive(sqlx::FromRow)]
struct LibEdgeRow {
	src_source: String,
	src_type: String,
	src_external: String,
	dst_source: String,
	dst_type: String,
	dst_external: String,
	r#type: String,
	ord: Option<f64>,
	outgoing: i64,
}

impl LibEdgeRow {
	fn split(self) -> (LibEdge, bool) {
		let outgoing = self.outgoing != 0;
		(
			LibEdge {
				src: RecordKey::new(self.src_source, self.src_type, self.src_external),
				dst: RecordKey::new(self.dst_source, self.dst_type, self.dst_external),
				type_: self.r#type,
				ord: self.ord,
			},
			outgoing,
		)
	}
}

#[derive(sqlx::FromRow)]
struct GroupingRow {
	uuid: String,
	r#type: String,
	title: Option<String>,
	predicate: Option<String>,
	custom_data: Option<String>,
	created_at: String,
}

impl GroupingRow {
	fn into_grouping(self) -> Grouping {
		Grouping {
			uuid: self.uuid,
			type_: self.r#type,
			title: self.title,
			predicate: self.predicate,
			custom_data: self.custom_data.and_then(|s| serde_json::from_str(&s).ok()),
			created_at: self.created_at,
		}
	}
}
