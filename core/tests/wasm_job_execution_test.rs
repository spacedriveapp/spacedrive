//! Test WASM job execution
//!
//! Tests that we can dispatch and execute WASM jobs

use sd_core::Core;
use std::path::PathBuf;
use tempfile::TempDir;

#[tokio::test]
async fn test_dispatch_wasm_job() {
	// Initialize tracing
	let _ = tracing_subscriber::fmt()
		.with_env_filter("info,sd_core::infra::extension=debug")
		.with_test_writer()
		.try_init();

	tracing::info!("🧪 Testing WASM job execution");

	// 1. Initialize Core
	let temp_dir = TempDir::new().unwrap();
	let core = Core::new_with_config(temp_dir.path().to_path_buf())
		.await
		.unwrap();

	// 2. Get the default library that Core creates
	// (Avoids database migration issues in tests)
	tokio::time::sleep(tokio::time::Duration::from_millis(100)).await; // Let Core finish initializing
	let libraries = core.libraries.list().await;
	let library = libraries
		.first()
		.expect("Core should create default library")
		.clone();

	tracing::info!("✅ Core and library initialized (using default library)");

	// 3. Load the test extension first!
	let pm = core
		.plugin_manager
		.as_ref()
		.expect("PluginManager should exist");

	let source_ext = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
		.parent()
		.unwrap()
		.join("extensions/test-extension");

	let target_ext = temp_dir.path().join("extensions/test-extension");
	std::fs::create_dir_all(&target_ext).unwrap();
	std::fs::copy(
		source_ext.join("manifest.json"),
		target_ext.join("manifest.json"),
	)
	.unwrap();
	std::fs::copy(
		source_ext.join("test_extension.wasm"),
		target_ext.join("test_extension.wasm"),
	)
	.unwrap();

	pm.write()
		.await
		.load_plugin("test-extension")
		.await
		.expect("Should load extension");

	tracing::info!("✅ Extension loaded");

	// 4. Dispatch job by name (auto-registered as "test-extension:counter")
	let job_handle = library
		.jobs()
		.dispatch_by_name(
			"test-extension:counter",
			serde_json::json!({
				"current": 0,
				"target": 10,
				"processed": []
			}),
		)
		.await
		.expect("Should dispatch extension job by name");

	tracing::info!("✅ WASM job dispatched: {}", job_handle.id());

	// 5. Wait for completion
	job_handle.wait().await.expect("Job should complete");

	tracing::info!("✅ WASM job completed!");
	tracing::info!("🎉 WASM job execution works!");
}
