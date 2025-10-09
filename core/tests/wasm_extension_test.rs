//! WASM Extension System Integration Test
//!
//! Tests that we can actually load and run WASM extensions.

use sd_core::Core;
use std::path::PathBuf;
use tempfile::TempDir;

#[tokio::test]
async fn test_load_wasm_extension() {
	// Initialize tracing for test output
	let _ = tracing_subscriber::fmt()
		.with_env_filter("debug,wasmer=info")
		.with_test_writer()
		.try_init();

	tracing::info!("🧪 Testing WASM extension loading");

	// 1. Initialize Core (same as other tests)
	let temp_dir = TempDir::new().unwrap();
	let core = Core::new_with_config(temp_dir.path().to_path_buf())
		.await
		.unwrap();

	tracing::info!("✅ Core initialized");

	// 2. Copy minimal test extension to Core's extensions directory
	let source_extensions = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
		.parent()
		.unwrap()
		.join("extensions/test-extension-minimal");

	let target_extensions = temp_dir.path().join("extensions/test-extension-minimal");
	std::fs::create_dir_all(&target_extensions).unwrap();

	// Copy manifest and WASM
	std::fs::copy(
		source_extensions.join("manifest.json"),
		target_extensions.join("manifest.json"),
	)
	.unwrap();

	std::fs::copy(
		source_extensions.join("test_extension_minimal.wasm"),
		target_extensions.join("test_extension_minimal.wasm"),
	)
	.unwrap();

	tracing::info!("✅ Extension files copied to temp directory");

	// 3. Get plugin manager
	let pm = core
		.plugin_manager
		.as_ref()
		.expect("PluginManager should be initialized");

	// 4. Load minimal test extension
	pm.write()
		.await
		.load_plugin("test-extension-minimal")
		.await
		.expect("Should load test-extension-minimal");

	tracing::info!("✅ Extension loaded!");

	// 5. Verify it's in the list
	let loaded = pm.read().await.list_plugins().await;
	assert!(
		loaded.contains(&"test-extension-minimal".to_string()),
		"test-extension-minimal should be in loaded plugins list"
	);

	// 6. Get manifest
	let manifest = pm
		.read()
		.await
		.get_manifest("test-extension-minimal")
		.await
		.expect("Should have manifest");

	assert_eq!(manifest.id, "test-extension-minimal");
	assert_eq!(manifest.name, "Minimal Test Extension");

	tracing::info!("✅ All checks passed!");
	tracing::info!("🎉 WASM extension system works!");
}
