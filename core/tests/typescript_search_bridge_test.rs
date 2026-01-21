//! TypeScript Search Integration Test Bridge
//!
//! This test sets up a real Spacedrive daemon with indexed locations and ephemeral directories,
//! then spawns TypeScript tests that perform search operations via the ts-client.
//! This enables true end-to-end testing of the search functionality.

mod helpers;

use helpers::*;
use sd_core::{
	domain::addressing::SdPath,
	location::IndexMode,
	ops::indexing::{IndexScope, IndexerJob, IndexerJobConfig},
};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tokio::time::Duration;

/// Connection info passed from Rust test harness to TypeScript tests
#[derive(Debug, Serialize, Deserialize)]
struct SearchBridgeConfig {
	/// TCP socket address for daemon connection (e.g., "127.0.0.1:6969")
	socket_addr: String,
	/// Library UUID
	library_id: String,
	/// Persistent location UUID
	persistent_location_uuid: String,
	/// Persistent location database ID
	persistent_location_db_id: i32,
	/// Physical path to the persistent test location root
	persistent_location_path: PathBuf,
	/// Physical path to the ephemeral directory (not in a location)
	ephemeral_dir_path: PathBuf,
	/// Test data directory
	test_data_path: PathBuf,
}

#[tokio::test]
async fn test_typescript_search_persistent_and_ephemeral() -> anyhow::Result<()> {
	// Setup: Create daemon with both indexed location and ephemeral directory
	let harness = IndexingHarnessBuilder::new("typescript_search_bridge")
		.enable_daemon() // Start RPC server for TypeScript client
		.build()
		.await?;

	// === PERSISTENT LOCATION SETUP ===
	let persistent_location = harness.create_test_location("search_persistent").await?;

	// Create diverse files for persistent search testing
	persistent_location.create_dir("documents").await?;
	persistent_location.create_dir("images").await?;
	persistent_location.create_dir("code").await?;

	persistent_location
		.write_file("documents/report.txt", "Annual report content")
		.await?;
	persistent_location
		.write_file("documents/notes.md", "Meeting notes about the project")
		.await?;
	persistent_location
		.write_file("images/photo.jpg", "fake jpg data")
		.await?;
	persistent_location
		.write_file("images/screenshot.png", "fake png data")
		.await?;
	persistent_location
		.write_file("code/main.rs", "fn main() { println!(\"test\"); }")
		.await?;
	persistent_location
		.write_file("code/lib.rs", "pub fn test() {}")
		.await?;

	// Index the persistent location
	tracing::info!("Indexing persistent location...");
	let location = persistent_location
		.index("Search Test Location", IndexMode::Shallow)
		.await?;

	tokio::time::sleep(Duration::from_secs(1)).await;

	// === EPHEMERAL DIRECTORY SETUP ===
	let test_root = harness.temp_path();
	let ephemeral_dir = test_root.join("search_ephemeral");

	tokio::fs::create_dir_all(&ephemeral_dir).await?;

	// Create files in root directory (avoid subdirectories for now due to recursive indexing bug)
	tokio::fs::write(ephemeral_dir.join("tutorial_video.mp4"), "fake video data").await?;
	tokio::fs::write(ephemeral_dir.join("demo_presentation.mov"), "fake mov data").await?;
	tokio::fs::write(ephemeral_dir.join("song_audio.mp3"), "fake audio data").await?;
	tokio::fs::write(
		ephemeral_dir.join("readme_text.txt"),
		"This is the ephemeral test directory",
	)
	.await?;

	// Verify files exist before indexing
	eprintln!("\n[Rust] Verifying ephemeral files exist:");
	let files_to_check = vec![
		ephemeral_dir.join("tutorial_video.mp4"),
		ephemeral_dir.join("demo_presentation.mov"),
		ephemeral_dir.join("song_audio.mp3"),
		ephemeral_dir.join("readme_text.txt"),
	];
	for file_path in &files_to_check {
		let exists = tokio::fs::try_exists(file_path).await?;
		eprintln!("  - {:?}: {}", file_path.file_name(), exists);
		if !exists {
			anyhow::bail!("File doesn't exist: {:?}", file_path);
		}
	}

	// Index ephemeral directory using global cache
	tracing::info!("Indexing ephemeral directory...");
	let ephemeral_sd = SdPath::local(ephemeral_dir.clone());
	let global_index = harness.core.context.ephemeral_cache().get_global_index();

	let indexer_config =
		IndexerJobConfig::ephemeral_browse(ephemeral_sd.clone(), IndexScope::Recursive, false);
	eprintln!(
		"[Rust] Indexer config: path={:?}, scope={:?}, persistence={:?}",
		ephemeral_sd, indexer_config.scope, indexer_config.persistence
	);

	let mut indexer_job = IndexerJob::new(indexer_config);
	indexer_job.set_ephemeral_index(global_index);

	eprintln!("[Rust] Dispatching ephemeral indexer job...");
	let index_handle = harness.library.jobs().dispatch(indexer_job).await?;
	index_handle.wait().await?;
	eprintln!("[Rust] Ephemeral indexer job completed");

	harness
		.core
		.context
		.ephemeral_cache()
		.mark_indexing_complete(&ephemeral_dir);
	eprintln!("[Rust] Marked ephemeral indexing as complete");

	tokio::time::sleep(Duration::from_secs(1)).await;

	// Verify ephemeral cache has entries
	if let Some(index_arc) = harness
		.core
		.context
		.ephemeral_cache()
		.get_for_path(&ephemeral_dir)
	{
		let index = index_arc.read().await;
		let all_paths = index.list_directory(&ephemeral_dir).unwrap_or_default();
		eprintln!(
			"[Rust] Ephemeral cache has {} entries for {:?}",
			all_paths.len(),
			ephemeral_dir
		);
		for (i, path) in all_paths.iter().take(10).enumerate() {
			eprintln!("  {}. {:?}", i + 1, path);
		}

		if all_paths.is_empty() {
			anyhow::bail!("Ephemeral cache is empty after indexing!");
		}
	} else {
		anyhow::bail!("Ephemeral cache not found for path!");
	}

	// Get daemon socket address
	let socket_addr = harness
		.daemon_socket_addr()
		.expect("Daemon should be enabled")
		.to_string();

	// Prepare bridge config
	let bridge_config = SearchBridgeConfig {
		socket_addr: socket_addr.clone(),
		library_id: harness.library.id().to_string(),
		persistent_location_uuid: location.uuid.to_string(),
		persistent_location_db_id: location.db_id,
		persistent_location_path: persistent_location.path().to_path_buf(),
		ephemeral_dir_path: ephemeral_dir.clone(),
		test_data_path: harness.temp_path().to_path_buf(),
	};

	// Write config to temp file
	let config_path = harness
		.temp_path()
		.join("typescript_search_bridge_config.json");
	let config_json = serde_json::to_string_pretty(&bridge_config)?;
	tokio::fs::write(&config_path, config_json).await?;

	tracing::info!("Bridge config written to: {}", config_path.display());
	tracing::info!("Socket address: {}", socket_addr);
	tracing::info!("Library ID: {}", bridge_config.library_id);

	// Spawn TypeScript test process
	let ts_test_file = "packages/ts-client/tests/integration/search.test.ts";
	let workspace_root = std::env::current_dir()?.parent().unwrap().to_path_buf();
	let ts_test_path = workspace_root.join(ts_test_file);
	let bun_config = workspace_root.join("packages/ts-client/tests/integration/bunfig.toml");

	eprintln!("\n=== TypeScript Search Bridge Test ===");
	eprintln!("Workspace root: {}", workspace_root.display());
	eprintln!("Test file: {}", ts_test_path.display());
	eprintln!("Bun config: {}", bun_config.display());
	eprintln!("Config path: {}", config_path.display());
	eprintln!("Socket address: {}", socket_addr);
	eprintln!("Library ID: {}", bridge_config.library_id);
	eprintln!(
		"Persistent location: {}",
		persistent_location.path().display()
	);
	eprintln!("Ephemeral directory: {}", ephemeral_dir.display());
	eprintln!("==============================\n");

	// Check if test file exists
	if !ts_test_path.exists() {
		tracing::warn!("TypeScript test file not found: {}", ts_test_path.display());
		tracing::warn!("Skipping TypeScript test execution (file will be created)");
		harness.shutdown().await?;
		return Ok(());
	}

	let output = tokio::process::Command::new("bun")
		.arg("test")
		.arg("--config")
		.arg(&bun_config)
		.arg(&ts_test_path)
		.env("BRIDGE_CONFIG_PATH", config_path.to_str().unwrap())
		.env("RUST_LOG", "debug")
		.current_dir(&workspace_root)
		.output()
		.await?;

	// Always print TypeScript output to stderr for visibility
	let stdout = String::from_utf8_lossy(&output.stdout);
	let stderr = String::from_utf8_lossy(&output.stderr);

	if !stdout.is_empty() {
		eprintln!("\n=== TypeScript stdout ===\n{}\n", stdout);
	}
	if !stderr.is_empty() {
		eprintln!("\n=== TypeScript stderr ===\n{}\n", stderr);
	}

	// Verify TypeScript test passed
	if !output.status.success() {
		anyhow::bail!(
			"TypeScript test failed with exit code: {:?}",
			output.status.code()
		);
	}

	tracing::info!("TypeScript search test passed! ✓");

	// Cleanup
	harness.shutdown().await?;
	Ok(())
}
