//! SourceManager: manages source folders and their databases.

use std::path::{Path, PathBuf};

use crate::db::SourceDb;
use crate::error::{Error, Result};
use crate::schema::codegen::generate_ddl;
use crate::schema::migration::{diff_schemas, MigrationResult};
use crate::schema::DataTypeSchema;

/// Manages source folders on disk.
pub struct SourceManager {
	sources_dir: PathBuf,
}

impl SourceManager {
	/// Create a new SourceManager.
	pub fn new(sources_dir: PathBuf) -> Self {
		Self { sources_dir }
	}

	/// Create a new source folder with database.
	pub async fn create(&self, source_id: &str, schema: &DataTypeSchema) -> Result<()> {
		let source_dir = self.sources_dir.join(source_id);
		std::fs::create_dir_all(&source_dir)?;

		let db_path = source_dir.join("data.db");
		let db_url = format!("sqlite:{}?mode=rwc", db_path.display());
		let pool = sqlx::SqlitePool::connect(&db_url).await?;

		// Apply DDL
		let ddl = generate_ddl(schema);
		for sql in &ddl {
			sqlx::query(sql).execute(&pool).await?;
		}

		// Store schema version
		let schema_toml =
			toml::to_string_pretty(schema).map_err(|e| Error::SchemaParse(e.to_string()))?;
		let schema_hash = blake3::hash(schema_toml.as_bytes()).to_hex().to_string();
		sqlx::query(
			"INSERT INTO _schema (id, data_type_id, schema_hash, schema_toml) VALUES (1, ?, ?, ?)",
		)
		.bind(&schema.data_type.id)
		.bind(&schema_hash[..16])
		.bind(&schema_toml)
		.execute(&pool)
		.await?;

		pool.close().await;

		Ok(())
	}

	/// Open a source database.
	pub async fn open(&self, source_id: &str) -> Result<SourceDb> {
		let source_dir = self.sources_dir.join(source_id);
		if !source_dir.exists() {
			return Err(Error::SourceNotFound(source_id.to_string()));
		}

		let db_path = source_dir.join("data.db");
		let db_url = format!("sqlite:{}?mode=rw", db_path.display());
		let pool = sqlx::SqlitePool::connect(&db_url).await?;

		// Load current schema from _schema table
		let row: Option<(String,)> = sqlx::query_as("SELECT schema_toml FROM _schema WHERE id = 1")
			.fetch_optional(&pool)
			.await?;

		let schema = match row {
			Some((toml_str,)) => crate::schema::parser::parse(&toml_str)?,
			None => return Err(Error::Other("source missing schema metadata".to_string())),
		};

		let db = SourceDb::new(pool, schema);
		db.ensure_system_columns().await?;

		Ok(db)
	}

	/// Open a source database with migration check.
	pub async fn open_with_migration(
		&self,
		source_id: &str,
		current_schema: &DataTypeSchema,
	) -> Result<(SourceDb, MigrationResult)> {
		let source_dir = self.sources_dir.join(source_id);
		let db_path = source_dir.join("data.db");
		let db_url = format!("sqlite:{}?mode=rw", db_path.display());
		let pool = sqlx::SqlitePool::connect(&db_url).await?;

		// Load stored schema
		let row: Option<(String,)> = sqlx::query_as("SELECT schema_toml FROM _schema WHERE id = 1")
			.fetch_optional(&pool)
			.await?;

		let stored_schema = match row {
			Some((toml_str,)) => crate::schema::parser::parse(&toml_str)?,
			None => return Err(Error::Other("source missing schema metadata".to_string())),
		};

		// Diff and apply migrations
		let migration_result = diff_schemas(&stored_schema, current_schema);

		// Apply safe migrations
		for action in &migration_result.applied {
			match action {
				crate::schema::migration::MigrationAction::AddTable { name } => {
					// Regenerate full DDL — SQLite doesn't support CREATE TABLE IF NOT EXISTS
					// for adding new tables, so we run the full DDL and let it no-op for existing tables
					let ddl = generate_ddl(current_schema);
					for sql in &ddl {
						sqlx::query(sql).execute(&pool).await?;
					}
				}
				crate::schema::migration::MigrationAction::AddColumn { table, column } => {
					let sql = format!("ALTER TABLE \"{table}\" ADD COLUMN \"{column}\" TEXT");
					sqlx::query(&sql).execute(&pool).await?;
				}
				crate::schema::migration::MigrationAction::AddFtsColumn { column: _ } => {
					// FTS columns are handled by rebuilding the FTS index
					// This requires a VACUUM/rebuild which is expensive — skip for now
					tracing::info!(
						"FTS column added — search index will update on next record change"
					);
				}
			}
		}

		// Update stored schema if any migrations were applied
		if !migration_result.applied.is_empty() {
			let schema_toml = toml::to_string_pretty(current_schema)
				.map_err(|e| Error::SchemaParse(e.to_string()))?;
			let schema_hash = blake3::hash(schema_toml.as_bytes()).to_hex().to_string();
			sqlx::query(
				"UPDATE _schema SET data_type_id = ?, schema_hash = ?, schema_toml = ? WHERE id = 1"
			)
			.bind(&current_schema.data_type.id)
			.bind(&schema_hash[..16])
			.bind(&schema_toml)
			.execute(&pool)
			.await?;
		}

		let db = SourceDb::new(pool, current_schema.clone());
		db.ensure_system_columns().await?;

		Ok((db, migration_result))
	}

	/// Delete a source folder.
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
