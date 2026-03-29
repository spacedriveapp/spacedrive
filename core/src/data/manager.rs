//! SourceManager: library-scoped wrapper around sd-archive Engine.

use std::path::PathBuf;

use sd_archive::{Engine, EngineConfig};
use tracing::info;

/// Manages archive data sources for a single library.
pub struct SourceManager {
	engine: Engine,
}

impl SourceManager {
	/// Create a new source manager rooted at the library's archive directory.
	pub async fn new(library_path: PathBuf) -> Result<Self, String> {
		let data_dir = library_path.join("archive");

		let config = EngineConfig {
			data_dir: data_dir.clone(),
		};
		let engine = Engine::new(config)
			.await
			.map_err(|e| format!("Failed to initialize archive engine: {e}"))?;

		// Sync bundled adapters from the source tree into the installed adapters
		// directory. Uses CARGO_MANIFEST_DIR at compile time to find the workspace
		// root, matching the pattern from the spacedrive-data prototype.
		let installed_dir = data_dir.join("adapters");
		Self::sync_bundled_adapters(&installed_dir);

		// Reload adapters after sync (picks up any newly copied adapters)
		Engine::load_script_adapters(&installed_dir, engine.adapters())
			.map_err(|e| format!("Failed to reload adapters: {e}"))?;

		info!("Source manager initialized at {}", library_path.display());

		Ok(Self { engine })
	}

	/// Sync bundled adapters from the compile-time workspace into the installed
	/// adapters directory. New adapters are copied; existing ones are updated if
	/// the adapter.toml has changed.
	fn sync_bundled_adapters(installed_dir: &std::path::Path) {
		let source_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
			.parent()
			.map(|p| p.join("adapters"));

		let source_dir = match source_dir {
			Some(d) if d.is_dir() => d,
			_ => return,
		};

		info!(
			"Syncing bundled adapters from {} to {}",
			source_dir.display(),
			installed_dir.display()
		);

		let entries = match std::fs::read_dir(&source_dir) {
			Ok(e) => e,
			Err(_) => return,
		};

		for entry in entries.flatten() {
			let src_path = entry.path();
			if !src_path.is_dir() || !src_path.join("adapter.toml").exists() {
				continue;
			}

			let adapter_name = match src_path.file_name() {
				Some(n) => n.to_owned(),
				None => continue,
			};
			let dest_path = installed_dir.join(&adapter_name);

			if !dest_path.exists() {
				// New adapter — copy entire directory
				if let Err(e) = copy_dir_recursive(&src_path, &dest_path) {
					tracing::warn!(
						adapter = ?adapter_name,
						error = %e,
						"failed to install bundled adapter"
					);
				} else {
					info!(adapter = ?adapter_name, "installed bundled adapter");
				}
			} else {
				// Existing adapter — update if adapter.toml changed
				let src_manifest = std::fs::read_to_string(src_path.join("adapter.toml"));
				let dest_manifest = std::fs::read_to_string(dest_path.join("adapter.toml"));

				if let (Ok(src), Ok(dest)) = (src_manifest, dest_manifest) {
					if src != dest {
						if let Err(e) = copy_dir_recursive(&src_path, &dest_path) {
							tracing::warn!(
								adapter = ?adapter_name,
								error = %e,
								"failed to update bundled adapter"
							);
						} else {
							info!(adapter = ?adapter_name, "updated bundled adapter");
						}
					}
				}
			}
		}
	}

	/// List all sources.
	pub async fn list_sources(&self) -> Result<Vec<sd_archive::SourceInfo>, String> {
		self.engine
			.list_sources()
			.await
			.map_err(|e| format!("Failed to list sources: {e}"))
	}

	/// Create a new source.
	pub async fn create_source(
		&self,
		name: &str,
		adapter_id: &str,
		config: serde_json::Value,
	) -> Result<sd_archive::SourceInfo, String> {
		self.engine
			.create_source(name, adapter_id, config)
			.await
			.map_err(|e| format!("Failed to create source: {e}"))
	}

	/// Delete a source.
	pub async fn delete_source(&self, source_id: &str) -> Result<(), String> {
		self.engine
			.delete_source(source_id)
			.await
			.map_err(|e| format!("Failed to delete source: {e}"))
	}

	/// Sync a source.
	pub async fn sync_source(
		&self,
		source_id: &str,
	) -> Result<sd_archive::SyncReport, String> {
		self.engine
			.sync(source_id)
			.await
			.map_err(|e| format!("Failed to sync source: {e}"))
	}

	/// List items from a source.
	pub async fn list_items(
		&self,
		source_id: &str,
		limit: usize,
		offset: usize,
	) -> Result<Vec<sd_archive::db::ItemRow>, String> {
		self.engine
			.list_items(source_id, limit, offset)
			.await
			.map_err(|e| format!("Failed to list items: {e}"))
	}

	/// List available adapters with update status.
	pub fn list_adapters(&self) -> Vec<sd_archive::AdapterInfo> {
		let source_dir = self.engine.source_adapters_dir();
		self.engine
			.list_adapters_with_updates(source_dir.as_deref())
	}

	/// Update an installed adapter from its source directory.
	pub fn update_adapter(
		&self,
		adapter_id: &str,
	) -> Result<sd_archive::AdapterUpdateResult, String> {
		let source_dir = self
			.engine
			.source_adapters_dir()
			.ok_or_else(|| "Cannot find source adapters directory".to_string())?
			.join(adapter_id);

		if !source_dir.join("adapter.toml").exists() {
			return Err(format!("No source adapter found for '{adapter_id}'"));
		}

		self.engine
			.update_adapter(adapter_id, &source_dir)
			.map_err(|e| format!("Failed to update adapter: {e}"))
	}

	/// Get config fields for an adapter.
	pub fn adapter_config_fields(
		&self,
		adapter_id: &str,
	) -> Result<Vec<sd_archive::adapter::script::ConfigField>, String> {
		self.engine
			.adapter_config_fields(adapter_id)
			.map_err(|e| format!("Failed to get adapter config: {e}"))
	}

	/// Get the underlying engine.
	pub fn engine(&self) -> &Engine {
		&self.engine
	}
}

/// Recursively copy a directory.
fn copy_dir_recursive(src: &std::path::Path, dest: &std::path::Path) -> Result<(), String> {
	std::fs::create_dir_all(dest).map_err(|e| e.to_string())?;
	for entry in std::fs::read_dir(src).map_err(|e| e.to_string())? {
		let entry = entry.map_err(|e| e.to_string())?;
		let src_path = entry.path();
		let dest_path = dest.join(entry.file_name());
		if src_path.is_dir() {
			copy_dir_recursive(&src_path, &dest_path)?;
		} else {
			std::fs::copy(&src_path, &dest_path).map_err(|e| e.to_string())?;
		}
	}
	Ok(())
}
