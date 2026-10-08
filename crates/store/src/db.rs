//! SourceDb: handle for reading/writing records in a source index.
//!
//! Every write lands on the record table ([`crate::record`]) first: one
//! `record` row carrying identity, hierarchy, timestamps and screening state.
//! The model's own declared fields go to its facet table, and relationships
//! become `edge` rows. Nothing is stored twice.

use std::collections::HashMap;
use std::fmt::Write;

use uuid::Uuid;

use crate::content::ContentId;
use crate::error::{Error, Result};
use crate::record::{facet_table, ContentIdentity, Record};
use crate::schema::codegen::indexed_search_fields;
use crate::schema::{DataTypeSchema, FieldType};

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
	/// The shape this handle reads. A writer is always at
	/// [`crate::migrate::SCHEMA_VERSION`]; a replica keeps its owner's.
	schema_version: i64,
}

/// An item row from the primary record type.
#[derive(Debug, Clone)]
pub struct ItemRow {
	pub id: Uuid,
	pub external_id: String,
	pub title: String,
	pub preview: Option<String>,
	pub subtitle: Option<String>,
}

/// An FTS search hit.
#[derive(Debug, Clone)]
pub struct FtsHit {
	pub id: Uuid,
	pub external_id: String,
	pub title: String,
	pub preview: Option<String>,
	pub subtitle: Option<String>,
	pub rank: f64,
	pub date: Option<String>,
}

/// A record edge with the neighbouring record resolved.
#[derive(Debug, Clone)]
pub struct Neighbor {
	pub uuid: Uuid,
	pub external_id: String,
	pub type_: String,
	pub title: Option<String>,
	pub edge_type: String,
	pub outgoing: bool,
}

/// What an assertion row rebinds from when this store is read somewhere the
/// record uuid means nothing: on another device, or after an index rebuild
/// minted fresh uuids. Neither key works alone, so the row carries both.
#[derive(Debug, Clone)]
pub struct OverlayEvidence {
	/// The record's open type key, half of the source's own key.
	pub type_: String,
	/// The other half. Portable, and rewritten when a file moves.
	pub external_id: String,
	/// The convergent content uuid, once hashing has reached the bytes.
	/// Computable offline by any machine that holds the same bytes.
	pub content_uuid: Option<Uuid>,
}

/// Ordering for a merge. Wall clocks disagree across devices, so an
/// assertion carries the HLC that ordered it and the device that wrote it.
#[derive(Debug, Clone)]
pub struct Stamp {
	pub hlc: String,
	pub device_uuid: Uuid,
}

/// Temporal filter for date range queries.
pub struct TemporalFilter<'a> {
	pub date_after: Option<&'a str>,
	pub date_before: Option<&'a str>,
}

impl SourceDb {
	/// Create a new SourceDb handle.
	pub(crate) fn new(
		pool: sqlx::SqlitePool,
		schema: DataTypeSchema,
		scan_epoch: i64,
		schema_version: i64,
	) -> Self {
		Self {
			pool,
			schema,
			scan_epoch: std::sync::atomic::AtomicI64::new(scan_epoch),
			schema_version,
		}
	}

	/// The schema version this handle reads; see [`crate::migrate`].
	pub fn schema_version(&self) -> i64 {
		self.schema_version
	}

	/// Get the underlying connection pool.
	pub fn pool(&self) -> &sqlx::SqlitePool {
		&self.pool
	}

	/// Get the schema.
	pub fn schema(&self) -> &DataTypeSchema {
		&self.schema
	}

	/// The store's revision; see [`crate::revision`].
	pub async fn revision(&self) -> Result<crate::revision::Revision> {
		crate::revision::read(&self.pool).await
	}

	/// Open a new sync run: advance the scan epoch that subsequent writes carry.
	pub async fn begin_sync(&self) -> Result<i64> {
		let epoch = crate::record::next_scan_epoch(&self.pool).await?;
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

	/// Resolve a record's record uuid from its type and source-side key.
	async fn resolve_uuid(&self, type_: &str, external_id: &str) -> Result<Uuid> {
		let row: Option<(Uuid,)> =
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
	/// `belongs_to` targets become record edges.
	pub async fn upsert(
		&self,
		model: &str,
		external_id: &str,
		fields: &serde_json::Value,
	) -> Result<Uuid> {
		let model_def = self
			.schema
			.models
			.get(model)
			.ok_or_else(|| Error::Other(format!("unknown model: {model}")))?;

		let fields_map = fields
			.as_object()
			.ok_or_else(|| Error::Other("fields must be a JSON object".to_string()))?;

		// Identity is assigned once and preserved across re-index.
		let existing: Option<(Uuid,)> =
			sqlx::query_as("SELECT uuid FROM record WHERE type = ? AND external_id = ?")
				.bind(model)
				.bind(external_id)
				.fetch_optional(&self.pool)
				.await?;
		let uuid = match existing {
			Some((u,)) => u,
			None => Uuid::now_v7(),
		};

		let mut parent_uuid = None;
		let mut edges: Vec<(Uuid, String)> = Vec::new();

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
			uuid,
			external_id: Some(external_id.to_string()),
			type_: model.to_string(),
			title: self.record_title(model, model_def, fields_map),
			created_at: self.record_created_at(model_def, fields_map),
			modified_at: self.record_modified_at(model, model_def, fields_map),
			parent_uuid,
			content_id: None,
		};

		self.put_record(&record, self.scan_epoch()).await?;
		self.put_facet(model, model_def, uuid, fields_map).await?;

		for (dst_uuid, edge_type) in edges {
			self.put_edge(uuid, dst_uuid, &edge_type, None).await?;
		}

		// The facet row is written by this point, so the index row can be built
		// from it.
		self.refresh_search_index(uuid).await?;

		Ok(uuid)
	}

	/// Write the record table row, preserving the assigned uuid on conflict.
	async fn put_record(&self, record: &Record, epoch: i64) -> Result<()> {
		crate::record::insert_record_query(record, epoch)
			.execute(&self.pool)
			.await?;
		Ok(())
	}

	/// Write the model's declared fields to its facet table.
	async fn put_facet(
		&self,
		model: &str,
		model_def: &crate::schema::ModelDef,
		uuid: Uuid,
		fields_map: &serde_json::Map<String, serde_json::Value>,
	) -> Result<()> {
		let table = facet_table(model);

		let mut columns = vec!["record_uuid".to_string()];
		let mut values: Vec<Option<String>> = Vec::new();

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

		let mut query = sqlx::query(&sql).bind(uuid);
		for value in &values {
			query = query.bind(value);
		}
		query.execute(&self.pool).await?;

		Ok(())
	}

	/// Insert a record edge, idempotent on `(src, dst, type)`.
	async fn put_edge(
		&self,
		src_uuid: Uuid,
		dst_uuid: Uuid,
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

	/// Display name for the record table row. The primary type takes it from the search
	/// contract; other types fall back to a conventional field name.
	fn record_title(
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

	fn record_created_at(
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

	fn record_modified_at(
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
		.or_else(|| self.record_created_at(model_def, fields_map))
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
				.bind(uuid)
				.execute(&self.pool)
				.await?;
		}

		sqlx::query("DELETE FROM record WHERE uuid = ?")
			.bind(uuid)
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
		self.put_edge(uuid_a, uuid_b, model_b, None).await
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
			.bind(uuid_a)
			.bind(uuid_b)
			.bind(model_b)
			.execute(&self.pool)
			.await?;
		Ok(())
	}

	/// Edges touching a record, with the neighbouring record resolved.
	pub async fn neighbors(&self, uuid: Uuid, edge_type: Option<&str>) -> Result<Vec<Neighbor>> {
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
	///
	/// One set of bytes is one row: copies of a file inside one source share a
	/// content row rather than each minting their own. Which row depends on
	/// how far the hash ladder has reached for this record: a sampled hash
	/// lands on the candidate row for that hash, an integrity hash on the
	/// confirmed row for that hash. The record's `content_id` moves when it
	/// climbs, and the row's uuid names the strongest hash the row holds.
	pub async fn set_content_identity(
		&self,
		uuid: Uuid,
		identity: &ContentIdentity,
	) -> Result<i64> {
		let mut conn = self.pool.acquire().await?;
		bind_content(&mut conn, uuid, identity).await
	}

	/// The same write for a batch, in one transaction.
	///
	/// What the hashing job produces: a few hundred files at a time, each one
	/// an insert and an update. Committing per file would spend the whole
	/// budget on fsync.
	///
	/// A single file that cannot be written does not take the batch with it.
	/// The record simply keeps its empty `content_id` and the next pass picks
	/// it up again, which is the same state it was already in.
	pub async fn set_content_identities(&self, batch: &[(Uuid, ContentIdentity)]) -> Result<usize> {
		if batch.is_empty() {
			return Ok(0);
		}

		let mut tx = self.pool.begin().await?;
		let mut written = 0;
		for (uuid, identity) in batch {
			match bind_content(&mut tx, *uuid, identity).await {
				Ok(_) => written += 1,
				Err(error) => {
					tracing::warn!(%uuid, %error, "could not record content identity")
				}
			}
		}
		tx.commit().await?;

		Ok(written)
	}

	/// Records that carry a path-typed facet field but no content identity yet.
	pub async fn records_needing_content_identity(
		&self,
		batch_size: usize,
	) -> Result<Vec<(Uuid, String)>> {
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

		Ok(sqlx::query_as::<_, (Uuid, String)>(&sql)
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

	/// Bring a record's search-index row into line with its facet row.
	/// Only the primary type is searchable.
	async fn refresh_search_index(&self, uuid: Uuid) -> Result<()> {
		let fields = indexed_search_fields(&self.schema);
		if fields.is_empty() {
			return Ok(());
		}

		sqlx::query("DELETE FROM search_index WHERE uuid = ?")
			.bind(uuid)
			.execute(&self.pool)
			.await?;

		let type_: Option<(String,)> = sqlx::query_as("SELECT type FROM record WHERE uuid = ?")
			.bind(uuid)
			.fetch_optional(&self.pool)
			.await?;

		let Some((type_,)) = type_ else {
			return Ok(());
		};
		if type_ != self.primary_type() {
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

	/// Merge fields onto a record's assertions. A field set to JSON `null` is
	/// removed; the rest are shallow-merged over what is already stored.
	///
	/// `evidence` is what a copy of this store rebinds from on another device,
	/// where this record uuid means nothing: the source-relative key always, and
	/// the content uuid once hashing has reached the bytes. `stamp` is what
	/// decides the winner when two devices have both written.
	pub async fn set_overlay(
		&self,
		record_uuid: Uuid,
		evidence: &OverlayEvidence,
		stamp: &Stamp,
		fields: &serde_json::Value,
	) -> Result<serde_json::Value> {
		let incoming = fields
			.as_object()
			.ok_or_else(|| Error::Other("overlay fields must be a JSON object".to_string()))?;

		let mut merged = match self.get_overlay(record_uuid).await? {
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
			"INSERT INTO record_overlay
				 (record_uuid, type, external_id, content_uuid, fields, hlc, device_uuid, updated_at)
				 VALUES (?, ?, ?, ?, ?, ?, ?, datetime('now'))
				 ON CONFLICT (record_uuid) DO UPDATE SET
					type = excluded.type,
					external_id = excluded.external_id,
					content_uuid = COALESCE(excluded.content_uuid, record_overlay.content_uuid),
					fields = excluded.fields,
					hlc = excluded.hlc,
					device_uuid = excluded.device_uuid,
					updated_at = excluded.updated_at",
		)
		.bind(record_uuid)
		.bind(&evidence.type_)
		.bind(&evidence.external_id)
		.bind(evidence.content_uuid)
		.bind(&encoded)
		.bind(&stamp.hlc)
		.bind(stamp.device_uuid)
		.execute(&self.pool)
		.await?;

		Ok(serde_json::Value::Object(merged))
	}

	/// Read one record's assertions. Returns an empty object when none are
	/// stored.
	pub async fn get_overlay(&self, record_uuid: Uuid) -> Result<serde_json::Value> {
		let row: Option<(String,)> =
			sqlx::query_as("SELECT fields FROM record_overlay WHERE record_uuid = ?")
				.bind(record_uuid)
				.fetch_optional(&self.pool)
				.await?;

		Ok(row
			.map(|(fields,)| decode_overlay(&fields))
			.unwrap_or_else(empty_object))
	}

	/// Read assertions for many records in one query. Records without any are
	/// absent from the map.
	pub async fn overlays_for(
		&self,
		record_uuids: &[Uuid],
	) -> Result<HashMap<Uuid, serde_json::Value>> {
		if record_uuids.is_empty() {
			return Ok(HashMap::new());
		}

		let placeholders = vec!["?"; record_uuids.len()].join(", ");
		let sql = format!(
			"SELECT record_uuid, fields FROM record_overlay
				 WHERE record_uuid IN ({placeholders})"
		);

		let mut query = sqlx::query_as::<_, (Uuid, String)>(&sql);
		for id in record_uuids {
			query = query.bind(*id);
		}

		Ok(query
			.fetch_all(&self.pool)
			.await?
			.into_iter()
			.map(|(record_uuid, fields)| (record_uuid, decode_overlay(&fields)))
			.collect())
	}

	/// The record at a source-relative path. See [`crate::read::resolve_path`].
	pub async fn resolve_path(&self, path: &str) -> Result<Option<Uuid>> {
		crate::read::resolve_path(&self.pool, path).await
	}

	/// One entry by record uuid. See [`crate::read::entry_by_uuid`].
	pub async fn entry_by_uuid(&self, uuid: Uuid) -> Result<Option<crate::read::FsEntry>> {
		crate::read::entry_by_uuid(&self.pool, uuid).await
	}

	/// Bind orphaned assertions back onto records, matching on the evidence each
	/// row carries. Content uuid first, since it is derived from the bytes and so
	/// holds across a rename and across a machine; the source's own key second.
	///
	/// Wanted whenever record uuids have been re-minted under standing
	/// assertions: after the generation is dropped and rebuilt, and when a store
	/// copied from another device is opened here. Returns the number of rows
	/// that found a home.
	pub async fn rebind_overlays(&self) -> Result<u64> {
		let orphans: Vec<(Uuid, String, String, Option<Uuid>)> = sqlx::query_as(
			"SELECT o.record_uuid, o.type, o.external_id, o.content_uuid
				 FROM record_overlay o
				 WHERE NOT EXISTS (SELECT 1 FROM record r WHERE r.uuid = o.record_uuid)",
		)
		.fetch_all(&self.pool)
		.await?;

		let mut rebound = 0;
		for (stale, type_, external_id, content_uuid) in orphans {
			let mut target: Option<(Uuid,)> = match content_uuid {
				Some(content) => {
					sqlx::query_as(
						"SELECT r.uuid FROM record r JOIN content c ON c.id = r.content_id
							 WHERE c.uuid = ? OR c.candidate_uuid = ? LIMIT 1",
					)
					.bind(content)
					.bind(content)
					.fetch_optional(&self.pool)
					.await?
				}
				None => None,
			};

			if target.is_none() {
				target =
					sqlx::query_as("SELECT uuid FROM record WHERE type = ? AND external_id = ?")
						.bind(&type_)
						.bind(&external_id)
						.fetch_optional(&self.pool)
						.await?;
			}

			// A filesystem file stores no key of its own, so the same evidence
			// has to be spent walking a path to it instead.
			if target.is_none() {
				target = self.resolve_path(&external_id).await?.map(|uuid| (uuid,));
			}

			let Some((target,)) = target else { continue };

			// An assertion already sitting on the target keeps it. That one was
			// written against a record that exists; the orphan was not.
			let taken: Option<(Uuid,)> =
				sqlx::query_as("SELECT record_uuid FROM record_overlay WHERE record_uuid = ?")
					.bind(target)
					.fetch_optional(&self.pool)
					.await?;
			if taken.is_some() {
				continue;
			}

			sqlx::query("UPDATE record_overlay SET record_uuid = ? WHERE record_uuid = ?")
				.bind(target)
				.bind(stale)
				.execute(&self.pool)
				.await?;
			rebound += 1;
		}

		Ok(rebound)
	}

	/// Rebuild the whole search index from the primary type's records.
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
			 WHERE r.type = ?"
		);

		let result = sqlx::query(&sql)
			.bind(self.primary_type())
			.execute(&self.pool)
			.await?;
		Ok(result.rows_affected())
	}

	/// The SELECT prefix shared by listing and search: record identity plus the
	/// presentation columns named by the search contract.
	fn presentation_select(&self) -> String {
		// A filesystem file has no external id; it is addressed by its parent
		// and its name, and search over such a source goes through the arena.
		let mut sql =
			String::from("SELECT r.uuid AS id, COALESCE(r.external_id, '') AS external_id, ");
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

	/// The declared fields of one model's records as JSON objects, typed back
	/// the way they went in: a `json` field is parsed, a `boolean` is a bool,
	/// numbers are numbers. `external_id` narrows to one record. Newest first.
	///
	/// This is how an extension reads its own models back; the facet row is
	/// the model, so nothing outside the declared fields comes out.
	pub async fn facet_rows(
		&self,
		model: &str,
		external_id: Option<&str>,
		limit: usize,
	) -> Result<Vec<serde_json::Value>> {
		let model_def = self
			.schema
			.models
			.get(model)
			.ok_or_else(|| Error::Other(format!("unknown model: {model}")))?;
		let table = facet_table(model);
		let mut pairs = String::new();
		for name in model_def.fields.keys() {
			if !pairs.is_empty() {
				pairs.push_str(", ");
			}
			let _ = write!(pairs, "'{name}', f.\"{name}\"");
		}
		let pairs = if pairs.is_empty() {
			String::new()
		} else {
			format!("json_object({pairs})")
		};
		let sql = format!(
			"SELECT r.uuid, {pairs} FROM record r \
			 LEFT JOIN \"{table}\" f ON f.record_uuid = r.uuid \
			 WHERE r.type = ?1 AND (?2 IS NULL OR r.external_id = ?2) \
			 ORDER BY COALESCE(r.modified_at, r.created_at) DESC, r.rowid DESC \
			 LIMIT ?3"
		);
		let rows = sqlx::query_as::<_, (Uuid, Option<String>)>(&sql)
			.bind(model)
			.bind(external_id)
			.bind(limit as i64)
			.fetch_all(&self.pool)
			.await?;

		Ok(rows
			.into_iter()
			.filter_map(|(_, json)| {
				let mut value: serde_json::Value = serde_json::from_str(&json?).ok()?;
				let object = value.as_object_mut()?;
				for (name, field_type) in &model_def.fields {
					let Some(stored) = object.get_mut(name) else {
						continue;
					};
					match field_type {
						FieldType::Json => {
							if let Some(text) = stored.as_str() {
								*stored =
									serde_json::from_str(text).unwrap_or(serde_json::Value::Null);
							}
						}
						FieldType::Boolean => {
							if let Some(n) = stored.as_i64() {
								*stored = serde_json::Value::Bool(n != 0);
							}
						}
						_ => {}
					}
				}
				Some(value)
			})
			.collect())
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
			"'external_id', COALESCE(r.external_id, ''), 'title', r.title, \
			 'created_at', r.created_at, 'modified_at', r.modified_at",
		);
		if let Some(model) = self.schema.models.get(self.primary_type()) {
			for name in model.fields.keys() {
				let _ = write!(pairs, ", '{name}', f.\"{name}\"");
			}
		}
		let sql = format!(
			"SELECT r.uuid, json_object({pairs}) FROM record r \
			 LEFT JOIN \"{table}\" f ON f.record_uuid = r.uuid \
			 WHERE r.type = ? \
			 ORDER BY COALESCE(r.modified_at, r.created_at) DESC, r.rowid DESC \
			 LIMIT ? OFFSET ?"
		);
		let rows = sqlx::query_as::<_, (Uuid, String)>(&sql)
			.bind(self.primary_type())
			.bind(limit as i64)
			.bind(offset as i64)
			.fetch_all(&self.pool)
			.await?;

		Ok(rows
			.into_iter()
			.filter_map(|(uuid, json)| {
				let mut value: serde_json::Value = serde_json::from_str(&json).ok()?;
				let object = value.as_object_mut()?;
				object.insert("id".to_string(), uuid.to_string().into());
				Some(value)
			})
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
			sqlx::query_as::<_, (Uuid, String, String, Option<String>, Option<String>)>(&sql)
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
				let _ = write!(sql, "f.\"{date_field}\" AS date ");
			}
			None => sql.push_str("NULL AS date "),
		}

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

fn empty_object() -> serde_json::Value {
	serde_json::Value::Object(serde_json::Map::new())
}

/// A stored overlay that no longer parses is treated as absent rather than
/// failing the read it was attached to.
fn decode_overlay(fields: &str) -> serde_json::Value {
	serde_json::from_str(fields).unwrap_or_else(|_| empty_object())
}

#[derive(sqlx::FromRow)]
struct FtsHitRow {
	id: Uuid,
	external_id: String,
	title: String,
	preview: Option<String>,
	subtitle: Option<String>,
	rank: f64,
	date: Option<String>,
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
		}
	}
}

#[derive(sqlx::FromRow)]
struct NeighborRow {
	uuid: Uuid,
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

/// The row a record pointed at before a content write, for what the write
/// has to carry across.
#[derive(sqlx::FromRow)]
struct PreviousContent {
	id: i64,
	uuid: Uuid,
	sampled_hash: Option<String>,
	integrity_hash: Option<String>,
	kind: Option<i64>,
	kind_name: Option<String>,
}

/// The content write itself, against whatever connection the caller holds: a
/// pooled one for a single file, a transaction for a batch.
///
/// With an integrity hash, the record binds to the confirmed row for that
/// hash, created if this is the first file read in full to produce it. With a
/// sampled hash alone, it binds to the candidate row for that hash and never
/// touches a confirmed row, so a confirmed row only ever holds records whose
/// own bytes were read. The candidate row keeps the size and kind it learns;
/// a confirmed row keeps its sampled hash so candidate lookups still reach it.
///
/// When the record leaves a candidate row for a confirmed one, assertions
/// anchored on it and keyed by the candidate uuid take the confirmed uuid, and
/// a candidate row no record points at any more is dropped.
async fn bind_content(
	conn: &mut sqlx::SqliteConnection,
	uuid: Uuid,
	identity: &ContentIdentity,
) -> Result<i64> {
	let content_uuid = ContentId::from_hashes(
		identity.sampled_hash.as_deref(),
		identity.integrity_hash.as_deref(),
	)
	.map(|id| id.uuid())
	.ok_or_else(|| Error::Other("content identity carries no hash".to_string()))?;
	let candidate_uuid = identity
		.sampled_hash
		.as_deref()
		.map(crate::content::uuid_for);

	let previous: Option<PreviousContent> = sqlx::query_as(
		"SELECT c.id, c.uuid, c.sampled_hash, c.integrity_hash, c.kind, c.kind_name
		 FROM record r JOIN content c ON c.id = r.content_id WHERE r.uuid = ?",
	)
	.bind(uuid)
	.fetch_optional(&mut *conn)
	.await?;

	// A sampled-only write says nothing against a full read of the same
	// bytes, so a record already confirmed under this sampled hash stays
	// where it is rather than walking back down to a guess.
	if let Some(old) = &previous {
		if old.integrity_hash.is_some()
			&& identity.integrity_hash.is_none()
			&& old.sampled_hash == identity.sampled_hash
		{
			return Ok(old.id);
		}
	}

	// The kind travels with the record: a verification pass carries no kind
	// of its own, and the confirmed row it moves the record to must not lose
	// the one the identity phase read from the bytes.
	let kind = identity
		.kind
		.or_else(|| previous.as_ref().and_then(|old| old.kind));
	let kind_name = identity
		.kind_name
		.clone()
		.or_else(|| previous.as_ref().and_then(|old| old.kind_name.clone()));

	let content_id: i64 = match identity.integrity_hash.as_deref() {
		Some(integrity) => sqlx::query_scalar(
			"INSERT INTO content (uuid, candidate_uuid, sampled_hash, integrity_hash, size, kind, kind_name)
					 VALUES (?, ?, ?, ?, ?, ?, ?)
					 ON CONFLICT (integrity_hash) DO UPDATE SET
						candidate_uuid = COALESCE(content.candidate_uuid, excluded.candidate_uuid),
						sampled_hash = COALESCE(content.sampled_hash, excluded.sampled_hash),
						size = COALESCE(excluded.size, content.size),
						kind = COALESCE(excluded.kind, content.kind),
						kind_name = COALESCE(excluded.kind_name, content.kind_name)
					 RETURNING id",
		)
		.bind(content_uuid)
		.bind(candidate_uuid)
		.bind(&identity.sampled_hash)
		.bind(integrity)
		.bind(identity.size)
		.bind(kind)
		.bind(&kind_name)
		.fetch_one(&mut *conn)
		.await?,
		None => sqlx::query_scalar(
			"INSERT INTO content (uuid, candidate_uuid, sampled_hash, integrity_hash, size, kind, kind_name)
					 VALUES (?, ?, ?, NULL, ?, ?, ?)
					 ON CONFLICT (sampled_hash) WHERE integrity_hash IS NULL DO UPDATE SET
						size = COALESCE(excluded.size, content.size),
						kind = COALESCE(excluded.kind, content.kind),
						kind_name = COALESCE(excluded.kind_name, content.kind_name)
					 RETURNING id",
		)
		.bind(content_uuid)
		.bind(candidate_uuid)
		.bind(&identity.sampled_hash)
		.bind(identity.size)
		.bind(kind)
		.bind(&kind_name)
		.fetch_one(&mut *conn)
		.await?,
	};

	sqlx::query("UPDATE record SET content_id = ? WHERE uuid = ?")
		.bind(content_id)
		.bind(uuid)
		.execute(&mut *conn)
		.await?;

	let Some(PreviousContent {
		id: old_id,
		uuid: old_uuid,
		integrity_hash: old_integrity,
		..
	}) = previous
	else {
		return Ok(content_id);
	};
	if old_id == content_id {
		return Ok(content_id);
	}

	if old_integrity.is_none() && identity.integrity_hash.is_some() {
		for table in ["tag_assertion", "record_overlay"] {
			sqlx::query(&format!(
				"UPDATE {table} SET content_uuid = ? WHERE record_uuid = ? AND content_uuid = ?"
			))
			.bind(content_uuid)
			.bind(uuid)
			.bind(old_uuid)
			.execute(&mut *conn)
			.await?;
		}
	}

	sqlx::query(
		"DELETE FROM content WHERE id = ? AND integrity_hash IS NULL
		 AND NOT EXISTS (SELECT 1 FROM record WHERE content_id = ?)",
	)
	.bind(old_id)
	.bind(old_id)
	.execute(&mut *conn)
	.await?;

	Ok(content_id)
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
