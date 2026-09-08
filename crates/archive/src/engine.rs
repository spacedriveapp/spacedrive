//! Engine: top-level orchestrator that wires together all subsystems.
//!
//! This is what consumers instantiate. The engine holds stores, adapters and
//! search. It does not hold the list of sources: that is library metadata, it
//! lives in the library database beside every other kind of source, and the
//! engine is told which source it is working on. Two lists of sources is how
//! they drift, and there used to be two.

use std::path::PathBuf;
use std::sync::Arc;

use crate::adapter::script::{ConfigField, ScriptAdapter};
use crate::adapter::{Adapter, AdapterRegistry, SyncReport};
use crate::error::{Error, Result};
use crate::search::router::SearchRouter;
use crate::search::{SearchFilter, SearchResult};
use sd_store::source::SourceManager;
use sd_store::TrustTier;

/// Configuration for initializing the engine.
pub struct EngineConfig {
	/// Where source stores live. Shared with every other kind of source, so an
	/// adapter's records sit beside a walk's rather than in a directory of
	/// their own.
	pub sources_dir: PathBuf,
	/// Where installed adapters live. A property of the machine rather than of
	/// a library, since the same adapter serves all of them.
	pub adapters_dir: PathBuf,
}

/// A source as the engine needs to see it: an identity, the adapter that fills
/// it, and the configuration that adapter reads.
#[derive(Debug, Clone)]
pub struct SourceRef {
	pub id: String,
	pub name: String,
	pub data_type: String,
	pub adapter_id: String,
	pub config: serde_json::Value,
	/// How much a result from this source is worth, which travels with the
	/// source rather than with the query.
	pub trust_tier: TrustTier,
}

/// What an adapter says a source made from it is.
///
/// Read off the adapter at creation time and written to the registration, so
/// nothing has to hold an adapter open to answer what data type a source has.
#[derive(Debug, Clone)]
pub struct AdapterFacts {
	pub data_type: String,
	pub trust_tier: TrustTier,
}

/// The top-level archive engine. Holds all subsystems.
pub struct Engine {
	config: EngineConfig,
	sources: Arc<SourceManager>,
	adapters: AdapterRegistry,
	search: SearchRouter,
}

impl Engine {
	/// Create a new engine rooted at the given data directory.
	pub async fn new(config: EngineConfig) -> Result<Self> {
		std::fs::create_dir_all(&config.sources_dir)?;
		std::fs::create_dir_all(&config.adapters_dir)?;

		let sources = Arc::new(SourceManager::new(config.sources_dir.clone()));
		let search = SearchRouter::new(sources.clone());

		let adapters = AdapterRegistry::new();
		Self::load_script_adapters(&config.adapters_dir, &adapters)?;

		Ok(Self {
			config,
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

	/// Where installed adapters live.
	pub fn adapters_dir(&self) -> &std::path::Path {
		&self.config.adapters_dir
	}

	/// Where source stores live.
	pub fn sources_dir(&self) -> &std::path::Path {
		&self.config.sources_dir
	}

	/// Cross-source search.
	///
	/// The sources to search are handed in, because which sources exist is a
	/// question for the library rather than for the engine.
	pub async fn search(
		&self,
		query: &str,
		filter: Option<SearchFilter>,
		sources: &[SourceRef],
	) -> Result<Vec<SearchResult>> {
		self.search.search(query, filter, sources).await
	}

	/// Create a new source from an adapter and config.
	/// Create a source's store, and answer with what its adapter says it is.
	///
	/// The caller owns the identity and writes the registration; this owns the
	/// store on disk and the schema inside it.
	pub async fn create_source(&self, id: &str, adapter_id: &str) -> Result<AdapterFacts> {
		let adapter = self
			.adapters
			.get(adapter_id)
			.ok_or_else(|| Error::AdapterNotFound(adapter_id.to_string()))?;

		// The adapter carries its data type schema, compiled in or parsed from
		// its manifest.
		self.sources.create(id, adapter.schema()).await?;

		Ok(AdapterFacts {
			data_type: adapter.data_type().to_string(),
			trust_tier: adapter.trust_tier(),
		})
	}

	/// Delete a source's store.
	///
	/// The store holds the source's assertions, so this discards them too.
	/// Re-indexing a source is a different operation — it replaces the rows an
	/// ingest produced and leaves `record_overlay` alone, which is what the
	/// `(type, external_id)` key is for.
	pub async fn delete_source(&self, source_id: &str) -> Result<()> {
		self.sources.delete(source_id).await?;

		Ok(())
	}

	/// Trigger a sync for a source.
	///
	/// Answers with what the run did. Recording that against the registration
	/// is the caller's, because the caller is where the registration lives.
	pub async fn sync(&self, source: &SourceRef) -> Result<SyncReport> {
		let adapter = self
			.adapters
			.get(&source.adapter_id)
			.ok_or_else(|| Error::AdapterNotFound(source.adapter_id.clone()))?;

		// Open the index against the adapter's current schema, applying any
		// safe migrations the diff allows
		let (db, migration_result) = self
			.sources
			.open_with_migration(&source.id, adapter.schema())
			.await?;

		if !migration_result.applied.is_empty() {
			tracing::info!(
				source_id = source.id,
				actions = ?migration_result.applied,
				"schema migration applied during sync"
			);
		}

		// Expose the source's data directory to the adapter
		let mut config = source.config.clone();
		let data_dir = self.sources.source_dir(&source.id);
		if let Some(obj) = config.as_object_mut() {
			obj.insert(
				"_data_dir".to_string(),
				serde_json::Value::String(data_dir.to_string_lossy().to_string()),
			);
		}

		// Stamp everything this run writes with a fresh epoch.
		let epoch = db.begin_sync().await?;
		tracing::debug!(source_id = source.id, epoch, "sync run started");

		adapter.sync(&db, &config).await
	}

	/// How many records a source's store holds.
	pub async fn record_count(&self, source_id: &str) -> Result<i64> {
		let db = self.sources.open(source_id).await?;
		Ok(db.count_all().await?)
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
			.adapters_dir
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
			Some(self.config.adapters_dir.join("bundled_adapters")),
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

		let installed_dir = self.config.adapters_dir.join(adapter_id);
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
		let backup_dir = self.config.adapters_dir.join(&backup_name);
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

		let dest = self.config.adapters_dir.join(&adapter_id);
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
