//! WASM Plugin Manager
//!
//! Manages the lifecycle of WASM extensions: loading, unloading, hot-reload.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use chrono::Utc;
use thiserror::Error;
use tokio::sync::RwLock;
use wasmer::{imports, Function, FunctionEnv, Instance, Memory, Module, Store};

use crate::{context::CoreContext, infra::api::ApiDispatcher};

use super::host_functions::{self, host_spacedrive_call, host_spacedrive_log, PluginEnv};
use super::permissions::ExtensionPermissions;
use super::types::{ExtensionManifest, LoadedPlugin};

#[derive(Error, Debug)]
pub enum PluginError {
	#[error("Plugin not found: {0}")]
	NotFound(String),

	#[error("Failed to load manifest: {0}")]
	ManifestLoadFailed(String),

	#[error("Failed to compile WASM module: {0}")]
	CompilationFailed(String),

	#[error("Failed to instantiate WASM module: {0}")]
	InstantiationFailed(String),

	#[error("Plugin already loaded: {0}")]
	AlreadyLoaded(String),

	#[error("I/O error: {0}")]
	Io(#[from] std::io::Error),
}

/// Manages WASM plugin lifecycle
pub struct PluginManager {
	store: Store,
	plugins: Arc<RwLock<HashMap<String, LoadedPlugin>>>,
	plugin_dir: PathBuf,
	core_context: Arc<CoreContext>,
	api_dispatcher: Arc<ApiDispatcher>,
}

impl PluginManager {
	/// Create new plugin manager
	pub fn new(
		plugin_dir: PathBuf,
		core_context: Arc<CoreContext>,
		api_dispatcher: Arc<ApiDispatcher>,
	) -> Self {
		let store = Store::default();

		Self {
			store,
			plugins: Arc::new(RwLock::new(HashMap::new())),
			plugin_dir,
			core_context,
			api_dispatcher,
		}
	}

	/// Load a WASM plugin from directory
	///
	/// Expected structure:
	/// ```
	/// plugins/finance/
	///   ├── manifest.json
	///   └── finance.wasm
	/// ```
	pub async fn load_plugin(&mut self, plugin_id: &str) -> Result<(), PluginError> {
		// Check if already loaded
		if self.plugins.read().await.contains_key(plugin_id) {
			return Err(PluginError::AlreadyLoaded(plugin_id.to_string()));
		}

		tracing::info!("Loading plugin: {}", plugin_id);

		// 1. Load manifest
		let manifest_path = self.plugin_dir.join(plugin_id).join("manifest.json");
		let manifest: ExtensionManifest = {
			let manifest_str = std::fs::read_to_string(&manifest_path).map_err(|e| {
				PluginError::ManifestLoadFailed(format!("Failed to read manifest: {}", e))
			})?;

			serde_json::from_str(&manifest_str).map_err(|e| {
				PluginError::ManifestLoadFailed(format!("Failed to parse manifest: {}", e))
			})?
		};

		tracing::debug!(
			"Loaded manifest for plugin '{}' v{}",
			manifest.name,
			manifest.version
		);

		// 2. Read WASM file
		let wasm_path = self.plugin_dir.join(plugin_id).join(&manifest.wasm_file);
		let wasm_bytes = std::fs::read(&wasm_path).map_err(|e| PluginError::Io(e))?;

		tracing::debug!("Read {} bytes of WASM", wasm_bytes.len());

		// 3. Compile WASM module
		let module = Module::new(&self.store, wasm_bytes).map_err(|e| {
			PluginError::CompilationFailed(format!("Failed to compile WASM: {}", e))
		})?;

		tracing::debug!("Compiled WASM module");

		// 4. Create plugin environment with temporary memory
		let permissions =
			ExtensionPermissions::from_manifest(manifest.id.clone(), &manifest.permissions);

		// Create temporary memory (will be replaced with instance's memory)
		let temp_memory = Memory::new(&mut self.store, wasmer::MemoryType::new(1, None, false))
			.map_err(|e| {
				PluginError::InstantiationFailed(format!("Failed to create temp memory: {}", e))
			})?;

		let plugin_env = PluginEnv {
			extension_id: manifest.id.clone(),
			core_context: self.core_context.clone(),
			api_dispatcher: self.api_dispatcher.clone(),
			permissions,
			memory: temp_memory,
		};

		let env = FunctionEnv::new(&mut self.store, plugin_env);

		// 5. Create imports (host functions exposed to WASM)
		let import_object = imports! {
			"spacedrive" => {
				// Core functions
				"spacedrive_call" => Function::new_typed_with_env(
					&mut self.store,
					&env,
					host_spacedrive_call
				),
				"spacedrive_log" => Function::new_typed_with_env(
					&mut self.store,
					&env,
					host_spacedrive_log
				),

				// Job-specific functions
				"job_report_progress" => Function::new_typed_with_env(
					&mut self.store,
					&env,
					host_functions::host_job_report_progress
				),
				"job_checkpoint" => Function::new_typed_with_env(
					&mut self.store,
					&env,
					host_functions::host_job_checkpoint
				),
				"job_check_interrupt" => Function::new_typed_with_env(
					&mut self.store,
					&env,
					host_functions::host_job_check_interrupt
				),
				"job_add_warning" => Function::new_typed_with_env(
					&mut self.store,
					&env,
					host_functions::host_job_add_warning
				),
				"job_increment_bytes" => Function::new_typed_with_env(
					&mut self.store,
					&env,
					host_functions::host_job_increment_bytes
				),
				"job_increment_items" => Function::new_typed_with_env(
					&mut self.store,
					&env,
					host_functions::host_job_increment_items
				),
			}
		};

		// 6. Instantiate WASM module
		let instance = Instance::new(&mut self.store, &module, &import_object).map_err(|e| {
			PluginError::InstantiationFailed(format!("Failed to instantiate WASM: {}", e))
		})?;

		tracing::debug!("Instantiated WASM module");

		// 7. Get actual memory from instance and update environment
		let memory = instance.exports.get_memory("memory").map_err(|e| {
			PluginError::InstantiationFailed(format!("Plugin missing memory export: {}", e))
		})?;

		env.as_mut(&mut self.store).memory = memory.clone();

		// 8. Call plugin initialization function
		if let Ok(init_fn) = instance.exports.get_function("plugin_init") {
			match init_fn.call(&mut self.store, &[]) {
				Ok(_) => tracing::info!("Plugin {} initialized successfully", plugin_id),
				Err(e) => {
					tracing::error!("Plugin init failed: {}", e);
					return Err(PluginError::InstantiationFailed(format!(
						"plugin_init() failed: {}",
						e
					)));
				}
			}
		} else {
			tracing::warn!("Plugin {} has no plugin_init() function", plugin_id);
		}

		// 9. Store loaded plugin
		self.plugins.write().await.insert(
			plugin_id.to_string(),
			LoadedPlugin {
				id: plugin_id.to_string(),
				manifest,
				loaded_at: Utc::now(),
			},
		);

		tracing::info!("✓ Plugin {} loaded successfully", plugin_id);

		Ok(())
	}

	/// Unload a plugin
	pub async fn unload_plugin(&mut self, plugin_id: &str) -> Result<(), PluginError> {
		tracing::info!("Unloading plugin: {}", plugin_id);

		let plugin = self
			.plugins
			.write()
			.await
			.remove(plugin_id)
			.ok_or_else(|| PluginError::NotFound(plugin_id.to_string()))?;

		// TODO: Call plugin_cleanup() if exported

		tracing::info!("✓ Plugin {} unloaded", plugin_id);

		Ok(())
	}

	/// Hot-reload a plugin (for development)
	pub async fn reload_plugin(&mut self, plugin_id: &str) -> Result<(), PluginError> {
		tracing::info!("Reloading plugin: {}", plugin_id);

		self.unload_plugin(plugin_id).await?;
		self.load_plugin(plugin_id).await?;

		tracing::info!("✓ Plugin {} reloaded", plugin_id);

		Ok(())
	}

	/// List all loaded plugins
	pub async fn list_plugins(&self) -> Vec<String> {
		self.plugins.read().await.keys().cloned().collect()
	}

	/// Get plugin manifest
	pub async fn get_manifest(&self, plugin_id: &str) -> Option<ExtensionManifest> {
		self.plugins
			.read()
			.await
			.get(plugin_id)
			.map(|p| p.manifest.clone())
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	// TODO: Add tests with a simple WASM module
	// Will implement once we have a test.wasm file
}
