//! SourceManager: manages source folders and their disposable indexes.

use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::time::Duration;

use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePool, SqlitePoolOptions};

use crate::db::SourceDb;
use crate::error::{Error, Result};
use crate::schema::codegen::generate_ddl;
use crate::schema::migration::{diff_schemas, MigrationResult};
use crate::schema::DataTypeSchema;
use crate::record::RECORD_SCHEMA;

/// Manages source folders on disk.
pub struct SourceManager {
	sources_dir: PathBuf,
}

/// Open a source index pool.
///
/// Foreign keys are enabled per connection — SQLite defaults them off, and the
/// record table relies on `ON DELETE CASCADE` to take facet rows and edges with a
/// deleted record.
async fn open_pool(db_path: &Path, create: bool) -> Result<SqlitePool> {
	let options = SqliteConnectOptions::from_str(&format!("sqlite:{}", db_path.display()))
		.map_err(|e| Error::Other(format!("invalid database path: {e}")))?
		.create_if_missing(create)
		.foreign_keys(true)
		.journal_mode(SqliteJournalMode::Wal)
		.busy_timeout(Duration::from_secs(5));

	Ok(SqlitePoolOptions::new().connect_with(options).await?)
}

impl SourceManager {
	/// Create a new SourceManager.
	pub fn new(sources_dir: PathBuf) -> Self {
		Self { sources_dir }
	}

	/// Apply the record table and the data type's facet DDL. Idempotent.
	async fn apply_schema(pool: &SqlitePool, schema: &DataTypeSchema) -> Result<()> {
		sqlx::raw_sql(RECORD_SCHEMA).execute(pool).await?;
		for sql in &generate_ddl(schema) {
			sqlx::query(sql).execute(pool).await?;
		}
		Ok(())
	}

	/// Record which schema an index was built against.
	async fn store_schema(pool: &SqlitePool, schema: &DataTypeSchema, update: bool) -> Result<()> {
		let schema_toml =
			toml::to_string_pretty(schema).map_err(|e| Error::SchemaParse(e.to_string()))?;
		let schema_hash = blake3::hash(schema_toml.as_bytes()).to_hex().to_string();

		let sql = if update {
			"UPDATE _schema SET data_type_id = ?, schema_hash = ?, schema_toml = ? WHERE id = 1"
		} else {
			"INSERT INTO _schema (data_type_id, schema_hash, schema_toml, id) VALUES (?, ?, ?, 1)"
		};

		sqlx::query(sql)
			.bind(&schema.data_type.id)
			.bind(&schema_hash[..16])
			.bind(&schema_toml)
			.execute(pool)
			.await?;

		Ok(())
	}

	/// Load the schema an index was built against.
	async fn load_schema(pool: &SqlitePool) -> Result<DataTypeSchema> {
		let row: Option<(String,)> = sqlx::query_as("SELECT schema_toml FROM _schema WHERE id = 1")
			.fetch_optional(pool)
			.await?;

		match row {
			Some((toml_str,)) => crate::schema::parser::parse(&toml_str),
			None => Err(Error::Other("source missing schema metadata".to_string())),
		}
	}

	/// Create a new source folder with its index.
	pub async fn create(&self, source_id: &str, schema: &DataTypeSchema) -> Result<()> {
		let source_dir = self.sources_dir.join(source_id);
		std::fs::create_dir_all(&source_dir)?;

		let pool = open_pool(&source_dir.join("data.db"), true).await?;

		Self::apply_schema(&pool, schema).await?;
		Self::store_schema(&pool, schema, false).await?;

		pool.close().await;

		Ok(())
	}

	/// Open a source index.
	pub async fn open(&self, source_id: &str) -> Result<SourceDb> {
		let source_dir = self.sources_dir.join(source_id);
		if !source_dir.exists() {
			return Err(Error::SourceNotFound(source_id.to_string()));
		}

		let pool = open_pool(&source_dir.join("data.db"), false).await?;
		let schema = Self::load_schema(&pool).await?;

		// An index created before a record table was introduced still needs it.
		Self::apply_schema(&pool, &schema).await?;

		let epoch = crate::record::next_scan_epoch(&pool).await? - 1;
		let db = SourceDb::new(pool, schema, epoch.max(0));
		db.ensure_facet_columns().await?;

		Ok(db)
	}

	/// Open a source index, applying any safe schema migrations first.
	pub async fn open_with_migration(
		&self,
		source_id: &str,
		current_schema: &DataTypeSchema,
	) -> Result<(SourceDb, MigrationResult)> {
		let source_dir = self.sources_dir.join(source_id);
		if !source_dir.exists() {
			return Err(Error::SourceNotFound(source_id.to_string()));
		}

		let pool = open_pool(&source_dir.join("data.db"), false).await?;
		let stored_schema = Self::load_schema(&pool).await?;
		let migration_result = diff_schemas(&stored_schema, current_schema);

		// New facet tables and columns both land through the idempotent DDL path.
		Self::apply_schema(&pool, current_schema).await?;

		if !migration_result.applied.is_empty() {
			Self::store_schema(&pool, current_schema, true).await?;
		}

		let epoch = crate::record::next_scan_epoch(&pool).await? - 1;
		let db = SourceDb::new(pool, current_schema.clone(), epoch.max(0));
		db.ensure_facet_columns().await?;

		// A widened search contract changes what the index should hold.
		if migration_result.search_fields_changed {
			let indexed = db.rebuild_search_index().await?;
			tracing::info!(
				source_id,
				indexed,
				"rebuilt search index after schema change"
			);
		}

		Ok((db, migration_result))
	}

	/// Delete a source folder and its index.
	pub async fn delete(&self, source_id: &str) -> Result<()> {
		let source_dir = self.sources_dir.join(source_id);
		if source_dir.exists() {
			tokio::fs::remove_dir_all(&source_dir).await?;
		}
		Ok(())
	}

	/// Get the path to a source directory.
	pub fn source_dir(&self, source_id: &str) -> PathBuf {
		self.sources_dir.join(source_id)
	}
}
