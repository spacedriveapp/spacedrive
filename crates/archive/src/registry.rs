//! Central registry: manages `registry.db` with source and data type metadata.

use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

use crate::error::{Error, Result};
use crate::safety::{SafetyMode, SafetyPolicy, TrustTier};
use crate::schema::DataTypeSchema;

/// Central registry backed by `registry.db`.
pub struct Registry {
	pool: SqlitePool,
}

/// Info about a registered source.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceInfo {
	pub id: String,
	pub name: String,
	pub data_type: String,
	pub adapter_id: String,
	pub config: serde_json::Value,
	pub item_count: i64,
	pub last_synced: Option<String>,
	pub status: String,
	pub created_at: String,
	pub trust_tier: TrustTier,
	pub safety_mode: SafetyMode,
	pub quarantine_threshold: u8,
	pub flag_threshold: u8,
}

/// Info about a registered data type.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataTypeInfo {
	pub id: String,
	pub name: String,
	pub icon: Option<String>,
	pub schema_hash: String,
}

/// Parameters for creating a new source.
pub struct NewSource {
	pub name: String,
	pub data_type: String,
	pub adapter_id: String,
	pub config: serde_json::Value,
	pub trust_tier: TrustTier,
}

impl Registry {
	/// Initialize the registry, creating tables if needed.
	pub async fn new(pool: SqlitePool) -> Result<Self> {
		sqlx::query("PRAGMA journal_mode = WAL")
			.execute(&pool)
			.await?;

		sqlx::query(
			"CREATE TABLE IF NOT EXISTS sources (
				id TEXT PRIMARY KEY,
				name TEXT NOT NULL,
				data_type TEXT NOT NULL,
				adapter_id TEXT NOT NULL,
				config TEXT NOT NULL DEFAULT '{}',
				item_count INTEGER NOT NULL DEFAULT 0,
				last_synced TEXT,
				status TEXT NOT NULL DEFAULT 'idle',
				trust_tier TEXT NOT NULL DEFAULT 'external',
				safety_mode TEXT NOT NULL DEFAULT 'strict',
				quarantine_threshold INTEGER NOT NULL DEFAULT 70,
				flag_threshold INTEGER NOT NULL DEFAULT 40,
				created_at TEXT NOT NULL DEFAULT (datetime('now'))
			)",
		)
		.execute(&pool)
		.await?;

		// Migrate existing databases
		for alter in [
			"ALTER TABLE sources ADD COLUMN trust_tier TEXT NOT NULL DEFAULT 'external'",
			"ALTER TABLE sources ADD COLUMN safety_mode TEXT NOT NULL DEFAULT 'strict'",
			"ALTER TABLE sources ADD COLUMN quarantine_threshold INTEGER NOT NULL DEFAULT 70",
			"ALTER TABLE sources ADD COLUMN flag_threshold INTEGER NOT NULL DEFAULT 40",
		] {
			let _ = sqlx::query(alter).execute(&pool).await;
		}

		sqlx::query(
			"CREATE TABLE IF NOT EXISTS data_types (
				id TEXT PRIMARY KEY,
				name TEXT NOT NULL,
				icon TEXT,
				schema_hash TEXT NOT NULL,
				schema_toml TEXT NOT NULL,
				registered_at TEXT NOT NULL DEFAULT (datetime('now'))
			)",
		)
		.execute(&pool)
		.await?;

		Ok(Self { pool })
	}

	/// List all sources.
	pub async fn list_sources(&self) -> Result<Vec<SourceInfo>> {
		let rows = sqlx::query_as::<_, SourceRow>(
			"SELECT id, name, data_type, adapter_id, config, item_count, last_synced, status,
					trust_tier, safety_mode, quarantine_threshold, flag_threshold, created_at
			 FROM sources ORDER BY created_at DESC",
		)
		.fetch_all(&self.pool)
		.await?;

		Ok(rows.into_iter().map(SourceRow::into_info).collect())
	}

	/// Get a specific source by ID.
	pub async fn get_source(&self, id: &str) -> Result<SourceInfo> {
		let row = sqlx::query_as::<_, SourceRow>(
			"SELECT id, name, data_type, adapter_id, config, item_count, last_synced, status,
					trust_tier, safety_mode, quarantine_threshold, flag_threshold, created_at
			 FROM sources WHERE id = ?",
		)
		.bind(id)
		.fetch_optional(&self.pool)
		.await?
		.ok_or_else(|| Error::SourceNotFound(id.to_string()))?;

		Ok(row.into_info())
	}

	/// Create a new source.
	pub async fn create_source(&self, new: &NewSource) -> Result<SourceInfo> {
		let id = uuid::Uuid::new_v4().to_string();
		let config_str = serde_json::to_string(&new.config)?;
		let policy = SafetyPolicy::default_for_tier(new.trust_tier);

		sqlx::query(
			"INSERT INTO sources (id, name, data_type, adapter_id, config,
								trust_tier, safety_mode, quarantine_threshold, flag_threshold)
			 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
		)
		.bind(&id)
		.bind(&new.name)
		.bind(&new.data_type)
		.bind(&new.adapter_id)
		.bind(&config_str)
		.bind(new.trust_tier.as_str())
		.bind(policy.mode.to_string())
		.bind(policy.quarantine_threshold as i32)
		.bind(policy.flag_threshold as i32)
		.execute(&self.pool)
		.await?;

		self.get_source(&id).await
	}

	/// Update a source's mutable fields.
	pub async fn update_source_status(
		&self,
		id: &str,
		status: &str,
		item_count: Option<i64>,
		last_synced: Option<&str>,
	) -> Result<()> {
		let mut query = String::from("UPDATE sources SET status = ?");
		let mut binds: Vec<String> = vec![status.to_string()];

		if let Some(count) = item_count {
			query.push_str(", item_count = ?");
			binds.push(count.to_string());
		}
		if let Some(synced) = last_synced {
			query.push_str(", last_synced = ?");
			binds.push(synced.to_string());
		}
		query.push_str(" WHERE id = ?");
		binds.push(id.to_string());

		let mut q = sqlx::query(&query);
		for b in &binds {
			q = q.bind(b);
		}
		q.execute(&self.pool).await?;

		Ok(())
	}

	/// Delete a source from the registry.
	pub async fn delete_source(&self, id: &str) -> Result<()> {
		let result = sqlx::query("DELETE FROM sources WHERE id = ?")
			.bind(id)
			.execute(&self.pool)
			.await?;

		if result.rows_affected() == 0 {
			return Err(Error::SourceNotFound(id.to_string()));
		}

		Ok(())
	}

	/// List all registered data types.
	pub async fn list_data_types(&self) -> Result<Vec<DataTypeInfo>> {
		let rows = sqlx::query_as::<_, DataTypeRow>(
			"SELECT id, name, icon, schema_hash FROM data_types ORDER BY name",
		)
		.fetch_all(&self.pool)
		.await?;

		Ok(rows
			.into_iter()
			.map(|r| DataTypeInfo {
				id: r.id,
				name: r.name,
				icon: r.icon,
				schema_hash: r.schema_hash,
			})
			.collect())
	}

	/// Register a data type schema.
	pub async fn register_data_type(&self, schema: &DataTypeSchema) -> Result<()> {
		let schema_toml =
			toml::to_string_pretty(schema).map_err(|e| Error::SchemaParse(e.to_string()))?;
		let schema_hash = blake3::hash(schema_toml.as_bytes()).to_hex();
		let hash_short = &schema_hash.as_str()[..16];

		sqlx::query(
			"INSERT INTO data_types (id, name, icon, schema_hash, schema_toml)
			 VALUES (?, ?, ?, ?, ?)
			 ON CONFLICT (id) DO UPDATE SET
				name = excluded.name,
				icon = excluded.icon,
				schema_hash = excluded.schema_hash,
				schema_toml = excluded.schema_toml",
		)
		.bind(&schema.data_type.id)
		.bind(&schema.data_type.name)
		.bind(&schema.data_type.icon)
		.bind(hash_short)
		.bind(&schema_toml)
		.execute(&self.pool)
		.await?;

		Ok(())
	}

	/// Get the underlying pool.
	pub fn pool(&self) -> &SqlitePool {
		&self.pool
	}
}

#[derive(sqlx::FromRow)]
struct SourceRow {
	id: String,
	name: String,
	data_type: String,
	adapter_id: String,
	config: String,
	item_count: i64,
	last_synced: Option<String>,
	status: String,
	trust_tier: String,
	safety_mode: String,
	quarantine_threshold: i32,
	flag_threshold: i32,
	created_at: String,
}

impl SourceRow {
	fn into_info(self) -> SourceInfo {
		SourceInfo {
			id: self.id,
			name: self.name,
			data_type: self.data_type,
			adapter_id: self.adapter_id,
			config: serde_json::from_str(&self.config).unwrap_or_default(),
			item_count: self.item_count,
			last_synced: self.last_synced,
			status: self.status,
			trust_tier: TrustTier::from_str_or_default(&self.trust_tier),
			safety_mode: SafetyMode::from_str_or_default(&self.safety_mode),
			quarantine_threshold: self.quarantine_threshold as u8,
			flag_threshold: self.flag_threshold as u8,
			created_at: self.created_at,
		}
	}
}

#[derive(sqlx::FromRow)]
struct DataTypeRow {
	id: String,
	name: String,
	icon: Option<String>,
	schema_hash: String,
}
