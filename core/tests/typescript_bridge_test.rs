//! TypeScript Integration Test Bridge
//!
//! This test harness sets up a real Spacedrive daemon with indexed locations,
//! then spawns TypeScript tests that interact with it via the ts-client.
//! This enables true end-to-end testing across the Rust backend and TypeScript frontend.
//!
//! ## Architecture
//!
//! 1. Rust test creates daemon + indexed location using IndexingHarnessBuilder
//! 2. Connection info (socket path, library ID) written to JSON file
//! 3. Rust spawns `bun test` with specific TypeScript test file
//! 4. TypeScript test reads connection info, connects to daemon via ts-client
//! 5. TypeScript test performs file operations and cache assertions
//! 6. Rust validates test exit code and cleans up
//!
//! ## Running
//!
//! ```bash
//! cargo test typescript_bridge -- --nocapture
//! ```

mod helpers;

use helpers::*;
use sd_core::location::IndexMode;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tokio::time::Duration;

/// Connection info passed from Rust test harness to TypeScript tests
#[derive(Debug, Serialize, Deserialize)]
struct TestBridgeConfig {
	/// TCP socket address for daemon connection (e.g., "127.0.0.1:6969")
	socket_addr: String,
	/// Library UUID
	library_id: String,
	/// Location database ID
	location_db_id: i32,
	/// Physical path to the test location root
	location_path: PathBuf,
	/// Test data directory (for file operations)
	test_data_path: PathBuf,
}

#[tokio::test]
async fn test_typescript_use_normalized_query_with_file_moves() -> anyhow::Result<()> {
	// Setup: Create daemon with indexed location
	let harness = IndexingHarnessBuilder::new("typescript_bridge_file_moves")
		.enable_daemon() // Start RPC server for TypeScript client
		.build()
		.await?;

	let test_location = harness.create_test_location("test_moves").await?;

	// Create initial folder structure
	test_location.create_dir("folder_a").await?;
	test_location.create_dir("folder_b").await?;
	test_location
		.write_file("folder_a/file1.txt", "Content 1")
		.await?;
	test_location
		.write_file("folder_a/file2.rs", "fn main() {}")
		.await?;
	test_location
		.write_file("folder_b/file3.md", "# Docs")
		.await?;

	// Index the location
	let location = test_location
		.index("TypeScript Test Location", IndexMode::Shallow)
		.await?;

	// Wait for indexing to complete
	tokio::time::sleep(Duration::from_secs(1)).await;

	// Get daemon socket address
	let socket_addr = harness
		.daemon_socket_addr()
		.expect("Daemon should be enabled")
		.to_string();

	// Prepare bridge config
	let bridge_config = TestBridgeConfig {
		socket_addr: socket_addr.clone(),
		library_id: harness.library.id().to_string(),
		location_db_id: location.db_id,
		location_path: test_location.path().to_path_buf(),
		test_data_path: harness.temp_path().to_path_buf(),
	};

	// Write config to temp file for TypeScript to read
	let config_path = harness.temp_path().join("typescript_bridge_config.json");
	let config_json = serde_json::to_string_pretty(&bridge_config)?;
	tokio::fs::write(&config_path, config_json).await?;

	tracing::info!("Bridge config written to: {}", config_path.display());
	tracing::info!("Socket address: {}", socket_addr);
	tracing::info!("Library ID: {}", bridge_config.library_id);

	// Spawn TypeScript test process
	let ts_test_file = "packages/ts-client/tests/integration/useNormalizedQuery.test.ts";
	let workspace_root = std::env::current_dir()?.parent().unwrap().to_path_buf();
	let ts_test_path = workspace_root.join(ts_test_file);
	let bun_config = workspace_root.join("packages/ts-client/tests/integration/bunfig.toml");

	eprintln!("\n=== TypeScript Bridge Test ===");
	eprintln!("Workspace root: {}", workspace_root.display());
	eprintln!("Test file: {}", ts_test_path.display());
	eprintln!("Bun config: {}", bun_config.display());
	eprintln!("Config path: {}", config_path.display());
	eprintln!("Socket address: {}", socket_addr);
	eprintln!("Library ID: {}", bridge_config.library_id);
	eprintln!("==============================\n");

	// Check if test file exists
	if !ts_test_path.exists() {
		anyhow::bail!("TypeScript test file not found: {}", ts_test_path.display());
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

	tracing::info!("TypeScript test passed! ✓");

	// Cleanup
	harness.shutdown().await?;
	Ok(())
}

#[tokio::test]
async fn test_typescript_use_normalized_query_with_folder_renames() -> anyhow::Result<()> {
	// Setup: Create daemon with indexed location
	let harness = IndexingHarnessBuilder::new("typescript_bridge_folder_renames")
		.enable_daemon() // Start RPC server for TypeScript client
		.build()
		.await?;

	let test_location = harness.create_test_location("test_renames").await?;

	// Create initial folder structure
	test_location.create_dir("original_folder").await?;
	test_location
		.write_file("original_folder/file1.txt", "Content 1")
		.await?;
	test_location
		.write_file("original_folder/file2.rs", "fn main() {}")
		.await?;
	test_location
		.write_file("original_folder/nested/file3.md", "# Docs")
		.await?;

	// Index the location
	let location = test_location
		.index("TypeScript Test Location", IndexMode::Shallow)
		.await?;

	// Wait for indexing to complete
	tokio::time::sleep(Duration::from_secs(1)).await;

	// Get daemon socket address
	let socket_addr = harness
		.daemon_socket_addr()
		.expect("Daemon should be enabled")
		.to_string();

	// Prepare bridge config
	let bridge_config = TestBridgeConfig {
		socket_addr: socket_addr.clone(),
		library_id: harness.library.id().to_string(),
		location_db_id: location.db_id,
		location_path: test_location.path().to_path_buf(),
		test_data_path: harness.temp_path().to_path_buf(),
	};

	// Write config to temp file
	let config_path = harness.temp_path().join("typescript_bridge_config.json");
	let config_json = serde_json::to_string_pretty(&bridge_config)?;
	tokio::fs::write(&config_path, config_json).await?;

	tracing::info!("Bridge config written to: {}", config_path.display());

	// Spawn TypeScript test process
	let ts_test_file =
		"packages/ts-client/tests/integration/useNormalizedQuery.folder-rename.test.ts";
	let workspace_root = std::env::current_dir()?.parent().unwrap().to_path_buf();
	let ts_test_path = workspace_root.join(ts_test_file);
	let bun_config = workspace_root.join("packages/ts-client/tests/integration/bunfig.toml");

	eprintln!("\n=== TypeScript Bridge Test ===");
	eprintln!("Workspace root: {}", workspace_root.display());
	eprintln!("Test file: {}", ts_test_path.display());
	eprintln!("Bun config: {}", bun_config.display());
	eprintln!("Config path: {}", config_path.display());
	eprintln!("Socket address: {}", socket_addr);
	eprintln!("Library ID: {}", bridge_config.library_id);
	eprintln!("==============================\n");

	// Check if test file exists
	if !ts_test_path.exists() {
		anyhow::bail!("TypeScript test file not found: {}", ts_test_path.display());
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

	// Verify test passed
	if !output.status.success() {
		anyhow::bail!(
			"TypeScript test failed with exit code: {:?}",
			output.status.code()
		);
	}

	tracing::info!("TypeScript test passed! ✓");

	harness.shutdown().await?;
	Ok(())
}

#[tokio::test]
async fn test_typescript_use_normalized_query_with_bulk_moves() -> anyhow::Result<()> {
	// Setup: Create daemon with indexed location
	let harness = IndexingHarnessBuilder::new("typescript_bridge_bulk_moves")
		.enable_daemon() // Start RPC server for TypeScript client
		.build()
		.await?;

	let test_location = harness.create_test_location("test_bulk").await?;

	// Create subfolder with 20 files
	// Mix of text files and files that will get content identity
	test_location.create_dir("bulk_test").await?;
	for i in 1..=20 {
		if i <= 10 {
			// First 10: simple text files (likely Physical paths)
			test_location
				.write_file(
					&format!("bulk_test/file{:02}.txt", i),
					&format!("Content of file {}", i),
				)
				.await?;
		} else {
			// Last 10: larger files more likely to get content identity
			// Create files with more content to trigger content identification
			let content = format!(
				"# File {}\n{}",
				i,
				"Lorem ipsum dolor sit amet. ".repeat(100)
			);
			test_location
				.write_file(&format!("bulk_test/file{:02}.md", i), &content)
				.await?;
		}
	}

	// Also create a couple files in root to verify they're not affected
	test_location
		.write_file("root_file1.md", "# Root file")
		.await?;
	test_location
		.write_file("root_file2.rs", "fn main() {}")
		.await?;

	// Index the location with Content mode to enable content identification
	// Shallow mode only indexes metadata; Content mode computes hashes and creates content identity
	// This is critical for testing the cache update bug with content-addressed files
	tracing::info!("Starting indexing with Content mode (includes content identification)...");
	let location = test_location
		.index("TypeScript Bulk Test Location", IndexMode::Content)
		.await?;

	tracing::info!("Indexing completed, waiting for content identification to settle...");
	// Wait extra time for content identification and event processing
	tokio::time::sleep(Duration::from_secs(5)).await;

	tracing::info!("Ready to start TypeScript test");

	// Get daemon socket address
	let socket_addr = harness
		.daemon_socket_addr()
		.expect("Daemon should be enabled")
		.to_string();

	// Prepare bridge config
	let bridge_config = TestBridgeConfig {
		socket_addr: socket_addr.clone(),
		library_id: harness.library.id().to_string(),
		location_db_id: location.db_id,
		location_path: test_location.path().to_path_buf(),
		test_data_path: harness.temp_path().to_path_buf(),
	};

	// Write config to temp file
	let config_path = harness.temp_path().join("typescript_bridge_config.json");
	let config_json = serde_json::to_string_pretty(&bridge_config)?;
	tokio::fs::write(&config_path, config_json).await?;

	tracing::info!("Bridge config written to: {}", config_path.display());

	// Spawn TypeScript test process - use dedicated bulk moves test file
	let ts_test_file = "packages/ts-client/tests/integration/useNormalizedQuery.bulk-moves.test.ts";
	let workspace_root = std::env::current_dir()?.parent().unwrap().to_path_buf();
	let ts_test_path = workspace_root.join(ts_test_file);
	let bun_config = workspace_root.join("packages/ts-client/tests/integration/bunfig.toml");

	eprintln!("\n=== TypeScript Bridge Test (Bulk Moves) ===");
	eprintln!("Workspace root: {}", workspace_root.display());
	eprintln!("Test file: {}", ts_test_path.display());
	eprintln!("Bun config: {}", bun_config.display());
	eprintln!("Config path: {}", config_path.display());
	eprintln!("Socket address: {}", socket_addr);
	eprintln!("Library ID: {}", bridge_config.library_id);
	eprintln!("==============================\n");

	// Check if test file exists
	if !ts_test_path.exists() {
		anyhow::bail!("TypeScript test file not found: {}", ts_test_path.display());
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

	// Verify test passed
	if !output.status.success() {
		anyhow::bail!(
			"TypeScript test failed with exit code: {:?}",
			output.status.code()
		);
	}

	tracing::info!("TypeScript bulk move test passed! ✓");

	harness.shutdown().await?;
	Ok(())
}

#[tokio::test]
async fn test_typescript_use_normalized_query_with_file_deletes() -> anyhow::Result<()> {
	// Setup: Create daemon with indexed location
	let harness = IndexingHarnessBuilder::new("typescript_bridge_file_deletes")
		.enable_daemon() // Start RPC server for TypeScript client
		.build()
		.await?;

	let test_location = harness.create_test_location("test_deletes").await?;

	// Create delete_test folder with files to delete
	test_location.create_dir("delete_test").await?;
	test_location
		.write_file("delete_test/file1.txt", "Content 1")
		.await?;
	test_location
		.write_file("delete_test/file2.rs", "fn main() {}")
		.await?;
	test_location
		.write_file("delete_test/file3.md", "# Docs")
		.await?;
	test_location
		.write_file("delete_test/file4.json", r#"{"data": "test"}"#)
		.await?;
	test_location
		.write_file("delete_test/file5.txt", "Extra file")
		.await?;

	// Index the location
	let location = test_location
		.index("TypeScript Test Location", IndexMode::Shallow)
		.await?;

	// Wait for indexing to complete
	tokio::time::sleep(Duration::from_secs(1)).await;

	// Get daemon socket address
	let socket_addr = harness
		.daemon_socket_addr()
		.expect("Daemon should be enabled")
		.to_string();

	// Prepare bridge config
	let bridge_config = TestBridgeConfig {
		socket_addr: socket_addr.clone(),
		library_id: harness.library.id().to_string(),
		location_db_id: location.db_id,
		location_path: test_location.path().to_path_buf(),
		test_data_path: harness.temp_path().to_path_buf(),
	};

	// Write config to temp file
	let config_path = harness.temp_path().join("typescript_bridge_config.json");
	let config_json = serde_json::to_string_pretty(&bridge_config)?;
	tokio::fs::write(&config_path, config_json).await?;

	tracing::info!("Bridge config written to: {}", config_path.display());

	// Spawn TypeScript test process
	let ts_test_file =
		"packages/ts-client/tests/integration/useNormalizedQuery.file-delete.test.ts";
	let workspace_root = std::env::current_dir()?.parent().unwrap().to_path_buf();
	let ts_test_path = workspace_root.join(ts_test_file);
	let bun_config = workspace_root.join("packages/ts-client/tests/integration/bunfig.toml");

	eprintln!("\n=== TypeScript Bridge Test ===");
	eprintln!("Workspace root: {}", workspace_root.display());
	eprintln!("Test file: {}", ts_test_path.display());
	eprintln!("Bun config: {}", bun_config.display());
	eprintln!("Config path: {}", config_path.display());
	eprintln!("Socket address: {}", socket_addr);
	eprintln!("Library ID: {}", bridge_config.library_id);
	eprintln!("==============================\n");

	// Check if test file exists
	if !ts_test_path.exists() {
		anyhow::bail!("TypeScript test file not found: {}", ts_test_path.display());
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

	// Verify test passed
	if !output.status.success() {
		anyhow::bail!(
			"TypeScript test failed with exit code: {:?}",
			output.status.code()
		);
	}

	tracing::info!("TypeScript test passed! ✓");

	harness.shutdown().await?;
	Ok(())
}
