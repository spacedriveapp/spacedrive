//! SourceDb: handle for reading/writing records in a source index.
//!
//! Every write lands on the universal spine ([`crate::spine`]) first: one
//! `record` row carrying identity, hierarchy, timestamps and screening state.
//! The model's own declared fields go to its facet table, and relationships
//! become spine `edge` rows. Nothing is stored twice.

use std::fmt::Write;

use crate::error::{Error, Result};
use crate::schema::codegen::indexed_search_fields;
use crate::schema::DataTypeSchema;
use crate::spine::{facet_table, ContentIdentity, Record};

/// `_sync_state` key holding the on-disk root a file-backed source's
/// locator paths are relative to. File-backed adapters set it every sync;
/// its absence marks a foreign source whose records are primary.
pub const FILE_ROOT_CURSOR: &str = "file_root";

/// Handle to a single source's SQLite index.
pub struct SourceDb {
	pool: sqlx::SqlitePool,
	schema: DataTypeSchema,
	/// Stamped onto every record written through this handle, recording which
	/// sync run last saw it. Bumped once per run by [`SourceDb::begin_sync`].
	scan_epoch: std::sync::atomic::AtomicI64,
}

/// A record that needs embedding.
#[derive(Debug, Clone)]
pub struct EmbeddingRecord {
	pub id: String,
	pub content: String,
}

/// A record that needs safety screening.
#[derive(Debug, Clone)]
pub struct ScreeningRecord {
	pub id: String,
	pub content: String,
}

/// An item row from the primary record type.
#[derive(Debug, Clone)]
pub struct ItemRow {
	pub id: String,
	pub external_id: String,
	pub title: String,
	pub preview: Option<String>,
	pub subtitle: Option<String>,
}

/// An FTS search hit.
#[derive(Debug, Clone)]
pub struct FtsHit {
	pub id: String,
	pub external_id: String,
	pub title: String,
	pub preview: Option<String>,
	pub subtitle: Option<String>,
	pub rank: f64,
	pub date: Option<String>,
	pub safety_verdict: Option<String>,
	pub safety_score: Option<u8>,
}

/// A spine edge with the neighbouring record resolved.
#[derive(Debug, Clone)]
pub struct Neighbor {
	pub uuid: String,
	pub external_id: String,
	pub type_: String,
	pub title: Option<String>,
	pub edge_type: String,
	pub outgoing: bool,
}

/// Temporal filter for date range queries.
pub struct TemporalFilter<'a> {
	pub date_after: Option<&'a str>,
	pub date_before: Option<&'a str>,
}

/// Verdicts whose records are allowed into the search index.
const INDEXABLE_VERDICTS: [&str; 2] = ["safe", "flagged"];

impl SourceDb {
	/// Create a new SourceDb handle.
	pub(crate) fn new(pool: sqlx::SqlitePool, schema: DataTypeSchema, scan_epoch: i64) -> Self {
		Self {
			pool,
			schema,
			scan_epoch: std::sync::atomic::AtomicI64::new(scan_epoch),
		}
	}

	/// Get the underlying connection pool.
	pub fn pool(&self) -> &sqlx::SqlitePool {
		&self.pool
	}

	/// Get the schema.
	pub fn schema(&self) -> &DataTypeSchema {
		&self.schema
	}

	/// Open a new sync run: advance the scan epoch that subsequent writes carry.
	pub async fn begin_sync(&self) -> Result<i64> {
		let epoch = crate::spine::next_scan_epoch(&self.pool).await?;
		self.scan_epoch
			.store(epoch, std::sync::atomic::Ordering::Relaxed);
		Ok(epoch)
	}

	/// The epoch currently stamped onto writes.
	pub fn scan_epoch(&self) -> i64 {
		self.scan_epoch.load(std::sync::atomic::Ordering::Relaxed)
	}

	/// The data type's primary record type.
	fn primary_type(&self) -> &str {
		&self.schema.search.primary_model
	}

	/// Add facet columns declared since the index was created. Facet DDL is
	/// `CREATE TABLE IF NOT EXISTS`, so a new field on an existing model needs an
	/// explicit `ALTER`.
	pub async fn ensure_facet_columns(&self) -> Result<()> {
		for (model_name, model) in &self.schema.models {
			let table = facet_table(model_name);
			for (field_name, field_type) in &model.fields {
				let present: Option<(String,)> =
					sqlx::query_as("SELECT name FROM pragma_table_info(?) WHERE name = ?")
						.bind(&table)
						.bind(field_name)
						.fetch_optional(&self.pool)
						.await?;

				if present.is_none() {
					let sql = format!(
						"ALTER TABLE \"{table}\" ADD COLUMN \"{field_name}\" {}",
						field_type.sql_type()
					);
					sqlx::query(&sql).execute(&self.pool).await?;
					tracing::info!(table, column = field_name, "added facet column");
				}
			}
		}
		Ok(())
	}

	/// Resolve a record's spine uuid from its type and source-side key.
	async fn resolve_uuid(&self, type_: &str, external_id: &str) -> Result<String> {
		let row: Option<(String,)> =
			sqlx::query_as("SELECT uuid FROM record WHERE type = ? AND external_id = ?")
				.bind(type_)
				.bind(external_id)
				.fetch_optional(&self.pool)
				.await?;

		row.map(|r| r.0).ok_or_else(|| {
			Error::Other(format!(
				"record resolution failed: {type_} with external_id {external_id}"
			))
		})
	}

	/// Insert or update a record and its facet row.
	///
	/// `model` becomes the record's open `type`. A `belongs_to` target resolves
	/// to `parent_uuid`; a `self_referential` column and any further
	/// `belongs_to` targets become spine edges.
	pub async fn upsert(
		&self,
		model: &str,
		external_id: &str,
		fields: &serde_json::Value,
	) -> Result<String> {
		let model_def = self
			.schema
			.models
			.get(model)
			.ok_or_else(|| Error::Other(format!("unknown model: {model}")))?;

		let fields_map = fields
			.as_object()
			.ok_or_else(|| Error::Other("fields must be a JSON object".to_string()))?;

		// Identity is assigned once and preserved across re-index.
		let existing: Option<(String,)> =
			sqlx::query_as("SELECT uuid FROM record WHERE type = ? AND external_id = ?")
				.bind(model)
				.bind(external_id)
				.fetch_optional(&self.pool)
				.await?;
		let is_new = existing.is_none();
		let uuid = match existing {
			Some((u,)) => u,
			None => uuid::Uuid::now_v7().to_string(),
		};

		let mut parent_uuid = None;
		let mut edges: Vec<(String, String)> = Vec::new();

		for (position, target) in model_def.relations.belongs_to.iter().enumerate() {
			let fk_col = format!("{target}_id");
			let Some(value) = fields_map.get(&fk_col).and_then(|v| v.as_str()) else {
				continue;
			};
			let target_uuid = self.resolve_uuid(target, value).await?;

			if position == 0 {
				parent_uuid = Some(target_uuid);
			} else {
				edges.push((target_uuid, format!("belongs_to:{target}")));
			}
		}

		if let Some(ref col) = model_def.relations.self_referential {
			if let Some(value) = fields_map.get(col).and_then(|v| v.as_str()) {
				let target_uuid = self.resolve_uuid(model, value).await?;
				edges.push((target_uuid, col.clone()));
			}
		}

		let record = Record {
			uuid: uuid.clone(),
			external_id: external_id.to_string(),
			type_: model.to_string(),
			title: self.spine_title(model, model_def, fields_map),
			created_at: self.spine_created_at(model_def, fields_map),
			modified_at: self.spine_modified_at(model, model_def, fields_map),
			parent_uuid,
			content_id: None,
		};

		self.put_record(&record, self.scan_epoch()).await?;
		self.put_facet(model, model_def, &uuid, fields_map).await?;

		for (dst_uuid, edge_type) in edges {
			self.put_edge(&uuid, &dst_uuid, &edge_type, None).await?;
		}

		// A record already cleared for indexing keeps the index in step with its
		// new content. A newly discovered one starts out unscreened and enters
		// the index when screening clears it.
		if !is_new {
			self.refresh_search_index(&uuid).await?;
		}

		Ok(uuid)
	}

	/// Write the spine row, preserving the assigned uuid on conflict.
	async fn put_record(&self, record: &Record, epoch: i64) -> Result<()> {
		sqlx::query(
			"INSERT INTO record
				(uuid, external_id, type, title, created_at, modified_at,
				 parent_uuid, content_id, scan_epoch, indexed_at)
			 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, datetime('now'))
			 ON CONFLICT (type, external_id) DO UPDATE SET
				title = excluded.title,
				created_at = excluded.created_at,
				modified_at = excluded.modified_at,
				parent_uuid = excluded.parent_uuid,
				scan_epoch = excluded.scan_epoch,
				indexed_at = excluded.indexed_at",
		)
		.bind(&record.uuid)
		.bind(&record.external_id)
		.bind(&record.type_)
		.bind(&record.title)
		.bind(record.created_at)
		.bind(record.modified_at)
		.bind(&record.parent_uuid)
		.bind(record.content_id)
		.bind(epoch)
		.execute(&self.pool)
		.await?;
		Ok(())
	}

	/// Write the model's declared fields to its facet table.
	async fn put_facet(
		&self,
		model: &str,
		model_def: &crate::schema::ModelDef,
		uuid: &str,
		fields_map: &serde_json::Map<String, serde_json::Value>,
	) -> Result<()> {
		let table = facet_table(model);

		let mut columns = vec!["record_uuid".to_string()];
		let mut values: Vec<Option<String>> = vec![Some(uuid.to_string())];

		for field_name in model_def.fields.keys() {
			if let Some(value) = fields_map.get(field_name) {
				columns.push(format!("\"{field_name}\""));
				values.push(json_to_sql_value(value));
			}
		}

		let placeholders = vec!["?"; columns.len()].join(", ");
		let columns_str = columns.join(", ");

		let sql = if columns.len() == 1 {
			format!(
				"INSERT INTO \"{table}\" ({columns_str}) VALUES ({placeholders}) \
				 ON CONFLICT (record_uuid) DO NOTHING"
			)
		} else {
			let updates: Vec<String> = columns[1..]
				.iter()
				.map(|c| format!("{c} = excluded.{c}"))
				.collect();
			format!(
				"INSERT INTO \"{table}\" ({columns_str}) VALUES ({placeholders}) \
				 ON CONFLICT (record_uuid) DO UPDATE SET {}",
				updates.join(", ")
			)
		};

		let mut query = sqlx::query(&sql);
		for value in &values {
			query = query.bind(value);
		}
		query.execute(&self.pool).await?;

		Ok(())
	}

	/// Insert a spine edge, idempotent on `(src, dst, type)`.
	async fn put_edge(
		&self,
		src_uuid: &str,
		dst_uuid: &str,
		edge_type: &str,
		ord: Option<f64>,
	) -> Result<()> {
		sqlx::query(
			"INSERT INTO edge (src_uuid, dst_uuid, type, ord)
			 VALUES (?, ?, ?, ?)
			 ON CONFLICT (src_uuid, dst_uuid, type) DO UPDATE SET ord = excluded.ord",
		)
		.bind(src_uuid)
		.bind(dst_uuid)
		.bind(edge_type)
		.bind(ord)
		.execute(&self.pool)
		.await?;
		Ok(())
	}

	/// Display name for the spine row. The primary type takes it from the search
	/// contract; other types fall back to a conventional field name.
	fn spine_title(
		&self,
		model: &str,
		model_def: &crate::schema::ModelDef,
		fields_map: &serde_json::Map<String, serde_json::Value>,
	) -> Option<String> {
		if model == self.primary_type() {
			if let Some(value) = fields_map.get(&self.schema.search.title) {
				return value.as_str().map(str::to_string);
			}
		}

		["title", "name", "subject", "summary", "display_name"]
			.iter()
			.find(|candidate| model_def.fields.contains_key(**candidate))
			.and_then(|candidate| fields_map.get(*candidate))
			.and_then(|v| v.as_str())
			.map(str::to_string)
	}

	fn spine_created_at(
		&self,
		model_def: &crate::schema::ModelDef,
		fields_map: &serde_json::Map<String, serde_json::Value>,
	) -> Option<i64> {
		first_timestamp(
			&["created_at", "created", "added_at", "date", "timestamp"],
			model_def,
			fields_map,
		)
	}

	fn spine_modified_at(
		&self,
		model: &str,
		model_def: &crate::schema::ModelDef,
		fields_map: &serde_json::Map<String, serde_json::Value>,
	) -> Option<i64> {
		// The search contract's date field is what this type is sorted and
		// filtered by, so it is the authoritative modification time.
		if model == self.primary_type() {
			if let Some(field) = &self.schema.search.date_field {
				if let Some(ts) = fields_map.get(field).and_then(parse_timestamp) {
					return Some(ts);
				}
			}
		}

		first_timestamp(
			&["modified_at", "modified", "updated_at", "updated"],
			model_def,
			fields_map,
		)
		.or_else(|| self.spine_created_at(model_def, fields_map))
	}

	/// Delete a record. Facet rows and edges cascade.
	pub async fn delete(&self, model: &str, external_id: &str) -> Result<()> {
		let uuid = self.resolve_uuid(model, external_id).await.map_err(|_| {
			Error::Other(format!(
				"record not found: {model} with external_id {external_id}"
			))
		})?;

		// The search index only exists when the data type declares search fields.
		if !indexed_search_fields(&self.schema).is_empty() {
			sqlx::query("DELETE FROM search_index WHERE uuid = ?")
				.bind(&uuid)
				.execute(&self.pool)
				.await?;
		}

		sqlx::query("DELETE FROM record WHERE uuid = ?")
			.bind(&uuid)
			.execute(&self.pool)
			.await?;

		Ok(())
	}

	/// Create a relationship edge between two records.
	pub async fn link(
		&self,
		model_a: &str,
		ext_id_a: &str,
		model_b: &str,
		ext_id_b: &str,
	) -> Result<()> {
		let uuid_a = self.resolve_uuid(model_a, ext_id_a).await?;
		let uuid_b = self.resolve_uuid(model_b, ext_id_b).await?;
		self.put_edge(&uuid_a, &uuid_b, model_b, None).await
	}

	/// Remove a relationship edge.
	pub async fn unlink(
		&self,
		model_a: &str,
		ext_id_a: &str,
		model_b: &str,
		ext_id_b: &str,
	) -> Result<()> {
		let uuid_a = self.resolve_uuid(model_a, ext_id_a).await?;
		let uuid_b = self.resolve_uuid(model_b, ext_id_b).await?;

		sqlx::query("DELETE FROM edge WHERE src_uuid = ? AND dst_uuid = ? AND type = ?")
			.bind(&uuid_a)
			.bind(&uuid_b)
			.bind(model_b)
			.execute(&self.pool)
			.await?;
		Ok(())
	}

	/// Edges touching a record, with the neighbouring record resolved.
	pub async fn neighbors(&self, uuid: &str, edge_type: Option<&str>) -> Result<Vec<Neighbor>> {
		let mut sql = String::from(
			"SELECT r.uuid, r.external_id, r.type, r.title, e.type AS edge_type, e.outgoing
			 FROM (
				SELECT dst_uuid AS other, type, ord, 1 AS outgoing FROM edge WHERE src_uuid = ?1
				UNION ALL
				SELECT src_uuid AS other, type, ord, 0 AS outgoing FROM edge WHERE dst_uuid = ?1
			 ) e
			 JOIN record r ON r.uuid = e.other",
		);
		if edge_type.is_some() {
			sql.push_str(" WHERE e.type = ?2");
		}
		sql.push_str(" ORDER BY e.ord, r.title");

		let mut query = sqlx::query_as::<_, NeighborRow>(&sql).bind(uuid);
		if let Some(t) = edge_type {
			query = query.bind(t);
		}

		let rows = query.fetch_all(&self.pool).await?;
		Ok(rows.into_iter().map(NeighborRow::into_neighbor).collect())
	}

	/// Record the identity of a record's underlying bytes.
	pub async fn set_content_identity(
		&self,
		uuid: &str,
		identity: &ContentIdentity,
	) -> Result<i64> {
		let content_id: i64 = sqlx::query_scalar(
			"INSERT INTO content (sampled_hash, integrity_hash, size, kind)
			 VALUES (?, ?, ?, ?) RETURNING id",
		)
		.bind(&identity.sampled_hash)
		.bind(&identity.integrity_hash)
		.bind(identity.size)
		.bind(identity.kind)
		.fetch_one(&self.pool)
		.await?;

		sqlx::query("UPDATE record SET content_id = ? WHERE uuid = ?")
			.bind(content_id)
			.bind(uuid)
			.execute(&self.pool)
			.await?;

		Ok(content_id)
	}

	/// Records that carry a path-typed facet field but no content identity yet.
	pub async fn records_needing_content_identity(
		&self,
		batch_size: usize,
	) -> Result<Vec<(String, String)>> {
		let Some((model, field)) = self.path_field() else {
			return Ok(Vec::new());
		};

		let table = facet_table(&model);
		let sql = format!(
			"SELECT r.uuid, f.\"{field}\"
			 FROM record r JOIN \"{table}\" f ON f.record_uuid = r.uuid
			 WHERE r.content_id IS NULL AND f.\"{field}\" IS NOT NULL AND f.\"{field}\" <> ''
			 LIMIT ?"
		);

		Ok(sqlx::query_as::<_, (String, String)>(&sql)
			.bind(batch_size as i64)
			.fetch_all(&self.pool)
			.await?)
	}

	/// The first `path`-typed field declared by the schema, if any.
	fn path_field(&self) -> Option<(String, String)> {
		for (model_name, model) in &self.schema.models {
			for (field_name, field_type) in &model.fields {
				if *field_type == crate::schema::FieldType::Path {
					return Some((model_name.clone(), field_name.clone()));
				}
			}
		}
		None
	}

	/// Get a sync cursor value.
	pub async fn get_cursor(&self, key: &str) -> Result<Option<String>> {
		let row: Option<(String,)> = sqlx::query_as("SELECT value FROM _sync_state WHERE key = ?")
			.bind(key)
			.fetch_optional(&self.pool)
			.await?;
		Ok(row.map(|r| r.0))
	}

	/// Set a sync cursor value.
	pub async fn set_cursor(&self, key: &str, value: &str) -> Result<()> {
		sqlx::query(
			"INSERT INTO _sync_state (key, value, updated_at) VALUES (?, ?, datetime('now'))
			 ON CONFLICT (key) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at",
		)
		.bind(key)
		.bind(value)
		.execute(&self.pool)
		.await?;
		Ok(())
	}

	/// Count records of one type.
	pub async fn count(&self, model: &str) -> Result<i64> {
		let row: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM record WHERE type = ?")
			.bind(model)
			.fetch_one(&self.pool)
			.await?;
		Ok(row.0)
	}

	/// Total records across every type.
	pub async fn count_all(&self) -> Result<i64> {
		let row: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM record")
			.fetch_one(&self.pool)
			.await?;
		Ok(row.0)
	}

	/// The concatenated search text for a record, as one SQL expression.
	fn search_text_expr(&self, alias: &str) -> Option<String> {
		let fields = indexed_search_fields(&self.schema);
		if fields.is_empty() {
			return None;
		}
		Some(
			fields
				.iter()
				.map(|f| format!("COALESCE({alias}.\"{f}\", '')"))
				.collect::<Vec<_>>()
				.join(" || ' ' || "),
		)
	}

	/// Fetch records needing embedding.
	pub async fn records_needing_embedding(
		&self,
		batch_size: usize,
	) -> Result<Vec<EmbeddingRecord>> {
		let Some(concat_expr) = self.search_text_expr("f") else {
			return Ok(Vec::new());
		};

		let table = facet_table(self.primary_type());
		let sql = format!(
			"SELECT r.uuid, ({concat_expr}) AS content
			 FROM record r JOIN \"{table}\" f ON f.record_uuid = r.uuid
			 WHERE r.type = ?
			   AND (r._embedded_at IS NULL OR r._embedded_at < r.indexed_at)
			   AND r._safety_verdict IN ('safe', 'flagged')
			 LIMIT ?"
		);

		let rows = sqlx::query_as::<_, (String, String)>(&sql)
			.bind(self.primary_type())
			.bind(batch_size as i64)
			.fetch_all(&self.pool)
			.await?;

		Ok(rows
			.into_iter()
			.map(|(id, content)| EmbeddingRecord { id, content })
			.collect())
	}

	/// Mark records as embedded.
	pub async fn mark_embedded(&self, ids: &[String]) -> Result<()> {
		if ids.is_empty() {
			return Ok(());
		}

		let placeholders = vec!["?"; ids.len()].join(", ");
		let sql = format!(
			"UPDATE record SET _embedded_at = datetime('now') WHERE uuid IN ({placeholders})"
		);

		let mut query = sqlx::query(&sql);
		for id in ids {
			query = query.bind(id);
		}
		query.execute(&self.pool).await?;

		Ok(())
	}

	/// Fetch records needing safety screening.
	pub async fn records_needing_screening(
		&self,
		batch_size: usize,
	) -> Result<Vec<ScreeningRecord>> {
		let Some(concat_expr) = self.search_text_expr("f") else {
			return Ok(Vec::new());
		};

		let table = facet_table(self.primary_type());
		let sql = format!(
			"SELECT r.uuid, ({concat_expr}) AS content
			 FROM record r JOIN \"{table}\" f ON f.record_uuid = r.uuid
			 WHERE r.type = ? AND r._safety_verdict = 'unscreened'
			 LIMIT ?"
		);

		let rows = sqlx::query_as::<_, (String, String)>(&sql)
			.bind(self.primary_type())
			.bind(batch_size as i64)
			.fetch_all(&self.pool)
			.await?;

		Ok(rows
			.into_iter()
			.map(|(id, content)| ScreeningRecord { id, content })
			.collect())
	}

	/// Record a screening verdict and bring the search index into line with it.
	pub async fn mark_screened(
		&self,
		id: &str,
		score: u8,
		verdict: &str,
		version: &str,
	) -> Result<()> {
		sqlx::query(
			"UPDATE record
			 SET _safety_score = ?, _safety_verdict = ?, _safety_version = ?
			 WHERE uuid = ?",
		)
		.bind(score as i32)
		.bind(verdict)
		.bind(version)
		.bind(id)
		.execute(&self.pool)
		.await?;

		self.refresh_search_index(id).await
	}

	/// Bring a record's search-index row into line with its current verdict:
	/// present and current when cleared, absent otherwise.
	async fn refresh_search_index(&self, uuid: &str) -> Result<()> {
		let fields = indexed_search_fields(&self.schema);
		if fields.is_empty() {
			return Ok(());
		}

		sqlx::query("DELETE FROM search_index WHERE uuid = ?")
			.bind(uuid)
			.execute(&self.pool)
			.await?;

		let verdict: Option<(String, String)> =
			sqlx::query_as("SELECT type, _safety_verdict FROM record WHERE uuid = ?")
				.bind(uuid)
				.fetch_optional(&self.pool)
				.await?;

		let Some((type_, verdict)) = verdict else {
			return Ok(());
		};
		if type_ != self.primary_type() || !INDEXABLE_VERDICTS.contains(&verdict.as_str()) {
			return Ok(());
		}

		let table = facet_table(self.primary_type());
		let columns = fields
			.iter()
			.map(|f| format!("\"{f}\""))
			.collect::<Vec<_>>()
			.join(", ");
		let selected = fields
			.iter()
			.map(|f| format!("f.\"{f}\""))
			.collect::<Vec<_>>()
			.join(", ");

		let sql = format!(
			"INSERT INTO search_index ({columns}, uuid)
			 SELECT {selected}, f.record_uuid FROM \"{table}\" f WHERE f.record_uuid = ?"
		);

		sqlx::query(&sql).bind(uuid).execute(&self.pool).await?;
		Ok(())
	}

	/// Rebuild the whole search index from records currently cleared for it.
	pub async fn rebuild_search_index(&self) -> Result<u64> {
		let fields = indexed_search_fields(&self.schema);
		if fields.is_empty() {
			return Ok(0);
		}

		sqlx::query("DELETE FROM search_index")
			.execute(&self.pool)
			.await?;

		let table = facet_table(self.primary_type());
		let columns = fields
			.iter()
			.map(|f| format!("\"{f}\""))
			.collect::<Vec<_>>()
			.join(", ");
		let selected = fields
			.iter()
			.map(|f| format!("f.\"{f}\""))
			.collect::<Vec<_>>()
			.join(", ");

		let sql = format!(
			"INSERT INTO search_index ({columns}, uuid)
			 SELECT {selected}, r.uuid
			 FROM record r JOIN \"{table}\" f ON f.record_uuid = r.uuid
			 WHERE r.type = ? AND r._safety_verdict IN ('safe', 'flagged')"
		);

		let result = sqlx::query(&sql)
			.bind(self.primary_type())
			.execute(&self.pool)
			.await?;
		Ok(result.rows_affected())
	}

	/// The SELECT prefix shared by listing and search: spine identity plus the
	/// presentation columns named by the search contract.
	fn presentation_select(&self) -> String {
		let mut sql = String::from("SELECT r.uuid AS id, r.external_id AS external_id, ");
		let _ = write!(sql, "COALESCE(r.title, '') AS title, ");

		let preview = &self.schema.search.preview;
		if preview.starts_with("_derived.") {
			sql.push_str("NULL AS preview, ");
		} else {
			let _ = write!(sql, "f.\"{preview}\" AS preview, ");
		}

		match &self.schema.search.subtitle {
			Some(subtitle) => {
				let _ = write!(sql, "f.\"{subtitle}\" AS subtitle");
			}
			None => sql.push_str("NULL AS subtitle"),
		}

		sql
	}

	/// List items of the primary record type, newest first.
	/// Full record rows for the primary model — every facet field included —
	/// as JSON objects, newest first. Powers presentation surfaces that need
	/// more than the search projection (media grids, inspectors).
	pub async fn list_records_full(
		&self,
		limit: usize,
		offset: usize,
	) -> Result<Vec<serde_json::Value>> {
		let table = facet_table(self.primary_type());
		let mut pairs = String::from(
			"'id', r.uuid, 'external_id', r.external_id, 'title', r.title, \
			 'created_at', r.created_at, 'modified_at', r.modified_at",
		);
		if let Some(model) = self.schema.models.get(self.primary_type()) {
			for name in model.fields.keys() {
				let _ = write!(pairs, ", '{name}', f.\"{name}\"");
			}
		}
		let sql = format!(
			"SELECT json_object({pairs}) FROM record r \
			 LEFT JOIN \"{table}\" f ON f.record_uuid = r.uuid \
			 WHERE r.type = ? \
			 ORDER BY COALESCE(r.modified_at, r.created_at) DESC, r.rowid DESC \
			 LIMIT ? OFFSET ?"
		);
		let rows = sqlx::query_scalar::<_, String>(&sql)
			.bind(self.primary_type())
			.bind(limit as i64)
			.bind(offset as i64)
			.fetch_all(&self.pool)
			.await?;
		Ok(rows
			.into_iter()
			.filter_map(|s| serde_json::from_str(&s).ok())
			.collect())
	}

	pub async fn list_items(&self, limit: usize, offset: usize) -> Result<Vec<ItemRow>> {
		let table = facet_table(self.primary_type());
		let sql = format!(
			"{} FROM record r LEFT JOIN \"{table}\" f ON f.record_uuid = r.uuid
			 WHERE r.type = ?
			 ORDER BY COALESCE(r.modified_at, r.created_at) DESC, r.rowid DESC
			 LIMIT ? OFFSET ?",
			self.presentation_select()
		);

		let rows =
			sqlx::query_as::<_, (String, String, String, Option<String>, Option<String>)>(&sql)
				.bind(self.primary_type())
				.bind(limit as i64)
				.bind(offset as i64)
				.fetch_all(&self.pool)
				.await?;

		Ok(rows
			.into_iter()
			.map(|(id, external_id, title, preview, subtitle)| ItemRow {
				id,
				external_id,
				title,
				preview,
				subtitle,
			})
			.collect())
	}

	/// FTS5 search over the primary record type.
	pub async fn fts_search(
		&self,
		query: &str,
		limit: usize,
		temporal: Option<TemporalFilter<'_>>,
	) -> Result<Vec<FtsHit>> {
		let table = facet_table(self.primary_type());

		let mut sql = self.presentation_select();
		sql.push_str(", s.rank AS rank, ");

		match &self.schema.search.date_field {
			Some(date_field) => {
				let _ = write!(sql, "f.\"{date_field}\" AS date, ");
			}
			None => sql.push_str("NULL AS date, "),
		}

		sql.push_str("r._safety_verdict, r._safety_score ");

		let _ = write!(
			sql,
			"FROM search_index s
			 JOIN record r ON r.uuid = s.uuid
			 LEFT JOIN \"{table}\" f ON f.record_uuid = r.uuid
			 WHERE search_index MATCH ?"
		);

		let date_field = self.schema.search.date_field.as_ref();
		let temporal = temporal.filter(|_| date_field.is_some());

		if let (Some(temp), Some(field)) = (&temporal, date_field) {
			if temp.date_after.is_some() {
				let _ = write!(sql, " AND f.\"{field}\" >= ?");
			}
			if temp.date_before.is_some() {
				let _ = write!(sql, " AND f.\"{field}\" <= ?");
			}
		}

		sql.push_str(" ORDER BY s.rank LIMIT ?");

		let mut q = sqlx::query_as::<_, FtsHitRow>(&sql).bind(query);

		if let Some(temp) = &temporal {
			if let Some(after) = temp.date_after {
				q = q.bind(after);
			}
			if let Some(before) = temp.date_before {
				q = q.bind(before);
			}
		}

		let rows = q.bind(limit as i64).fetch_all(&self.pool).await?;

		Ok(rows.into_iter().map(Into::into).collect())
	}
}

/// Convert a JSON value to the string SQLite stores. JSON `null` becomes SQL
/// NULL rather than an empty string, so absent values stay distinguishable from
/// empty ones.
fn json_to_sql_value(value: &serde_json::Value) -> Option<String> {
	match value {
		serde_json::Value::Null => None,
		serde_json::Value::String(s) => Some(s.clone()),
		serde_json::Value::Number(n) => Some(n.to_string()),
		serde_json::Value::Bool(b) => Some(if *b { "1".into() } else { "0".into() }),
		other => Some(other.to_string()),
	}
}

/// First present candidate field parsed as a timestamp.
fn first_timestamp(
	candidates: &[&str],
	model_def: &crate::schema::ModelDef,
	fields_map: &serde_json::Map<String, serde_json::Value>,
) -> Option<i64> {
	candidates
		.iter()
		.filter(|c| model_def.fields.contains_key(**c))
		.find_map(|c| fields_map.get(*c).and_then(parse_timestamp))
}

/// Parse a source timestamp into unix milliseconds. Accepts RFC 3339, a few
/// common naive formats, and raw epoch numbers in seconds or milliseconds.
fn parse_timestamp(value: &serde_json::Value) -> Option<i64> {
	if let Some(n) = value.as_i64() {
		return Some(normalize_epoch(n));
	}

	let raw = value.as_str()?.trim();
	if raw.is_empty() {
		return None;
	}

	if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(raw) {
		return Some(dt.timestamp_millis());
	}

	for format in ["%Y-%m-%d %H:%M:%S%.f", "%Y-%m-%dT%H:%M:%S%.f"] {
		if let Ok(naive) = chrono::NaiveDateTime::parse_from_str(raw, format) {
			return Some(naive.and_utc().timestamp_millis());
		}
	}

	if let Ok(date) = chrono::NaiveDate::parse_from_str(raw, "%Y-%m-%d") {
		return date
			.and_hms_opt(0, 0, 0)
			.map(|d| d.and_utc().timestamp_millis());
	}

	raw.parse::<i64>().ok().map(normalize_epoch)
}

/// Epoch values below this are seconds rather than milliseconds — the boundary
/// sits far outside the range of dates any source realistically carries.
const EPOCH_MILLIS_THRESHOLD: i64 = 100_000_000_000;

fn normalize_epoch(value: i64) -> i64 {
	if value.abs() < EPOCH_MILLIS_THRESHOLD {
		value * 1000
	} else {
		value
	}
}

#[derive(sqlx::FromRow)]
struct FtsHitRow {
	id: String,
	external_id: String,
	title: String,
	preview: Option<String>,
	subtitle: Option<String>,
	rank: f64,
	date: Option<String>,
	_safety_verdict: Option<String>,
	_safety_score: Option<i32>,
}

impl From<FtsHitRow> for FtsHit {
	fn from(row: FtsHitRow) -> Self {
		Self {
			id: row.id,
			external_id: row.external_id,
			title: row.title,
			preview: row.preview,
			subtitle: row.subtitle,
			rank: row.rank,
			date: row.date,
			safety_verdict: row._safety_verdict,
			safety_score: row._safety_score.map(|s| s as u8),
		}
	}
}

#[derive(sqlx::FromRow)]
struct NeighborRow {
	uuid: String,
	external_id: String,
	r#type: String,
	title: Option<String>,
	edge_type: String,
	outgoing: i64,
}

impl NeighborRow {
	fn into_neighbor(self) -> Neighbor {
		Neighbor {
			uuid: self.uuid,
			external_id: self.external_id,
			type_: self.r#type,
			title: self.title,
			edge_type: self.edge_type,
			outgoing: self.outgoing != 0,
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn parses_rfc3339() {
		let value = serde_json::json!("2026-07-28T12:00:00Z");
		assert_eq!(parse_timestamp(&value), Some(1_785_240_000_000));
	}

	#[test]
	fn parses_naive_datetime() {
		let value = serde_json::json!("2026-07-28 12:00:00");
		assert_eq!(parse_timestamp(&value), Some(1_785_240_000_000));
	}

	#[test]
	fn parses_date_only() {
		let value = serde_json::json!("2026-07-28");
		assert_eq!(parse_timestamp(&value), Some(1_785_196_800_000));
	}

	#[test]
	fn promotes_epoch_seconds_to_millis() {
		assert_eq!(
			parse_timestamp(&serde_json::json!(1_785_240_000)),
			Some(1_785_240_000_000)
		);
		assert_eq!(
			parse_timestamp(&serde_json::json!(1_785_240_000_000i64)),
			Some(1_785_240_000_000)
		);
	}

	#[test]
	fn rejects_unparseable_values() {
		assert_eq!(parse_timestamp(&serde_json::json!("")), None);
		assert_eq!(parse_timestamp(&serde_json::json!("not a date")), None);
		assert_eq!(parse_timestamp(&serde_json::Value::Null), None);
	}

	#[test]
	fn null_fields_become_sql_null() {
		assert_eq!(json_to_sql_value(&serde_json::Value::Null), None);
		assert_eq!(
			json_to_sql_value(&serde_json::json!("text")),
			Some("text".to_string())
		);
		assert_eq!(
			json_to_sql_value(&serde_json::json!(true)),
			Some("1".to_string())
		);
	}
}
