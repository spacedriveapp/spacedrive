//! Engine: top-level orchestrator that wires together all subsystems.
//!
//! This is what consumers instantiate. The Engine manages sources (archived data),
//! adapters, search, and the processing pipeline.

use std::path::PathBuf;
use std::sync::Arc;

use crate::adapter::script::{ConfigField, ScriptAdapter};
use crate::adapter::{Adapter, AdapterRegistry, AdapterUpdateResult, SyncReport};
use crate::embed::EmbeddingModel;
use crate::error::{Error, Result};
use crate::registry::{NewSource, Registry, SourceInfo};
use crate::safety::{SafetyModel, SafetyPolicy, TrustTier, SAFETY_MODEL_VERSION};
use crate::search::router::SearchRouter;
use crate::search::{SearchFilter, SearchResult};
use crate::source::SourceManager;

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
	embedding: Arc<EmbeddingModel>,
	safety: Option<Arc<SafetyModel>>,
}

impl Engine {
	/// Create a new engine rooted at the given data directory.
	pub async fn new(config: EngineConfig) -> Result<Self> {
		let data_dir = &config.data_dir;

		// Ensure data directory exists
		std::fs::create_dir_all(data_dir)?;

		// Initialize registry (registry.db)
		let registry_path = data_dir.join("registry.db");
		let registry_url = format!("sqlite:{}?mode=rwc", registry_path.display());
		let pool = sqlx::SqlitePool::connect(&registry_url).await?;
		let registry = Arc::new(Registry::new(pool).await?);

		// Initialize source manager
		let sources_dir = data_dir.join("sources");
		std::fs::create_dir_all(&sources_dir)?;
		let sources = Arc::new(SourceManager::new(sources_dir));

		// Initialize embedding model
		let cache_dir = data_dir.join("models");
		std::fs::create_dir_all(&cache_dir)?;
		let embedding = Arc::new(EmbeddingModel::with_cache_dir(&cache_dir)?);

		// Initialize safety screening model (optional — non-fatal if it fails)
		let models_dir = data_dir.join("models");
		std::fs::create_dir_all(&models_dir)?;
		let safety = match SafetyModel::new(&models_dir) {
			Ok(model) => {
				tracing::info!("safety screening model loaded (Prompt Guard 2 22M)");
				Some(Arc::new(model))
			}
			Err(e) => {
				tracing::warn!(error = %e, "safety screening model failed to load — records will be marked as 'unscreened'");
				None
			}
		};

		// Initialize search router
		let search = SearchRouter::new(registry.clone(), sources.clone(), embedding.clone());

		// Load adapters from adapters directory
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
			embedding,
			safety,
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

	/// Access the embedding model.
	pub fn embedding(&self) -> &EmbeddingModel {
		&self.embedding
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

		// Get the adapter's data type
		let data_type = adapter.data_type().to_string();

		// Extract schema from the adapter
		let adapters_dir = self.config.data_dir.join("adapters").join(adapter_id);
		let schema = if adapters_dir.join("adapter.toml").exists() {
			let sa = ScriptAdapter::from_dir(&adapters_dir)?;
			sa.schema().clone()
		} else {
			return Err(Error::Other(format!(
				"cannot resolve schema for adapter '{adapter_id}' (not a script adapter with adapter.toml)"
			)));
		};

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

	/// Delete a source (removes folder + registry entry).
	pub async fn delete_source(&self, source_id: &str) -> Result<()> {
		// Delete from disk
		self.sources.delete(source_id).await?;

		// Delete from registry
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

		// Open database with migration check
		let adapters_dir = self
			.config
			.data_dir
			.join("adapters")
			.join(&source_info.adapter_id);
		let db = if adapters_dir.join("adapter.toml").exists() {
			let sa = ScriptAdapter::from_dir(&adapters_dir)?;
			let current_schema = sa.schema().clone();

			let (db, migration_result) = self
				.sources
				.open_with_migration(source_id, &current_schema)
				.await?;

			if !migration_result.applied.is_empty() {
				tracing::info!(
					source_id,
					actions = ?migration_result.applied,
					"schema migration applied during sync"
				);
			}

			db
		} else {
			self.sources.open(source_id).await?
		};

		// Build config with secrets resolved at the library level
		let config = source_info.config.clone();

		// Inject _data_dir for script adapters
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

		// Run sync
		let report = adapter.sync(&db, &config).await?;

		// Post-sync: screen new records for prompt injection
		let safety_policy = SafetyPolicy {
			mode: source_info.safety_mode,
			quarantine_threshold: source_info.quarantine_threshold,
			flag_threshold: source_info.flag_threshold,
			skip_screening: source_info.trust_tier == TrustTier::Authored
				&& source_info.safety_mode != crate::safety::SafetyMode::Strict,
		};

		if report.error.is_none() {
			match self
				.screen_new_records(source_id, &db, &safety_policy)
				.await
			{
				Ok(count) if count > 0 => {
					tracing::info!(
						source_id,
						screened = count,
						trust_tier = %source_info.trust_tier,
						mode = %safety_policy.mode,
						"safety screening after sync"
					);
				}
				Ok(_) => {}
				Err(e) => {
					tracing::warn!(source_id, error = %e, "post-sync safety screening failed (non-fatal)");
				}
			}
		}

		// Post-sync: embed new/updated records
		if report.error.is_none() {
			match self.embed_new_records(source_id, &db).await {
				Ok(count) if count > 0 => {
					tracing::info!(
						source_id,
						embedded = count,
						"generated embeddings after sync"
					);
				}
				Ok(_) => {}
				Err(e) => {
					tracing::warn!(source_id, error = %e, "post-sync embedding failed (non-fatal)");
				}
			}
		}

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
			// Count total records
			let schema = db.schema();
			let mut total_count = 0i64;
			for model_name in schema.models.keys() {
				total_count += db.count(model_name).await.unwrap_or(0);
			}

			self.registry
				.update_source_status(source_id, "idle", Some(total_count), Some(&now))
				.await?;
		}

		Ok(report)
	}

	/// Screen records that haven't been through safety screening yet.
	async fn screen_new_records(
		&self,
		source_id: &str,
		db: &crate::db::SourceDb,
		policy: &SafetyPolicy,
	) -> Result<usize> {
		// Fast path: skip screening entirely for authored sources
		if policy.skip_screening {
			return self
				.mark_all_unscreened_safe(source_id, db, "skipped")
				.await;
		}

		let safety = match &self.safety {
			Some(s) => s.clone(),
			None => return self.mark_all_unscreened_safe(source_id, db, "none").await,
		};

		const BATCH_SIZE: usize = 64;
		let mut total_screened = 0;

		loop {
			let records = db.records_needing_screening(BATCH_SIZE).await?;
			if records.is_empty() {
				break;
			}

			let count = records.len();
			let texts: Vec<String> = records.iter().map(|r| r.content.clone()).collect();
			let verdicts = safety.screen_batch(texts).await?;

			for (record, verdict) in records.iter().zip(verdicts.iter()) {
				let verdict_str =
					verdict.verdict_string(policy.quarantine_threshold, policy.flag_threshold);

				db.mark_screened(&record.id, verdict.score, verdict_str, SAFETY_MODEL_VERSION)
					.await?;

				if verdict_str == "quarantined" {
					tracing::warn!(
						source_id,
						record_id = %record.id,
						score = verdict.score,
						trust_tier = %policy.mode,
						"record quarantined — suspected prompt injection"
					);
				}
			}

			total_screened += count;

			if count < BATCH_SIZE {
				break;
			}
		}

		Ok(total_screened)
	}

	/// Mark all unscreened records as 'safe' without running the model.
	async fn mark_all_unscreened_safe(
		&self,
		source_id: &str,
		db: &crate::db::SourceDb,
		version: &str,
	) -> Result<usize> {
		let mut total = 0;
		loop {
			let records = db.records_needing_screening(64).await?;
			if records.is_empty() {
				break;
			}
			let count = records.len();
			for record in &records {
				db.mark_screened(&record.id, 0, "safe", version).await?;
			}
			total += count;
			if count < 64 {
				break;
			}
		}
		if total > 0 {
			tracing::debug!(
				source_id,
				total,
				version,
				"marked records as safe (screening skipped)"
			);
		}
		Ok(total)
	}

	/// Embed records that are new or updated since their last embedding.
	async fn embed_new_records(&self, source_id: &str, db: &crate::db::SourceDb) -> Result<usize> {
		const BATCH_SIZE: usize = 64;
		let mut total_embedded = 0;

		let lance_dir = self.sources.source_dir(source_id).join("embeddings.lance");
		let vector_store = crate::search::vector::VectorStore::open_or_create(&lance_dir).await?;

		loop {
			let records = db.records_needing_embedding(BATCH_SIZE).await?;
			if records.is_empty() {
				break;
			}

			let count = records.len();
			let texts: Vec<String> = records.iter().map(|r| r.content.clone()).collect();

			let embeddings = self.embedding.embed_batch(texts).await?;

			for (record, embedding) in records.iter().zip(embeddings.iter()) {
				if let Err(e) = vector_store
					.upsert(&record.id, &record.content, embedding)
					.await
				{
					tracing::warn!(
						source_id,
						record_id = %record.id,
						error = %e,
						"failed to upsert embedding"
					);
				}
			}

			let ids: Vec<String> = records.iter().map(|r| r.id.clone()).collect();
			db.mark_embedded(&ids).await?;

			total_embedded += count;

			if count < BATCH_SIZE {
				break;
			}
		}

		Ok(total_embedded)
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
	) -> Result<Vec<crate::db::ItemRow>> {
		let db = self.sources.open(source_id).await?;
		db.list_items(limit, offset).await
	}

	/// Get config fields for an adapter (from its manifest).
	pub fn adapter_config_fields(
		&self,
		adapter_id: &str,
	) -> Result<Vec<crate::adapter::script::ConfigField>> {
		let manifest_path = self
			.config
			.data_dir
			.join("adapters")
			.join(adapter_id)
			.join("adapter.toml");

		if !manifest_path.exists() {
			return Err(Error::AdapterNotFound(adapter_id.to_string()));
		}

		let manifest = crate::adapter::script::AdapterManifest::from_file(&manifest_path)?;
		Ok(manifest.adapter.config)
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

		for candidate in candidates.into_iter().flatten() {
			if candidate.is_dir() {
				return Some(candidate);
			}
		}

		None
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
		let schema_changed = crate::schema::migration::schema_hash(&old_schema)
			!= crate::schema::migration::schema_hash(&new_schema);

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
