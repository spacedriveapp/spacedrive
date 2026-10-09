//! Cross-device copy test using the action system
//!
//! This test demonstrates the copy system's routing capabilities by having Alice
//! create files and then dispatch copy actions where the source SdPath is on
//! Alice's device and the destination is on Bob's device.

use sd_core::{
	domain::addressing::{SdPath, SdPathBatch},
	ops::files::copy::{action::FileCopyAction, CopyOptions},
	testing::CargoTestRunner,
	Core,
};
use std::{env, path::PathBuf, time::Duration};
use tokio::time::timeout;

#[path = "helpers/wait.rs"]
mod wait;
use wait::{wait_for_connected_device, wait_for_dir_entries, wait_for_file};

/// Alice's cross-device copy scenario - sender role
#[tokio::test]
#[ignore] // Only run when explicitly called via subprocess
async fn alice_cross_device_copy_scenario() {
	// Exit early if not running as Alice
	if env::var("TEST_ROLE").unwrap_or_default() != "alice" {
		return;
	}

	// Set test directory for file-based discovery
	env::set_var(
		"SPACEDRIVE_TEST_DIR",
		"/tmp/spacedrive-cross-device-copy-test",
	);

	let data_dir = PathBuf::from("/tmp/spacedrive-cross-device-copy-test/alice");
	let device_name = "Alice's Test Device";

	println!("Alice: Starting Core cross-device copy test (sender)");
	println!("Alice: Data dir: {:?}", data_dir);

	// Initialize Core
	println!("Alice: Initializing Core...");
	let mut core = timeout(Duration::from_secs(10), Core::new(data_dir.clone()))
		.await
		.unwrap()
		.unwrap();
	println!("Alice: Core initialized successfully");

	// Set device name
	println!("Alice: Setting device name for testing...");
	core.device.set_name(device_name.to_string()).unwrap();

	// Initialize networking
	println!("Alice: Initializing networking...");
	timeout(Duration::from_secs(10), core.init_networking())
		.await
		.unwrap()
		.unwrap();

	println!("Alice: Networking initialized successfully");

	// Create a library for job dispatch
	println!("Alice: Creating library for copy operations...");
	let library = core
		.libraries
		.create_library("Alice Copy Library", None, core.context.clone())
		.await
		.unwrap();
	let library_id = library.id();
	println!("Alice: Library created successfully (ID: {})", library_id);

	// Start pairing as initiator
	println!("Alice: Starting pairing as initiator...");
	let (pairing_code, expires_in) = if let Some(networking) = core.networking() {
		timeout(
			Duration::from_secs(15),
			networking.start_pairing_as_initiator(false),
		)
		.await
		.unwrap()
		.unwrap()
	} else {
		panic!("Networking not initialized");
	};

	let short_code = pairing_code
		.split_whitespace()
		.take(3)
		.collect::<Vec<_>>()
		.join(" ");
	println!(
		"Alice: Pairing code generated: {}... (expires in {}s)",
		short_code, expires_in
	);

	// Write pairing code to shared location for Bob to read
	std::fs::create_dir_all("/tmp/spacedrive-cross-device-copy-test").unwrap();
	std::fs::write(
		"/tmp/spacedrive-cross-device-copy-test/pairing_code.txt",
		&pairing_code,
	)
	.unwrap();
	println!(
		"Alice: Pairing code written to /tmp/spacedrive-cross-device-copy-test/pairing_code.txt"
	);

	// Wait for pairing completion
	println!("Alice: Waiting for Bob to connect...");
	let bob_id = wait_for_connected_device(&core, Duration::from_secs(45))
		.await
		.expect("Alice: Pairing timeout - Bob not connected");
	println!("Alice: Bob connected! Device ID: {}", bob_id);

	// Bob's marker says his registry holds Alice too, so the copy cannot race
	// his side of the pairing.
	wait_for_file(
		"/tmp/spacedrive-cross-device-copy-test/bob_paired.txt",
		Duration::from_secs(60),
	)
	.await
	.expect("Alice: Bob never confirmed pairing");

	// Create test files to copy
	println!("Alice: Creating test files for cross-device copy...");
	let test_files_dir = data_dir.join("test_files");
	std::fs::create_dir_all(&test_files_dir).unwrap();

	let test_files = vec![
		("test1.txt", "Hello from Alice's device - file 1!"),
		("test2.txt", "Cross-device copy test - file 2"),
		(
			"test3.json",
			r#"{"test": "cross-device-copy", "from": "alice", "to": "bob"}"#,
		),
	];

	let mut source_paths = Vec::new();
	for (filename, content) in &test_files {
		let file_path = test_files_dir.join(filename);
		std::fs::write(&file_path, content).unwrap();
		println!("  Created: {} ({} bytes)", filename, content.len());
		source_paths.push(file_path);
	}

	// Write file list for Bob to expect
	let file_list: Vec<String> = test_files
		.iter()
		.map(|(name, content)| format!("{}:{}", name, content.len()))
		.collect();
	std::fs::write(
		"/tmp/spacedrive-cross-device-copy-test/expected_files.txt",
		file_list.join("\n"),
	)
	.unwrap();

	// Get Alice's device ID
	let alice_device_id = core.device.device_id().unwrap();
	println!("Alice: My device ID is {}", alice_device_id);

	// Prepare copy operations using the action system
	println!("Alice: Dispatching cross-device copy actions...");

	// Get the action manager from context
	let action_manager = core
		.context
		.get_action_manager()
		.await
		.expect("Action manager not initialized");

	// Copy each file individually to test the routing
	for (i, (source_path, (filename, _))) in source_paths.iter().zip(&test_files).enumerate() {
		println!("Alice: Preparing copy action {} for {}", i + 1, filename);

		// Create source SdPath (on Alice's device)
		// Note: slug is generated from device name "Alice's Test Device" → "alice-s-test-device"
		let source_sdpath = SdPath::physical("alice-s-test-device".to_string(), source_path);

		// Create destination SdPath (on Bob's device) - use directory, not full path
		// The job will automatically join the filename for cross-device copies
		// Note: slug is generated from device name "Bob's Test Device" → "bob-s-test-device"
		let dest_dir = PathBuf::from("/tmp/received_files");
		let dest_sdpath = SdPath::physical("bob-s-test-device".to_string(), &dest_dir);

		println!(
			"  Source: {} (device: {})",
			source_path.display(),
			alice_device_id
		);
		println!(
			"  Destination dir: {} (device: {}) - file will be: {}",
			dest_dir.display(),
			bob_id,
			filename
		);

		// Build the copy action directly with SdPath
		let copy_action = FileCopyAction {
			sources: SdPathBatch::new(vec![source_sdpath]),
			destination: dest_sdpath,
			options: CopyOptions {
				overwrite: true,
				verify_checksum: true,
				preserve_timestamps: true,
				..Default::default()
			},
			on_conflict: None,
		};

		// Dispatch the action
		match action_manager
			.dispatch_library(Some(library_id), copy_action)
			.await
		{
			Ok(output) => {
				println!("Alice: Copy action {} dispatched successfully", i + 1);
				println!("  Output: {:?}", output);
			}
			Err(e) => {
				println!("Alice: Copy action {} failed: {}", i + 1, e);
				panic!("Failed to dispatch copy action: {}", e);
			}
		}
	}

	// Wait for Bob to confirm receipt
	println!("Alice: Waiting for Bob to confirm file receipt...");
	let bob_confirmed = wait_for_file(
		"/tmp/spacedrive-cross-device-copy-test/bob_verified.txt",
		Duration::from_secs(60),
	)
	.await
	.map(|content| content.starts_with("verified:"))
	.unwrap_or(false);

	if bob_confirmed {
		println!("CROSS_DEVICE_COPY_SUCCESS: Alice successfully dispatched copy actions");
		std::fs::write(
			"/tmp/spacedrive-cross-device-copy-test/alice_success.txt",
			"success",
		)
		.unwrap();
	} else {
		panic!("Alice: Bob did not confirm file receipt within timeout");
	}

	println!("Alice: Cross-device copy test completed");
}

/// Bob's cross-device copy scenario - receiver role
#[tokio::test]
#[ignore] // Only run when explicitly called via subprocess
async fn bob_cross_device_copy_scenario() {
	// Exit early if not running as Bob
	if env::var("TEST_ROLE").unwrap_or_default() != "bob" {
		return;
	}

	// Set test directory for file-based discovery
	env::set_var(
		"SPACEDRIVE_TEST_DIR",
		"/tmp/spacedrive-cross-device-copy-test",
	);

	let data_dir = PathBuf::from("/tmp/spacedrive-cross-device-copy-test/bob");
	let device_name = "Bob's Test Device";

	println!("Bob: Starting Core cross-device copy test (receiver)");
	println!("Bob: Data dir: {:?}", data_dir);

	// Initialize Core
	println!("Bob: Initializing Core...");
	let mut core = timeout(Duration::from_secs(10), Core::new(data_dir))
		.await
		.unwrap()
		.unwrap();
	println!("Bob: Core initialized successfully");

	// Set device name
	println!("Bob: Setting device name for testing...");
	core.device.set_name(device_name.to_string()).unwrap();

	// Initialize networking
	println!("Bob: Initializing networking...");
	timeout(Duration::from_secs(10), core.init_networking())
		.await
		.unwrap()
		.unwrap();

	println!("Bob: Networking initialized successfully");

	// Set up allowed paths for file transfers BEFORE pairing
	let received_dir = std::path::Path::new("/tmp/received_files");
	std::fs::create_dir_all(received_dir).unwrap();
	println!(
		"Bob: Adding {} to allowed file transfer paths...",
		received_dir.display()
	);
	if let Some(networking) = core.networking() {
		let protocol_registry = networking.protocol_registry();
		let registry_guard = protocol_registry.read().await;
		if let Some(file_transfer_handler) = registry_guard.get_handler("file_transfer") {
			if let Some(handler) = file_transfer_handler
				.as_any()
				.downcast_ref::<sd_core::service::network::protocol::FileTransferProtocolHandler>(
			) {
				handler.add_allowed_path(received_dir.to_path_buf());
				println!("Bob: Added {} to allowed paths", received_dir.display());
			}
		}
	}

	// Create a library for job dispatch
	println!("Bob: Creating library for copy operations...");
	let _library = core
		.libraries
		.create_library("Bob Copy Library", None, core.context.clone())
		.await
		.unwrap();
	println!("Bob: Library created successfully");

	// Wait for Alice to create pairing code
	println!("Bob: Looking for pairing code from Alice...");
	let pairing_code = wait_for_file(
		"/tmp/spacedrive-cross-device-copy-test/pairing_code.txt",
		Duration::from_secs(60),
	)
	.await
	.expect("Bob: Alice never wrote a pairing code");
	println!("Bob: Found pairing code");

	// Join pairing session
	println!("Bob: Joining pairing with Alice...");
	if let Some(networking) = core.networking() {
		timeout(
			Duration::from_secs(15),
			networking.start_pairing_as_joiner(&pairing_code, false),
		)
		.await
		.unwrap()
		.unwrap();
	} else {
		panic!("Networking not initialized");
	}
	println!("Bob: Successfully joined pairing");

	// Wait for pairing completion
	println!("Bob: Waiting for pairing to complete...");
	let alice_id = wait_for_connected_device(&core, Duration::from_secs(30))
		.await
		.expect("Bob: Pairing timeout - no devices connected");
	println!("Bob: Pairing completed successfully!");
	println!("Bob: Connected to {}", alice_id);
	std::fs::write(
		"/tmp/spacedrive-cross-device-copy-test/bob_paired.txt",
		"paired",
	)
	.unwrap();

	// Directory already created and added to allowed paths above
	let received_dir = std::path::Path::new("/tmp/received_files");

	// Load expected files
	println!("Bob: Loading expected file list...");
	let expected_files = wait_for_file(
		"/tmp/spacedrive-cross-device-copy-test/expected_files.txt",
		Duration::from_secs(60),
	)
	.await
	.expect("Bob: Alice never wrote the expected file list")
	.lines()
	.map(|line| {
		let parts: Vec<&str> = line.split(':').collect();
		(parts[0].to_string(), parts[1].parse::<usize>().unwrap_or(0))
	})
	.collect::<Vec<(String, usize)>>();

	println!(
		"Bob: Expecting {} files via cross-device copy",
		expected_files.len()
	);
	for (filename, size) in &expected_files {
		println!("  Expecting: {} ({} bytes)", filename, size);
	}

	// Monitor for received files
	println!("Bob: Waiting for files to arrive via action system...");
	let received_files =
		wait_for_dir_entries(received_dir, expected_files.len(), Duration::from_secs(60))
			.await
			.unwrap_or_default();
	for filename in &received_files {
		let size = std::fs::metadata(received_dir.join(filename))
			.map(|metadata| metadata.len())
			.unwrap_or(0);
		println!("Bob: Received file: {} ({} bytes)", filename, size);
	}

	// Verify all expected files were received
	if received_files.len() == expected_files.len() {
		println!("Bob: All expected files received successfully!");

		// Write verification confirmation
		std::fs::write(
			"/tmp/spacedrive-cross-device-copy-test/bob_verified.txt",
			format!("verified:{}", chrono::Utc::now().timestamp()),
		)
		.unwrap();

		// Write success marker
		std::fs::write(
			"/tmp/spacedrive-cross-device-copy-test/bob_success.txt",
			"success",
		)
		.unwrap();

		println!("CROSS_DEVICE_COPY_SUCCESS: Bob verified all received files");
	} else {
		println!(
			"Bob: Only received {}/{} expected files",
			received_files.len(),
			expected_files.len()
		);
		panic!("Bob: Not all files were received");
	}

	println!("Bob: Cross-device copy test completed");
}

/// Main test orchestrator - spawns cargo test subprocesses
#[tokio::test]
async fn test_cross_device_copy() {
	// Clean up any old test files
	let _ = std::fs::remove_dir_all("/tmp/spacedrive-cross-device-copy-test");
	let _ = std::fs::remove_dir_all("/tmp/received_files");
	std::fs::create_dir_all("/tmp/spacedrive-cross-device-copy-test").unwrap();

	println!("Testing cross-device copy with action system routing");

	let mut runner = CargoTestRunner::for_test_file("cross_device_copy_test")
		.with_timeout(Duration::from_secs(180))
		.add_subprocess("alice", "alice_cross_device_copy_scenario")
		.add_subprocess("bob", "bob_cross_device_copy_scenario");

	// Spawn Alice first
	println!("Starting Alice as copy action dispatcher...");
	runner
		.spawn_single_process("alice")
		.await
		.expect("Failed to spawn Alice");

	// Bob waits for the pairing code file himself, so he can start at once.
	// Start Bob as receiver
	println!("Starting Bob as copy receiver...");
	runner
		.spawn_single_process("bob")
		.await
		.expect("Failed to spawn Bob");

	// Run until both complete successfully
	let result = runner
		.wait_for_success(|_outputs| {
			let alice_success =
				std::fs::read_to_string("/tmp/spacedrive-cross-device-copy-test/alice_success.txt")
					.map(|content| content.trim() == "success")
					.unwrap_or(false);
			let bob_success =
				std::fs::read_to_string("/tmp/spacedrive-cross-device-copy-test/bob_success.txt")
					.map(|content| content.trim() == "success")
					.unwrap_or(false);

			alice_success && bob_success
		})
		.await;

	match result {
		Ok(_) => {
			println!("Cross-device copy test successful! Action system routing works correctly.");
		}
		Err(e) => {
			println!("Cross-device copy test failed: {}", e);
			for (name, output) in runner.get_all_outputs() {
				println!("\n{} output:\n{}", name, output);
			}
			panic!("Cross-device copy test failed");
		}
	}
}
