//! Plugin Manager Demo
//!
//! Demonstrates loading and managing WASM extensions.
//!
//! Run with:
//!   cargo run --example plugin_manager_demo

use std::path::PathBuf;
use std::sync::Arc;

use sd_core::infra::extension::PluginManager;
use sd_core::Core;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
	// Initialize tracing
	tracing_subscriber::fmt().with_env_filter("debug").init();

	tracing::info!("Plugin Manager Demo Starting...");

	// Create a minimal Core instance (in a real app, this would be fully initialized)
	// For now, we'll need to mock this or use a test core
	tracing::warn!("Note: This example requires a fully initialized Core instance");
	tracing::warn!("Will be functional once Core initialization is added");

	// Example usage (commented out until Core is ready):
	/*
	let core = Arc::new(Core::new(...).await?);

	// Create plugin manager pointing to extensions directory
	let extensions_dir = PathBuf::from("./extensions");
	let mut pm = PluginManager::new(core.clone(), extensions_dir);

	// Load the test extension
	tracing::info!("Loading test-extension...");
	pm.load_plugin("test-extension").await?;

	tracing::info!("✓ Test extension loaded successfully!");

	// List loaded plugins
	let loaded = pm.list_plugins().await;
	tracing::info!("Loaded plugins: {:?}", loaded);

	// Get manifest
	if let Some(manifest) = pm.get_manifest("test-extension").await {
		tracing::info!("Extension: {} v{}", manifest.name, manifest.version);
		tracing::info!("Permissions: {:?}", manifest.permissions.methods);
	}

	// Hot-reload (for development)
	tracing::info!("Testing hot-reload...");
	pm.reload_plugin("test-extension").await?;
	tracing::info!("✓ Hot-reload successful!");

	// Unload
	pm.unload_plugin("test-extension").await?;
	tracing::info!("✓ Extension unloaded");
	*/

	tracing::info!("Demo complete - see commented code for actual usage");

	Ok(())
}
