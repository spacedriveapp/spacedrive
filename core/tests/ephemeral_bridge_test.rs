//! Ephemeral Directory Event Streaming Bridge Test
//!
//! Tests the core ephemeral browsing flow end-to-end:
//! 1. TS client subscribes to events for a directory path scope
//! 2. TS client queries the directory listing (backend returns empty, dispatches indexer)
//! 3. Indexer emits ResourceChangedBatch events
//! 4. Events stream through EventBuffer -> RPC -> TCP -> TS subscription
//! 5. TS client receives events and verifies files arrive
//!
//! This test exists to catch regressions in the event delivery pipeline
//! for ephemeral (non-indexed) directory browsing.

mod helpers;

use helpers::*;
use sd_core::device::get_current_device_slug;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Connection info passed from Rust test harness to TypeScript tests
#[derive(Debug, Serialize, Deserialize)]
struct EphemeralBridgeConfig {
	/// TCP socket address for daemon connection
	socket_addr: String,
	/// Library UUID
	library_id: String,
	/// Device slug used by this daemon (must match path_scope in subscriptions)
	device_slug: String,
	/// Physical path to the ephemeral directory (not a managed location)
	ephemeral_dir_path: PathBuf,
	/// Test data directory
	test_data_path: PathBuf,
}

#[tokio::test]
async fn test_ephemeral_directory_event_streaming() -> anyhow::Result<()> {
	let harness = IndexingHarnessBuilder::new("ephemeral_event_streaming")
		.enable_daemon()
		.build()
		.await?;

	// Create an ephemeral directory with files (NOT a managed location)
	let test_root = harness.temp_path();
	let ephemeral_dir = test_root.join("ephemeral_browse");
	tokio::fs::create_dir_all(&ephemeral_dir).await?;

	// Create files the indexer will discover
	tokio::fs::write(ephemeral_dir.join("document.txt"), "Hello world").await?;
	tokio::fs::write(ephemeral_dir.join("photo.jpg"), "fake jpeg data").await?;
	tokio::fs::write(ephemeral_dir.join("notes.md"), "# Notes").await?;
	tokio::fs::write(ephemeral_dir.join("script.rs"), "fn main() {}").await?;
	tokio::fs::write(ephemeral_dir.join("data.json"), r#"{"key": "value"}"#).await?;

	// Create a subdirectory too
	tokio::fs::create_dir_all(ephemeral_dir.join("subfolder")).await?;
	tokio::fs::write(ephemeral_dir.join("subfolder/nested.txt"), "nested").await?;

	let socket_addr = harness
		.daemon_socket_addr()
		.expect("Daemon should be enabled")
		.to_string();

	let device_slug = get_current_device_slug();
	eprintln!("[Rust] Device slug: {}", device_slug);

	let bridge_config = EphemeralBridgeConfig {
		socket_addr: socket_addr.clone(),
		library_id: harness.library.id().to_string(),
		device_slug: device_slug.clone(),
		ephemeral_dir_path: ephemeral_dir.clone(),
		test_data_path: harness.temp_path().to_path_buf(),
	};

	let config_path = harness.temp_path().join("ephemeral_bridge_config.json");
	let config_json = serde_json::to_string_pretty(&bridge_config)?;
	tokio::fs::write(&config_path, config_json).await?;

	tracing::info!("Bridge config written to: {}", config_path.display());
	tracing::info!("Socket address: {}", socket_addr);
	tracing::info!("Library ID: {}", bridge_config.library_id);
	tracing::info!("Ephemeral dir: {}", ephemeral_dir.display());

	let ts_test_file = "packages/ts-client/tests/integration/ephemeral-streaming.test.ts";
	let workspace_root = std::env::current_dir()?.parent().unwrap().to_path_buf();
	let ts_test_path = workspace_root.join(ts_test_file);
	let bun_config = workspace_root.join("packages/ts-client/tests/integration/bunfig.toml");

	eprintln!("\n=== Ephemeral Event Streaming Bridge Test ===");
	eprintln!("Workspace root: {}", workspace_root.display());
	eprintln!("Test file: {}", ts_test_path.display());
	eprintln!("Config path: {}", config_path.display());
	eprintln!("Socket address: {}", socket_addr);
	eprintln!("Library ID: {}", bridge_config.library_id);
	eprintln!("Ephemeral dir: {}", ephemeral_dir.display());
	eprintln!("=============================================\n");

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

	let stdout = String::from_utf8_lossy(&output.stdout);
	let stderr = String::from_utf8_lossy(&output.stderr);

	if !stdout.is_empty() {
		eprintln!("\n=== TypeScript stdout ===\n{}\n", stdout);
	}
	if !stderr.is_empty() {
		eprintln!("\n=== TypeScript stderr ===\n{}\n", stderr);
	}

	if !output.status.success() {
		anyhow::bail!(
			"TypeScript test failed with exit code: {:?}",
			output.status.code()
		);
	}

	tracing::info!("Ephemeral event streaming test passed!");

	harness.shutdown().await?;
	Ok(())
}
