//! TypeScript Search Integration Test Bridge
//!
//! This test sets up a real Spacedrive daemon with a tracked source and a directory indexed by
//! browsing alone, then spawns TypeScript tests that perform search operations via the ts-client.
//! This enables true end-to-end testing of the search functionality.

mod helpers;

use helpers::*;
use sd_core::ops::indexing::IndexScope;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Connection info passed from Rust test harness to TypeScript tests
#[derive(Debug, Serialize, Deserialize)]
struct SearchBridgeConfig {
	/// TCP socket address for daemon connection (e.g., "127.0.0.1:6969")
	socket_addr: String,
	/// Library UUID
	library_id: String,
	/// The daemon's device slug, which physical paths are addressed by
	device_slug: String,
	/// Physical path to the tracked source root
	source_path: PathBuf,
	/// Physical path to the directory indexed by browsing, not tracked
	browsed_dir_path: PathBuf,
	/// Test data directory
	test_data_path: PathBuf,
}

#[tokio::test]
async fn test_typescript_search_tracked_and_browsed() -> anyhow::Result<()> {
	// Setup: Create daemon with a tracked source and a browsed directory
	let harness = IndexingHarnessBuilder::new("typescript_search_bridge")
		.enable_daemon() // Start RPC server for TypeScript client
		.build()
		.await?;

	// === TRACKED SOURCE SETUP ===
	let source = harness.create_test_dir("search_source").await?;

	// Create diverse files for library-wide search testing
	source.create_dir("documents").await?;
	source.create_dir("images").await?;
	source.create_dir("code").await?;

	source
		.write_file("documents/report.txt", "Annual report content")
		.await?;
	source
		.write_file("documents/notes.md", "Meeting notes about the project")
		.await?;
	source
		.write_file("images/photo.jpg", "fake jpg data")
		.await?;
	source
		.write_file("images/screenshot.png", "fake png data")
		.await?;
	source
		.write_file("code/main.rs", "fn main() { println!(\"test\"); }")
		.await?;
	source.write_file("code/lib.rs", "pub fn test() {}").await?;

	tracing::info!("Tracking the source...");
	source.track().await?;

	// === BROWSED DIRECTORY SETUP ===
	let test_root = harness.temp_path();
	let browsed_dir = test_root.join("search_browsed");

	tokio::fs::create_dir_all(&browsed_dir).await?;

	// Create files in root directory (avoid subdirectories for now due to recursive indexing bug)
	tokio::fs::write(browsed_dir.join("tutorial_video.mp4"), "fake video data").await?;
	tokio::fs::write(browsed_dir.join("demo_presentation.mov"), "fake mov data").await?;
	tokio::fs::write(browsed_dir.join("song_audio.mp3"), "fake audio data").await?;
	tokio::fs::write(
		browsed_dir.join("readme_text.txt"),
		"This is the browsed test directory",
	)
	.await?;

	// Verify files exist before indexing
	eprintln!("\n[Rust] Verifying browsed files exist:");
	let files_to_check = vec![
		browsed_dir.join("tutorial_video.mp4"),
		browsed_dir.join("demo_presentation.mov"),
		browsed_dir.join("song_audio.mp3"),
		browsed_dir.join("readme_text.txt"),
	];
	for file_path in &files_to_check {
		let exists = tokio::fs::try_exists(file_path).await?;
		eprintln!("  - {:?}: {}", file_path.file_name(), exists);
		if !exists {
			anyhow::bail!("File doesn't exist: {:?}", file_path);
		}
	}

	tracing::info!("Indexing the browsed directory...");
	harness
		.index_dir(&browsed_dir, IndexScope::Recursive)
		.await?;

	// Get daemon socket address
	let socket_addr = harness
		.daemon_socket_addr()
		.expect("Daemon should be enabled")
		.to_string();

	// Prepare bridge config
	let bridge_config = SearchBridgeConfig {
		socket_addr: socket_addr.clone(),
		library_id: harness.library.id().to_string(),
		device_slug: sd_core::device::get_current_device_slug(),
		source_path: source.path().to_path_buf(),
		browsed_dir_path: browsed_dir.clone(),
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
	eprintln!("Tracked source: {}", source.path().display());
	eprintln!("Browsed directory: {}", browsed_dir.display());
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
