//! SourceDb: handle for reading/writing records in a source database.

use std::fmt::Write;

use crate::error::{Error, Result};
use crate::schema::{DataTypeSchema, RelationsDef};

fn pluralize(name: &str) -> String {
	format!("{name}s")
}

/// Handle to a single source's SQLite database.
pub struct SourceDb {
	pool: sqlx::SqlitePool,
	schema: DataTypeSchema,
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

/// An item row from the primary model table.
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

/// Temporal filter for date range queries.
pub struct TemporalFilter<'a> {
	pub date_after: Option<&'a str>,
	pub date_before: Option<&'a str>,
}

impl SourceDb {
	/// Create a new SourceDb handle.
	pub(crate) fn new(pool: sqlx::SqlitePool, schema: DataTypeSchema) -> Self {
		Self { pool, schema }
	}

	/// Get the underlying connection pool.
	pub fn pool(&self) -> &sqlx::SqlitePool {
		&self.pool
	}

	/// Get the schema.
	pub fn schema(&self) -> &DataTypeSchema {
		&self.schema
	}

	/// Ensure system columns exist on all model tables.
	pub async fn ensure_system_columns(&self) -> Result<()> {
		let system_columns = [
			("_embedded_at", "TEXT"),
			("_safety_score", "INTEGER"),
			("_safety_verdict", "TEXT DEFAULT 'unscreened'"),
			("_safety_version", "TEXT"),
		];

		for model_name in self.schema.models.keys() {
			let table = pluralize(model_name);
			for (col_name, col_type) in &system_columns {
				let rows = sqlx::query_as::<_, (String,)>(&format!(
					"SELECT name FROM pragma_table_info(\"{table}\") WHERE name = ?"
				))
				.bind(col_name)
				.fetch_optional(&self.pool)
				.await?;

				if rows.is_none() {
					let sql =
						format!("ALTER TABLE \"{table}\" ADD COLUMN \"{col_name}\" {col_type}");
					sqlx::query(&sql).execute(&self.pool).await?;
					tracing::info!(table, column = col_name, "added system column");
				}
			}
		}

		Ok(())
	}

	/// Resolve an external_id to an internal UUID.
	async fn resolve_external_id(&self, table: &str, external_id: &str) -> Result<String> {
		let row: Option<(String,)> =
			sqlx::query_as(&format!("SELECT id FROM \"{table}\" WHERE external_id = ?"))
				.bind(external_id)
				.fetch_optional(&self.pool)
				.await?;

		row.map(|r| r.0).ok_or_else(|| {
			Error::Other(format!(
				"foreign key resolution failed: {table} with external_id {external_id}"
			))
		})
	}

	/// Insert or update a record by external ID.
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

		let table = pluralize(model);
		let id = uuid::Uuid::new_v4().to_string();

		let mut columns = vec!["\"id\"".to_string(), "\"external_id\"".to_string()];
		let mut placeholders = vec!["?".to_string(), "?".to_string()];
		let mut values: Vec<String> = vec![id.clone(), external_id.to_string()];

		let fields_map = fields
			.as_object()
			.ok_or_else(|| Error::Other("fields must be a JSON object".to_string()))?;

		for (field_name, _field_type) in &model_def.fields {
			if let Some(value) = fields_map.get(field_name) {
				columns.push(format!("\"{field_name}\""));
				placeholders.push("?".to_string());
				values.push(json_to_sql_string(value));
			}
		}

		for target in &model_def.relations.belongs_to {
			let fk_col = format!("{target}_id");
			if let Some(ext_id_value) = fields_map.get(&fk_col) {
				let ext_id = ext_id_value
					.as_str()
					.ok_or_else(|| Error::Other(format!("{fk_col} must be a string")))?;
				let target_table = pluralize(target);
				let internal_id = self.resolve_external_id(&target_table, ext_id).await?;
				columns.push(format!("\"{fk_col}\""));
				placeholders.push("?".to_string());
				values.push(internal_id);
			}
		}

		if let Some(ref col) = model_def.relations.self_referential {
			if let Some(ext_id_value) = fields_map.get(col) {
				if !ext_id_value.is_null() {
					let ext_id = ext_id_value
						.as_str()
						.ok_or_else(|| Error::Other(format!("{col} must be a string or null")))?;
					let internal_id = self.resolve_external_id(&table, ext_id).await?;
					columns.push(format!("\"{col}\""));
					placeholders.push("?".to_string());
					values.push(internal_id);
				}
			}
		}

		let columns_str = columns.join(", ");
		let placeholders_str = placeholders.join(", ");
		let update_cols: Vec<String> = columns[2..]
			.iter()
			.map(|c| format!("{c} = excluded.{c}"))
			.collect();

		let sql = if update_cols.is_empty() {
			format!(
				"INSERT INTO \"{table}\" ({columns_str}) VALUES ({placeholders_str}) \
				 ON CONFLICT (external_id) DO NOTHING"
			)
		} else {
			let mut all_updates = update_cols;
			all_updates.push("indexed_at = datetime('now')".to_string());
			let update_str = all_updates.join(", ");
			format!(
				"INSERT INTO \"{table}\" ({columns_str}) VALUES ({placeholders_str}) \
				 ON CONFLICT (external_id) DO UPDATE SET {update_str}"
			)
		};

		let mut query = sqlx::query(&sql);
		for v in &values {
			query = query.bind(v);
		}
		query.execute(&self.pool).await?;

		let row: (String,) =
			sqlx::query_as(&format!("SELECT id FROM \"{table}\" WHERE external_id = ?"))
				.bind(external_id)
				.fetch_one(&self.pool)
				.await?;

		Ok(row.0)
	}

	/// Delete a record by external ID.
	pub async fn delete(&self, model: &str, external_id: &str) -> Result<()> {
		let table = pluralize(model);
		let result = sqlx::query(&format!("DELETE FROM \"{table}\" WHERE external_id = ?"))
			.bind(external_id)
			.execute(&self.pool)
			.await?;

		if result.rows_affected() == 0 {
			return Err(Error::Other(format!(
				"record not found: {model} with external_id {external_id}"
			)));
		}
		Ok(())
	}

	/// Insert a many_to_many junction record.
	pub async fn link(
		&self,
		model_a: &str,
		ext_id_a: &str,
		model_b: &str,
		ext_id_b: &str,
	) -> Result<()> {
		let table_a = pluralize(model_a);
		let table_b = pluralize(model_b);
		let id_a = self.resolve_external_id(&table_a, ext_id_a).await?;
		let id_b = self.resolve_external_id(&table_b, ext_id_b).await?;

		let (a, b) = if model_a <= model_b {
			(model_a, model_b)
		} else {
			(model_b, model_a)
		};
		let junction = format!("{a}_{b}");

		let (col_a, col_b, val_a, val_b) = if model_a == model_b {
			(format!("{a}_id"), format!("related_{a}_id"), &id_a, &id_b)
		} else if model_a <= model_b {
			(format!("{a}_id"), format!("{b}_id"), &id_a, &id_b)
		} else {
			(format!("{a}_id"), format!("{b}_id"), &id_b, &id_a)
		};

		sqlx::query(&format!(
			"INSERT OR IGNORE INTO \"{junction}\" (\"{col_a}\", \"{col_b}\") VALUES (?, ?)"
		))
		.bind(val_a)
		.bind(val_b)
		.execute(&self.pool)
		.await?;

		Ok(())
	}

	/// Remove a many_to_many junction record.
	pub async fn unlink(
		&self,
		model_a: &str,
		ext_id_a: &str,
		model_b: &str,
		ext_id_b: &str,
	) -> Result<()> {
		let table_a = pluralize(model_a);
		let table_b = pluralize(model_b);
		let id_a = self.resolve_external_id(&table_a, ext_id_a).await?;
		let id_b = self.resolve_external_id(&table_b, ext_id_b).await?;

		let (a, b) = if model_a <= model_b {
			(model_a, model_b)
		} else {
			(model_b, model_a)
		};
		let junction = format!("{a}_{b}");

		let (col_a, col_b, val_a, val_b) = if model_a == model_b {
			(format!("{a}_id"), format!("related_{a}_id"), &id_a, &id_b)
		} else if model_a <= model_b {
			(format!("{a}_id"), format!("{b}_id"), &id_a, &id_b)
		} else {
			(format!("{a}_id"), format!("{b}_id"), &id_b, &id_a)
		};

		sqlx::query(&format!(
			"DELETE FROM \"{junction}\" WHERE \"{col_a}\" = ? AND \"{col_b}\" = ?"
		))
		.bind(val_a)
		.bind(val_b)
		.execute(&self.pool)
		.await?;

		Ok(())
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

	/// Count records in a model's table.
	pub async fn count(&self, model: &str) -> Result<i64> {
		let table = pluralize(model);
		let row: (i64,) = sqlx::query_as(&format!("SELECT COUNT(*) FROM \"{table}\""))
			.fetch_one(&self.pool)
			.await?;
		Ok(row.0)
	}

	/// Fetch records needing embedding.
	pub async fn records_needing_embedding(
		&self,
		batch_size: usize,
	) -> Result<Vec<EmbeddingRecord>> {
		let primary_table = pluralize(&self.schema.search.primary_model);

		let search_fields: Vec<&str> = self
			.schema
			.search
			.search_fields
			.iter()
			.filter(|f| !f.starts_with("_derived."))
			.map(|f| f.as_str())
			.collect();

		if search_fields.is_empty() {
			return Ok(Vec::new());
		}

		let field_exprs: Vec<String> = search_fields
			.iter()
			.map(|f| format!("COALESCE(\"{f}\", '')"))
			.collect();
		let concat_expr = field_exprs.join(" || ' ' || ");

		let sql = format!(
			"SELECT id, ({concat_expr}) AS content \
			 FROM \"{primary_table}\" \
			 WHERE (_embedded_at IS NULL OR _embedded_at < indexed_at) \
			 AND _safety_verdict IN ('safe', 'flagged') \
			 LIMIT ?"
		);

		let rows = sqlx::query_as::<_, (String, String)>(&sql)
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

		let primary_table = pluralize(&self.schema.search.primary_model);
		let placeholders: Vec<&str> = ids.iter().map(|_| "?").collect();
		let sql = format!(
			"UPDATE \"{primary_table}\" SET _embedded_at = datetime('now') WHERE id IN ({})",
			placeholders.join(", ")
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
		let primary_table = pluralize(&self.schema.search.primary_model);

		let search_fields: Vec<&str> = self
			.schema
			.search
			.search_fields
			.iter()
			.filter(|f| !f.starts_with("_derived."))
			.map(|f| f.as_str())
			.collect();

		if search_fields.is_empty() {
			return Ok(Vec::new());
		}

		let field_exprs: Vec<String> = search_fields
			.iter()
			.map(|f| format!("COALESCE(\"{f}\", '')"))
			.collect();
		let concat_expr = field_exprs.join(" || ' ' || ");

		let sql = format!(
			"SELECT id, ({concat_expr}) AS content \
			 FROM \"{primary_table}\" \
			 WHERE _safety_verdict = 'unscreened' OR _safety_verdict IS NULL \
			 LIMIT ?"
		);

		let rows = sqlx::query_as::<_, (String, String)>(&sql)
			.bind(batch_size as i64)
			.fetch_all(&self.pool)
			.await?;

		Ok(rows
			.into_iter()
			.map(|(id, content)| ScreeningRecord { id, content })
			.collect())
	}

	/// Mark records as screened.
	pub async fn mark_screened(
		&self,
		id: &str,
		score: u8,
		verdict: &str,
		version: &str,
	) -> Result<()> {
		let primary_table = pluralize(&self.schema.search.primary_model);
		let sql = format!(
			"UPDATE \"{primary_table}\" \
			 SET _safety_score = ?, _safety_verdict = ?, _safety_version = ? \
			 WHERE id = ?"
		);

		sqlx::query(&sql)
			.bind(score as i32)
			.bind(verdict)
			.bind(version)
			.bind(id)
			.execute(&self.pool)
			.await?;

		Ok(())
	}

	/// List items from the primary model table.
	pub async fn list_items(&self, limit: usize, offset: usize) -> Result<Vec<ItemRow>> {
		let primary_table = pluralize(&self.schema.search.primary_model);
		let title_col = &self.schema.search.title;
		let preview_col = &self.schema.search.preview;

		let mut sql = String::from("SELECT t.\"id\" AS id, t.\"external_id\" AS external_id, ");
		let _ = write!(sql, "t.\"{title_col}\" AS title, ");

		if preview_col.starts_with("_derived.") {
			sql.push_str("NULL AS preview, ");
		} else {
			let _ = write!(sql, "t.\"{preview_col}\" AS preview, ");
		}

		if let Some(ref subtitle_col) = self.schema.search.subtitle {
			let _ = write!(sql, "t.\"{subtitle_col}\" AS subtitle ");
		} else {
			sql.push_str("NULL AS subtitle ");
		}

		let _ = write!(
			sql,
			"FROM \"{primary_table}\" t ORDER BY t.rowid DESC LIMIT ? OFFSET ?"
		);

		let rows =
			sqlx::query_as::<_, (String, String, String, Option<String>, Option<String>)>(&sql)
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

	/// FTS5 search on the primary model.
	pub async fn fts_search(
		&self,
		query: &str,
		limit: usize,
		temporal: Option<TemporalFilter<'_>>,
	) -> Result<Vec<FtsHit>> {
		let primary_table = pluralize(&self.schema.search.primary_model);

		let mut sql = format!(
			"SELECT t.id, t.external_id, t.\"{}\" AS title, ",
			self.schema.search.title
		);

		if self.schema.search.preview.starts_with("_derived.") {
			sql.push_str("NULL AS preview, ");
		} else {
			sql.push_str(&format!(
				"t.\"{}\" AS preview, ",
				self.schema.search.preview
			));
		}

		if let Some(ref subtitle) = self.schema.search.subtitle {
			sql.push_str(&format!("t.\"{}\" AS subtitle, ", subtitle));
		} else {
			sql.push_str("NULL AS subtitle, ");
		}

		sql.push_str("rank, ");

		if let Some(ref date_field) = self.schema.search.date_field {
			sql.push_str(&format!("t.\"{}\" AS date, ", date_field));
		} else {
			sql.push_str("NULL AS date, ");
		}

		sql.push_str("t._safety_verdict, t._safety_score ");

		sql.push_str(&format!(
			"FROM \"{primary_table}\" t \
			 JOIN search_index s ON s.rowid = t.rowid \
			 WHERE search_index MATCH ?"
		));

		if let Some(ref temp) = temporal {
			if let Some(date_field) = &self.schema.search.date_field {
				if let Some(after) = temp.date_after {
					sql.push_str(&format!(" AND t.\"{}\" >= ?", date_field));
				}
				if let Some(before) = temp.date_before {
					sql.push_str(&format!(" AND t.\"{}\" <= ?", date_field));
				}
			}
		}

		sql.push_str(" ORDER BY rank LIMIT ?");

		let mut q = sqlx::query_as::<_, FtsHitRow>(&sql).bind(query);

		if let Some(ref temp) = temporal {
			if let Some(date_field) = &self.schema.search.date_field {
				if temp.date_after.is_some() {
					q = q.bind(temp.date_after.unwrap());
				}
				if temp.date_before.is_some() {
					q = q.bind(temp.date_before.unwrap());
				}
			}
		}

		let rows = q.bind(limit as i64).fetch_all(&self.pool).await?;

		Ok(rows.into_iter().map(|r| r.into()).collect())
	}
}

/// Convert JSON value to SQL string.
fn json_to_sql_string(value: &serde_json::Value) -> String {
	match value {
		serde_json::Value::String(s) => s.clone(),
		serde_json::Value::Number(n) => n.to_string(),
		serde_json::Value::Bool(b) => b.to_string(),
		serde_json::Value::Null => String::new(),
		other => other.to_string(),
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
