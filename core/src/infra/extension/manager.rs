//! WASM Plugin Manager
//!
//! Manages the lifecycle of WASM extensions: discovery under the data
//! directory, loading, unloading, and running their registered jobs.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use chrono::Utc;
use thiserror::Error;
use tokio::sync::RwLock;
use wasmer::{imports, Function, FunctionEnv, Instance, Memory, Module, Store, TypedFunction};

use crate::{context::CoreContext, infra::api::ApiDispatcher};

use super::host_functions::{
	self, host_spacedrive_call, host_spacedrive_log, JobBridge, PluginEnv,
};
use super::job_registry::ExtensionJobRegistry;
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

	#[error("Plugin export missing or mistyped: {0}")]
	BadExport(String),

	#[error("WASM trap: {0}")]
	Trap(String),

	#[error("I/O error: {0}")]
	Io(#[from] std::io::Error),
}

/// A plugin's compiled module, its store and its instance, which must stay
/// together for the plugin's lifetime and are used from one thread at a time.
pub struct PluginRuntime {
	store: Store,
	instance: Instance,
	env: FunctionEnv<PluginEnv>,
}

impl PluginRuntime {
	/// Run one registered job export to completion on the calling thread.
	///
	/// The context and state JSON go through the guest's own allocator, so the
	/// guest reads them with plain slices. The bridge is installed for the
	/// duration of the call and removed after, whatever the guest did.
	/// Returns the guest's exit code: 0 completed, 1 interrupted, 2 failed.
	pub fn run_job(
		&mut self,
		export_fn: &str,
		ctx_json: &str,
		state_json: &str,
		bridge: JobBridge,
	) -> Result<i32, PluginError> {
		let store = &mut self.store;
		let exports = &self.instance.exports;
		let alloc: TypedFunction<i32, i32> = exports
			.get_typed_function(store, "wasm_alloc")
			.map_err(|e| PluginError::BadExport(format!("wasm_alloc: {e}")))?;
		let free: TypedFunction<(i32, i32), ()> = exports
			.get_typed_function(store, "wasm_free")
			.map_err(|e| PluginError::BadExport(format!("wasm_free: {e}")))?;
		let run: TypedFunction<(u32, u32, u32, u32), i32> = exports
			.get_typed_function(store, export_fn)
			.map_err(|e| PluginError::BadExport(format!("{export_fn}: {e}")))?;
		let memory = exports
			.get_memory("memory")
			.map_err(|e| PluginError::BadExport(format!("memory: {e}")))?
			.clone();

		let mut write_guest = |store: &mut Store, bytes: &[u8]| -> Result<i32, PluginError> {
			let len = bytes.len().max(1) as i32;
			let ptr = alloc
				.call(store, len)
				.map_err(|e| PluginError::Trap(format!("wasm_alloc: {e}")))?;
			memory
				.view(store)
				.write(ptr as u64, bytes)
				.map_err(|e| PluginError::Trap(format!("write guest memory: {e}")))?;
			Ok(ptr)
		};
		let ctx_ptr = write_guest(store, ctx_json.as_bytes())?;
		let state_ptr = write_guest(store, state_json.as_bytes())?;

		self.env.as_mut(store).current_job = Some(bridge);
		let result = run.call(
			store,
			ctx_ptr as u32,
			ctx_json.len() as u32,
			state_ptr as u32,
			state_json.len() as u32,
		);
		self.env.as_mut(store).current_job = None;

		// Best-effort: a leaked buffer costs guest memory until the plugin is
		// reloaded, never correctness.
		let _ = free.call(store, ctx_ptr, ctx_json.len().max(1) as i32);
		let _ = free.call(store, state_ptr, state_json.len().max(1) as i32);

		result.map_err(|e| PluginError::Trap(e.to_string()))
	}
}

/// Manages WASM plugin lifecycle
pub struct PluginManager {
	plugins: Arc<RwLock<HashMap<String, LoadedPlugin>>>,
	plugin_dir: PathBuf,
	core_context: Arc<CoreContext>,
	api_dispatcher: Arc<ApiDispatcher>,
	job_registry: Arc<ExtensionJobRegistry>,
}

impl PluginManager {
	/// Create new plugin manager
	pub fn new(
		plugin_dir: PathBuf,
		core_context: Arc<CoreContext>,
		api_dispatcher: Arc<ApiDispatcher>,
	) -> Self {
		Self {
			plugins: Arc::new(RwLock::new(HashMap::new())),
			plugin_dir,
			core_context,
			api_dispatcher,
			job_registry: Arc::new(ExtensionJobRegistry::new()),
		}
	}

	/// Get the job registry for extension jobs
	pub fn job_registry(&self) -> Arc<ExtensionJobRegistry> {
		self.job_registry.clone()
	}

	/// The directory extensions are installed under: `<data dir>/extensions`.
	pub fn plugin_dir(&self) -> &PathBuf {
		&self.plugin_dir
	}

	/// Load every extension installed under the plugin directory.
	///
	/// A directory is an extension when it holds a `manifest.json`. One broken
	/// extension is logged and skipped so it cannot keep the others, or the
	/// daemon, from starting. Returns the ids that loaded.
	pub async fn load_all(&mut self) -> Vec<String> {
		let mut dirs = match tokio::fs::read_dir(&self.plugin_dir).await {
			Ok(dirs) => dirs,
			Err(e) => {
				tracing::warn!(dir = %self.plugin_dir.display(), "Cannot read extensions directory: {e}");
				return Vec::new();
			}
		};

		let mut loaded = Vec::new();
		while let Ok(Some(entry)) = dirs.next_entry().await {
			if !entry.path().join("manifest.json").is_file() {
				continue;
			}
			let dir_name = entry.file_name().to_string_lossy().to_string();
			match self.load_plugin(&dir_name).await {
				Ok(id) => loaded.push(id),
				Err(e) => tracing::error!(extension = %dir_name, "Extension did not load: {e}"),
			}
		}
		loaded
	}

	/// Load a WASM plugin from its directory under the plugin directory.
	///
	/// Expected structure:
	/// ```text
	/// extensions/finance/
	///   ├── manifest.json
	///   └── finance.wasm
	/// ```
	///
	/// The plugin is keyed by the id in its manifest, which may differ from the
	/// directory name. Returns that id.
	pub async fn load_plugin(&mut self, dir_name: &str) -> Result<String, PluginError> {
		let plugin_path = self.plugin_dir.join(dir_name);
		let manifest_path = plugin_path.join("manifest.json");
		let manifest: ExtensionManifest = {
			let manifest_str = tokio::fs::read_to_string(&manifest_path)
				.await
				.map_err(|e| {
					PluginError::ManifestLoadFailed(format!("Failed to read manifest: {}", e))
				})?;

			serde_json::from_str(&manifest_str).map_err(|e| {
				PluginError::ManifestLoadFailed(format!("Failed to parse manifest: {}", e))
			})?
		};
		let plugin_id = manifest.id.clone();

		if self.plugins.read().await.contains_key(&plugin_id) {
			return Err(PluginError::AlreadyLoaded(plugin_id));
		}

		tracing::info!(
			extension = %plugin_id,
			"Loading extension {} v{}",
			manifest.name,
			manifest.version
		);

		let wasm_path = plugin_path.join(&manifest.wasm_file);
		let wasm_bytes = tokio::fs::read(&wasm_path).await?;

		// Each plugin owns a store so one plugin's job never holds another's
		// instance.
		let mut store = Store::default();
		let module = Module::new(&store, wasm_bytes).map_err(|e| {
			PluginError::CompilationFailed(format!("Failed to compile WASM: {}", e))
		})?;

		let permissions =
			ExtensionPermissions::from_manifest(manifest.id.clone(), &manifest.permissions);

		// Placeholder memory until the instance exists; host functions only run
		// after it is swapped for the real export.
		let temp_memory = Memory::new(&mut store, wasmer::MemoryType::new(1, None, false))
			.map_err(|e| {
				PluginError::InstantiationFailed(format!("Failed to create temp memory: {}", e))
			})?;

		let plugin_env = PluginEnv {
			extension_id: manifest.id.clone(),
			core_context: self.core_context.clone(),
			api_dispatcher: self.api_dispatcher.clone(),
			permissions,
			memory: temp_memory,
			job_registry: self.job_registry.clone(),
			current_job: None,
		};

		let env = FunctionEnv::new(&mut store, plugin_env);

		let import_object = imports! {
			"spacedrive" => {
				"spacedrive_call" => Function::new_typed_with_env(
					&mut store,
					&env,
					host_spacedrive_call
				),
				"spacedrive_log" => Function::new_typed_with_env(
					&mut store,
					&env,
					host_spacedrive_log
				),
				"job_report_progress" => Function::new_typed_with_env(
					&mut store,
					&env,
					host_functions::host_job_report_progress
				),
				"job_checkpoint" => Function::new_typed_with_env(
					&mut store,
					&env,
					host_functions::host_job_checkpoint
				),
				"job_check_interrupt" => Function::new_typed_with_env(
					&mut store,
					&env,
					host_functions::host_job_check_interrupt
				),
				"job_add_warning" => Function::new_typed_with_env(
					&mut store,
					&env,
					host_functions::host_job_add_warning
				),
				"job_increment_bytes" => Function::new_typed_with_env(
					&mut store,
					&env,
					host_functions::host_job_increment_bytes
				),
				"job_increment_items" => Function::new_typed_with_env(
					&mut store,
					&env,
					host_functions::host_job_increment_items
				),
				"register_job" => Function::new_typed_with_env(
					&mut store,
					&env,
					host_functions::host_register_job
				),
			}
		};

		let instance = Instance::new(&mut store, &module, &import_object).map_err(|e| {
			PluginError::InstantiationFailed(format!("Failed to instantiate WASM: {}", e))
		})?;

		let memory = instance.exports.get_memory("memory").map_err(|e| {
			PluginError::InstantiationFailed(format!("Plugin missing memory export: {}", e))
		})?;
		env.as_mut(&mut store).memory = memory.clone();

		// plugin_init registers the plugin's jobs through host_register_job.
		match instance.exports.get_function("plugin_init") {
			Ok(init_fn) => {
				if let Err(e) = init_fn.call(&mut store, &[]) {
					self.job_registry.unregister_extension_jobs(&plugin_id);
					return Err(PluginError::InstantiationFailed(format!(
						"plugin_init() failed: {}",
						e
					)));
				}
			}
			Err(_) => {
				tracing::warn!(extension = %plugin_id, "Plugin has no plugin_init() function")
			}
		}

		let jobs = self.job_registry.list_jobs_for_extension(&plugin_id);
		tracing::info!(
			extension = %plugin_id,
			jobs = ?jobs.iter().map(|j| j.job_name.as_str()).collect::<Vec<_>>(),
			"Extension loaded"
		);

		self.plugins.write().await.insert(
			plugin_id.clone(),
			LoadedPlugin {
				id: plugin_id.clone(),
				manifest,
				loaded_at: Utc::now(),
				runtime: Arc::new(Mutex::new(PluginRuntime {
					store,
					instance,
					env,
				})),
			},
		);

		Ok(plugin_id)
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
		self.job_registry.unregister_extension_jobs(plugin_id);

		if let Ok(mut runtime) = plugin.runtime.lock() {
			if let Ok(cleanup) = runtime.instance.exports.get_function("plugin_cleanup") {
				let cleanup = cleanup.clone();
				if let Err(e) = cleanup.call(&mut runtime.store, &[]) {
					tracing::warn!(extension = %plugin_id, "plugin_cleanup() failed: {e}");
				}
			}
		}

		Ok(())
	}

	/// Reload a plugin from its directory, for development.
	pub async fn reload_plugin(&mut self, plugin_id: &str) -> Result<(), PluginError> {
		let dir_name = self
			.plugins
			.read()
			.await
			.get(plugin_id)
			.map(|_| plugin_id.to_string())
			.ok_or_else(|| PluginError::NotFound(plugin_id.to_string()))?;
		self.unload_plugin(plugin_id).await?;
		self.load_plugin(&dir_name).await?;
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

	/// The runtime of a loaded plugin, for running one of its jobs.
	pub async fn runtime(&self, plugin_id: &str) -> Option<Arc<Mutex<PluginRuntime>>> {
		self.plugins
			.read()
			.await
			.get(plugin_id)
			.map(|p| p.runtime.clone())
	}
}
