//! Engine: top-level orchestrator that wires together all subsystems.
//!
//! This is what consumers instantiate. The Engine manages sources (archived data),
//! adapters, search, and the processing pipeline.

use std::path::PathBuf;
use std::sync::Arc;

use crate::adapter::script::{ConfigField, ScriptAdapter};
use crate::adapter::{Adapter, AdapterRegistry, SyncReport};
use crate::error::{Error, Result};
use crate::registry::{NewSource, Registry, SourceInfo};
use crate::search::router::SearchRouter;
use crate::search::{SearchFilter, SearchResult};
use sd_store::source::SourceManager;

/// Configuration for initializing the engine.
pub struct EngineConfig {
	/// Path to the data directory where sources are stored.
	pub data_dir: PathBuf,
}

/// The top-level archive engine. Holds all subsystems.
pub struct Engine {
	config: EngineConfig,
	registry: Arc<Registry>,
	sources: Arc<SourceManager>,
	adapters: AdapterRegistry,
	search: SearchRouter,
}

impl Engine {
	/// Create a new engine rooted at the given data directory.
	pub async fn new(config: EngineConfig) -> Result<Self> {
		let data_dir = &config.data_dir;

		// Ensure data directory exists
		std::fs::create_dir_all(data_dir)?;

		// Initialize the source registry (registry.db)
		let registry_path = data_dir.join("registry.db");
		let registry_url = format!("sqlite:{}?mode=rwc", registry_path.display());
		let pool = sqlx::SqlitePool::connect(&registry_url).await?;
		let registry = Arc::new(Registry::new(pool).await?);

		// Initialize source manager
		let sources_dir = data_dir.join("sources");
		std::fs::create_dir_all(&sources_dir)?;
		let sources = Arc::new(SourceManager::new(sources_dir));

		// Initialize search router
		let search = SearchRouter::new(registry.clone(), sources.clone());

		let adapters = AdapterRegistry::new();
		let adapters_dir = data_dir.join("adapters");
		std::fs::create_dir_all(&adapters_dir)?;
		Self::load_script_adapters(&adapters_dir, &adapters)?;

		Ok(Self {
			config,
			registry,
			sources,
			adapters,
			search,
		})
	}

	/// Load all script adapters from the adapters directory.
	pub fn load_script_adapters(
		adapters_dir: &std::path::Path,
		registry: &AdapterRegistry,
	) -> Result<()> {
		for entry in std::fs::read_dir(adapters_dir)? {
			let entry = entry?;
			let path = entry.path();
			if path.is_dir() && path.join("adapter.toml").exists() {
				match ScriptAdapter::from_dir(&path) {
					Ok(adapter) => {
						tracing::info!(
							adapter_id = %adapter.id(),
							adapter_name = %adapter.name(),
							"loaded script adapter"
						);
						registry.register(Arc::new(adapter));
					}
					Err(e) => {
						tracing::warn!(
							path = %path.display(),
							error = %e,
							"failed to load adapter"
						);
					}
				}
			}
		}
		Ok(())
	}

	// ── Public API ──────────────────────────────────────────────────────

	/// Access the registry (list sources, data types).
	pub fn registry(&self) -> &Registry {
		&self.registry
	}

	/// Access the source manager.
	pub fn sources(&self) -> &SourceManager {
		&self.sources
	}

	/// Access the search router.
	pub fn search_router(&self) -> &SearchRouter {
		&self.search
	}

	/// Access the adapter registry.
	pub fn adapters(&self) -> &AdapterRegistry {
		&self.adapters
	}

	/// The data directory path.
	pub fn data_dir(&self) -> &std::path::Path {
		&self.config.data_dir
	}

	/// Cross-source search.
	pub async fn search(
		&self,
		query: &str,
		filter: Option<SearchFilter>,
	) -> Result<Vec<SearchResult>> {
		self.search.search(query, filter).await
	}

	/// Create a new source from an adapter and config.
	pub async fn create_source(
		&self,
		name: &str,
		adapter_id: &str,
		config: serde_json::Value,
	) -> Result<SourceInfo> {
		// Find adapter
		let adapter = self
			.adapters
			.get(adapter_id)
			.ok_or_else(|| Error::AdapterNotFound(adapter_id.to_string()))?;

		// The adapter carries its data type schema, compiled in or parsed from
		// its manifest
		let data_type = adapter.data_type().to_string();
		let schema = adapter.schema().clone();

		// Create registry entry — trust tier comes from the adapter
		let trust_tier = adapter.trust_tier();
		let source_info = self
			.registry
			.create_source(&NewSource {
				name: name.to_string(),
				data_type,
				adapter_id: adapter_id.to_string(),
				config,
				trust_tier,
			})
			.await?;

		// Create source folder + database
		self.sources.create(&source_info.id, &schema).await?;

		Ok(source_info)
	}

	/// Delete a source: its store on disk and its registry entry.
	///
	/// The store holds the source's assertions, so this discards them too.
	/// Re-indexing a source is a different operation — it replaces the rows an
	/// ingest produced and leaves `record_overlay` alone, which is what the
	/// `(type, external_id)` key is for.
	pub async fn delete_source(&self, source_id: &str) -> Result<()> {
		self.sources.delete(source_id).await?;
		self.registry.delete_source(source_id).await?;

		Ok(())
	}

	/// Trigger a sync for a source.
	pub async fn sync(&self, source_id: &str) -> Result<SyncReport> {
		// Get source info
		let source_info = self.registry.get_source(source_id).await?;

		// Find adapter
		let adapter = self
			.adapters
			.get(&source_info.adapter_id)
			.ok_or_else(|| Error::AdapterNotFound(source_info.adapter_id.clone()))?;

		// Open the index against the adapter's current schema, applying any
		// safe migrations the diff allows
		let (db, migration_result) = self
			.sources
			.open_with_migration(source_id, adapter.schema())
			.await?;

		if !migration_result.applied.is_empty() {
			tracing::info!(
				source_id,
				actions = ?migration_result.applied,
				"schema migration applied during sync"
			);
		}

		// Build config with secrets resolved at the library level
		let config = source_info.config.clone();

		// Expose the source's data directory to the adapter
		let mut config = config;
		let data_dir = self.sources.source_dir(source_id);
		if let Some(obj) = config.as_object_mut() {
			obj.insert(
				"_data_dir".to_string(),
				serde_json::Value::String(data_dir.to_string_lossy().to_string()),
			);
		}

		// Update status to syncing
		self.registry
			.update_source_status(source_id, "syncing", None, None)
			.await?;

		// Stamp everything this run writes with a fresh epoch.
		let epoch = db.begin_sync().await?;
		tracing::debug!(source_id, epoch, "sync run started");

		// Run sync
		let report = adapter.sync(&db, &config).await?;

		// Update status based on result
		let now = chrono::Utc::now().to_rfc3339();
		if report.error.is_some() {
			self.registry
				.update_source_status(
					source_id,
					"error",
					Some(report.records_upserted as i64),
					Some(&now),
				)
				.await?;
		} else {
			let total_count = db.count_all().await.unwrap_or(0);

			self.registry
				.update_source_status(source_id, "idle", Some(total_count), Some(&now))
				.await?;
		}

		Ok(report)
	}

	/// List all sources.
	pub async fn list_sources(&self) -> Result<Vec<SourceInfo>> {
		self.registry.list_sources().await
	}

	/// List items from a source's primary model table.
	pub async fn list_items(
		&self,
		source_id: &str,
		limit: usize,
		offset: usize,
	) -> Result<Vec<sd_store::db::ItemRow>> {
		let db = self.sources.open(source_id).await?;
		Ok(db.list_items(limit, offset).await?)
	}

	pub async fn list_records_full(
		&self,
		source_id: &str,
		limit: usize,
		offset: usize,
	) -> Result<Vec<serde_json::Value>> {
		let db = self.sources.open(source_id).await?;
		Ok(db.list_records_full(limit, offset).await?)
	}

	/// The on-disk root a file-backed source's locator paths resolve
	/// against. `None` for foreign sources, whose records are primary.
	pub async fn file_root(&self, source_id: &str) -> Result<Option<String>> {
		let db = self.sources.open(source_id).await?;
		Ok(db.get_cursor(sd_store::db::FILE_ROOT_CURSOR).await?)
	}

	/// Get config fields for an adapter.
	pub fn adapter_config_fields(&self, adapter_id: &str) -> Result<Vec<ConfigField>> {
		let adapter = self
			.adapters
			.get(adapter_id)
			.ok_or_else(|| Error::AdapterNotFound(adapter_id.to_string()))?;

		Ok(adapter.config_fields())
	}

	/// Check whether a source adapter directory has changed compared to the installed version.
	pub fn check_adapter_update(
		&self,
		adapter_id: &str,
		source_dir: &std::path::Path,
	) -> Option<bool> {
		let installed_toml = self
			.config
			.data_dir
			.join("adapters")
			.join(adapter_id)
			.join("adapter.toml");
		let source_toml = source_dir.join("adapter.toml");

		if !installed_toml.exists() || !source_toml.exists() {
			return None;
		}

		let installed_content = std::fs::read(&installed_toml).ok()?;
		let source_content = std::fs::read(&source_toml).ok()?;

		let installed_hash = blake3::hash(&installed_content);
		let source_hash = blake3::hash(&source_content);

		Some(installed_hash != source_hash)
	}

	/// List adapters with update-available status.
	pub fn list_adapters_with_updates(
		&self,
		source_adapters_dir: Option<&std::path::Path>,
	) -> Vec<crate::adapter::AdapterInfo> {
		let mut infos = self.adapters.list();

		if let Some(source_dir) = source_adapters_dir {
			for info in &mut infos {
				let adapter_source = source_dir.join(&info.id);
				if let Some(has_update) = self.check_adapter_update(&info.id, &adapter_source) {
					info.update_available = has_update;
				}
			}
		}

		infos
	}

	/// The path to the bundled adapters directory (workspace root's adapters/).
	pub fn source_adapters_dir(&self) -> Option<std::path::PathBuf> {
		let candidates = [
			std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
				.parent()
				.map(|p| p.join("adapters")),
			Some(self.config.data_dir.join("bundled_adapters")),
		];

		candidates
			.into_iter()
			.flatten()
			.find(|candidate| candidate.is_dir())
	}

	/// Update an installed adapter from a source directory.
	///
	/// Backs up the installed adapter, copies new files, and re-registers.
	/// Schema migrations happen automatically on next sync.
	pub fn update_adapter(
		&self,
		adapter_id: &str,
		source_dir: &std::path::Path,
	) -> Result<crate::adapter::AdapterUpdateResult> {
		let new_adapter = ScriptAdapter::from_dir(source_dir)?;
		if new_adapter.id() != adapter_id {
			return Err(Error::Other(format!(
				"adapter ID mismatch: expected '{}', got '{}'",
				adapter_id,
				new_adapter.id()
			)));
		}

		let installed_dir = self.config.data_dir.join("adapters").join(adapter_id);
		if !installed_dir.exists() {
			return Err(Error::AdapterNotFound(adapter_id.to_string()));
		}

		// Read old version
		let old_manifest = crate::adapter::script::AdapterManifest::from_file(
			&installed_dir.join("adapter.toml"),
		)?;
		let old_version = old_manifest.adapter.version.clone();
		let new_version = new_adapter.manifest().adapter.version.clone();

		// Schema diff
		let old_schema = ScriptAdapter::from_dir(&installed_dir)?.schema().clone();
		let new_schema = new_adapter.schema().clone();
		let schema_changed = sd_store::schema::migration::schema_hash(&old_schema)
			!= sd_store::schema::migration::schema_hash(&new_schema);

		// Backup
		let backup_name = format!(
			"{}.bak.{}",
			adapter_id,
			chrono::Utc::now().format("%Y%m%d_%H%M%S")
		);
		let backup_dir = self.config.data_dir.join("adapters").join(&backup_name);
		std::fs::rename(&installed_dir, &backup_dir)?;

		tracing::info!(adapter_id, backup = %backup_dir.display(), "backed up adapter before update");

		// Copy new files (restore backup on failure)
		if let Err(e) = copy_dir_recursive(source_dir, &installed_dir) {
			tracing::error!(adapter_id, error = %e, "update failed, restoring backup");
			if installed_dir.exists() {
				let _ = std::fs::remove_dir_all(&installed_dir);
			}
			std::fs::rename(&backup_dir, &installed_dir)?;
			return Err(e);
		}

		// Re-register
		let adapter = ScriptAdapter::from_dir(&installed_dir)?;
		self.adapters.register(Arc::new(adapter));

		tracing::info!(adapter_id, %old_version, %new_version, schema_changed, "adapter updated");

		Ok(crate::adapter::AdapterUpdateResult {
			adapter_id: adapter_id.to_string(),
			old_version,
			new_version,
			schema_changed,
			backup_path: backup_dir.to_string_lossy().to_string(),
		})
	}

	/// Install a script adapter from a directory path (sideloading).
	pub fn install_adapter(&self, source_dir: &std::path::Path) -> Result<String> {
		let adapter = ScriptAdapter::from_dir(source_dir)?;
		let adapter_id = adapter.id().to_string();

		let dest = self.config.data_dir.join("adapters").join(&adapter_id);
		if dest.exists() {
			return Err(Error::AlreadyExists(format!("adapter: {adapter_id}")));
		}

		copy_dir_recursive(source_dir, &dest)?;

		let adapter = ScriptAdapter::from_dir(&dest)?;
		self.adapters.register(Arc::new(adapter));

		tracing::info!(adapter_id = %adapter_id, "installed adapter");

		Ok(adapter_id)
	}
}

/// Recursively copy a directory.
fn copy_dir_recursive(src: &std::path::Path, dest: &std::path::Path) -> Result<()> {
	std::fs::create_dir_all(dest)?;
	for entry in std::fs::read_dir(src)? {
		let entry = entry?;
		let src_path = entry.path();
		let dest_path = dest.join(entry.file_name());

		if src_path.is_dir() {
			copy_dir_recursive(&src_path, &dest_path)?;
		} else {
			std::fs::copy(&src_path, &dest_path)?;
		}
	}
	Ok(())
}
