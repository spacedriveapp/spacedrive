//! Core file transfer test using the new cargo test subprocess framework
//!
//! This test demonstrates cross-device file sharing functionality where Alice
//! (sender) pairs with Bob (receiver) and transfers multiple test files.

use sd_core::{domain::content_identity::ContentHashGenerator, testing::CargoTestRunner, Core};
use std::{env, path::PathBuf, time::Duration};
use tokio::time::timeout;

#[path = "helpers/wait.rs"]
mod wait;
use wait::{wait_for_dir_entries, wait_for_file, wait_for_paired_device, wait_for_transfer};

/// Alice's file transfer scenario - sender role
#[tokio::test]
#[ignore] // Only run when explicitly called via subprocess
async fn alice_file_transfer_scenario() {
	// Exit early if not running as Alice
	if env::var("TEST_ROLE").unwrap_or_default() != "alice" {
		return;
	}

	// Set test directory for file-based discovery
	env::set_var("SPACEDRIVE_TEST_DIR", "/tmp/spacedrive-file-transfer-test");

	let data_dir = PathBuf::from("/tmp/spacedrive-file-transfer-test/alice");
	let device_name = "Alice's Test Device";

	println!("Alice: Starting Core file transfer test (sender)");
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

	// Create directory for received files BEFORE adding as allowed path
	std::fs::create_dir_all("/tmp/received_files").unwrap();

	// Add allowed path for file transfers (security requirement from PR #2944)
	if let Some(networking) = core.networking() {
		let protocol_registry = networking.protocol_registry();
		let registry = protocol_registry.read().await;
		if let Some(handler) = registry.get_handler("file_transfer") {
			if let Some(ft_handler) =
				handler
					.as_any()
					.downcast_ref::<sd_core::service::network::protocol::FileTransferProtocolHandler>(
					) {
				ft_handler.add_allowed_path(std::path::PathBuf::from("/tmp/received_files"));
				println!("Alice: Added /tmp/received_files as allowed path");
			}
		}
	}

	// Create a library for job dispatch (required for file transfers)
	println!("Alice: Creating library for file transfer jobs...");
	let _library = core
		.libraries
		.create_library("Alice Transfer Library", None, core.context.clone())
		.await
		.unwrap();
	println!("Alice: Library created successfully");

	// Start pairing as initiator
	println!("Alice: Starting pairing as initiator for file transfer...");
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
	std::fs::create_dir_all("/tmp/spacedrive-file-transfer-test").unwrap();
	std::fs::write(
		"/tmp/spacedrive-file-transfer-test/pairing_code.txt",
		&pairing_code,
	)
	.unwrap();
	println!("Alice: Pairing code written to /tmp/spacedrive-file-transfer-test/pairing_code.txt");

	// Wait for pairing completion
	println!("Alice: Waiting for Bob to connect...");
	let receiver_id = wait_for_paired_device(&core, Duration::from_secs(45))
		.await
		.expect("Alice: Pairing timeout - Bob not connected");
	println!("Alice: Bob connected! Device ID: {}", receiver_id);

	// Bob's marker says his registry holds Alice too, so the transfer cannot
	// race his side of the pairing.
	wait_for_file(
		"/tmp/spacedrive-file-transfer-test/bob_paired.txt",
		Duration::from_secs(60),
	)
	.await
	.expect("Alice: Bob never confirmed pairing");

	// Create test files to transfer
	println!("Alice: Creating test files for transfer...");
	let test_files_dir = data_dir.join("test_files");
	std::fs::create_dir_all(&test_files_dir).unwrap();

	let medium_content = "A".repeat(1024);
	let test_files = vec![
		("small_file.txt", "Hello from Alice's device!"),
		("medium_file.txt", medium_content.as_str()), // 1KB file
		(
			"metadata_test.json",
			r#"{"test": "file", "size": "medium", "purpose": "cross-device-transfer"}"#,
		),
	];

	let mut source_paths = Vec::new();
	for (filename, content) in &test_files {
		let file_path = test_files_dir.join(filename);
		std::fs::write(&file_path, content).unwrap();

		// Generate and display checksum for the file Alice is about to send
		match ContentHashGenerator::generate_content_hash(&file_path).await {
			Ok(checksum) => {
				println!(
					"  Created: {} ({} bytes, checksum: {})",
					filename,
					content.len(),
					checksum
				); // Show full checksum
			}
			Err(e) => {
				println!(
					"  Created: {} ({} bytes, checksum error: {})",
					filename,
					content.len(),
					e
				);
			}
		}

		source_paths.push(file_path);
	}

	// Write file list for Bob to expect
	let file_list: Vec<String> = test_files
		.iter()
		.map(|(name, content)| format!("{}:{}", name, content.len()))
		.collect();
	std::fs::write(
		"/tmp/spacedrive-file-transfer-test/expected_files.txt",
		file_list.join("\n"),
	)
	.unwrap();

	// Debug: Show Alice's view of connected devices
	let alice_devices = core
		.services
		.device
		.get_connected_devices_info()
		.await
		.unwrap();
	println!("Alice: Connected devices before transfer:");
	for device in &alice_devices {
		println!(
			"  Device: {} (ID: {})",
			device.device_name, device.device_id
		);
	}

	// Initiate cross-device file transfer
	println!("Alice: Starting cross-device file transfer...");
	println!("Alice: Sending files to device ID: {}", receiver_id);

	let transfer_results = core
		.services
		.file_sharing
		.share_with_device(
			source_paths,
			receiver_id,
			Some(PathBuf::from("/tmp/received_files")),
		)
		.await;

	match transfer_results {
		Ok(transfer_id) => {
			println!("Alice: File transfer initiated successfully!");
			println!("Alice: Transfer ID: {:?}", transfer_id);

			// Wait for transfer to complete
			println!("Alice: Waiting for transfer to complete...");
			let completed = wait_for_transfer(&core, &transfer_id, Duration::from_secs(30))
				.await
				.unwrap_or(false);
			if completed {
				println!("Alice: Transfer {:?} completed successfully", transfer_id);
				println!("Alice: All transfers completed, now waiting for Bob's confirmation...");

				// Wait for Bob to confirm receipt and verification
				let bob_confirmed = wait_for_file(
					"/tmp/spacedrive-file-transfer-test/bob_received_confirmation.txt",
					Duration::from_secs(60),
				)
				.await
				.map(|content| content.starts_with("received_and_verified:"))
				.unwrap_or(false);

				if bob_confirmed {
					println!("FILE_TRANSFER_SUCCESS: Alice completed all file transfers and Bob confirmed receipt");
					// Write success marker for orchestrator to detect
					std::fs::write(
						"/tmp/spacedrive-file-transfer-test/alice_success.txt",
						"success",
					)
					.unwrap();
				} else {
					panic!("Alice: Bob did not confirm file receipt within timeout");
				}
			} else {
				println!("Alice: Transfer {:?} did not complete in time", transfer_id);
				panic!("Alice: File transfer did not complete in time");
			}
		}
		Err(e) => {
			println!("Alice: File transfer failed: {}", e);
			panic!("Alice: File transfer initiation failed: {}", e);
		}
	}

	println!("Alice: File transfer sender test completed");
}

/// Bob's file transfer scenario - receiver role
#[tokio::test]
#[ignore] // Only run when explicitly called via subprocess
async fn bob_file_transfer_scenario() {
	// Exit early if not running as Bob
	if env::var("TEST_ROLE").unwrap_or_default() != "bob" {
		return;
	}

	// Set test directory for file-based discovery
	env::set_var("SPACEDRIVE_TEST_DIR", "/tmp/spacedrive-file-transfer-test");

	let data_dir = PathBuf::from("/tmp/spacedrive-file-transfer-test/bob");
	let device_name = "Bob's Test Device";

	println!("Bob: Starting Core file transfer test (receiver)");
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

	// Create directory for received files BEFORE pairing (security requirement from PR #2944)
	let received_dir = std::path::Path::new("/tmp/received_files");
	std::fs::create_dir_all(received_dir).unwrap();
	println!(
		"Bob: Created directory for received files: {:?}",
		received_dir
	);

	// Add allowed path for file transfers AFTER directory creation (security requirement from PR #2944)
	if let Some(networking) = core.networking() {
		let protocol_registry = networking.protocol_registry();
		let registry = protocol_registry.read().await;
		if let Some(handler) = registry.get_handler("file_transfer") {
			if let Some(ft_handler) =
				handler
					.as_any()
					.downcast_ref::<sd_core::service::network::protocol::FileTransferProtocolHandler>(
					) {
				ft_handler.add_allowed_path(std::path::PathBuf::from("/tmp/received_files"));
				println!("Bob: Added /tmp/received_files as allowed path");
			}
		}
	}

	// Create a library for job dispatch (required for file transfers)
	println!("Bob: Creating library for file transfer jobs...");
	let _library = core
		.libraries
		.create_library("Bob Transfer Library", None, core.context.clone())
		.await
		.unwrap();
	println!("Bob: Library created successfully");

	// Wait for Alice to create pairing code
	println!("Bob: Looking for pairing code from Alice...");
	let pairing_code = wait_for_file(
		"/tmp/spacedrive-file-transfer-test/pairing_code.txt",
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
	wait_for_paired_device(&core, Duration::from_secs(30))
		.await
		.expect("Bob: Pairing timeout - no devices connected");
	println!("Bob: Pairing completed successfully!");

	// Debug: Show Bob's view of connected devices
	let bob_devices = core
		.services
		.device
		.get_connected_devices_info()
		.await
		.unwrap();
	println!("Bob: Connected devices after pairing:");
	for device in &bob_devices {
		println!(
			"  Device: {} (ID: {})",
			device.device_name, device.device_id
		);
	}
	std::fs::write(
		"/tmp/spacedrive-file-transfer-test/bob_paired.txt",
		"paired",
	)
	.unwrap();

	// Wait for file transfers
	println!("Bob: Waiting for file transfers...");

	// Directory and allowed path already configured before pairing

	// Wait for expected files to arrive
	let expected_files = wait_for_file(
		"/tmp/spacedrive-file-transfer-test/expected_files.txt",
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
		"Bob: Expecting {} files to be received",
		expected_files.len()
	);
	for (filename, size) in &expected_files {
		println!("  Expecting: {} ({} bytes)", filename, size);
	}

	// Monitor for received files
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

		// Verify file contents and checksums
		let mut verification_success = true;
		for (expected_name, expected_size) in &expected_files {
			let received_path = received_dir.join(expected_name);
			if received_path.exists() {
				if let Ok(metadata) = std::fs::metadata(&received_path) {
					if metadata.len() == *expected_size as u64 {
						// Generate checksum for received file
						match ContentHashGenerator::generate_content_hash(&received_path).await {
							Ok(checksum) => {
								println!(
									"Bob: Verified: {} (size: {} bytes, checksum: {})",
									expected_name,
									metadata.len(),
									checksum
								); // Show full checksum
							}
							Err(e) => {
								println!(
									"Bob: Could not generate checksum for {}: {}",
									expected_name, e
								);
								println!("Bob: Verified: {} (size matches)", expected_name);
							}
						}
					} else {
						println!(
							"Bob: Size mismatch for {}: expected {}, got {}",
							expected_name,
							expected_size,
							metadata.len()
						);
						verification_success = false;
					}
				} else {
					println!("Bob: Could not read metadata for {}", expected_name);
					verification_success = false;
				}
			} else {
				println!("Bob: Expected file not found: {}", expected_name);
				verification_success = false;
			}
		}

		if verification_success {
			println!("FILE_TRANSFER_SUCCESS: Bob verified all received files");
			// Write success marker for orchestrator to detect
			std::fs::write(
				"/tmp/spacedrive-file-transfer-test/bob_success.txt",
				"success",
			)
			.unwrap();

			// Also write a timestamped confirmation that Alice can detect
			let timestamp = std::time::SystemTime::now()
				.duration_since(std::time::UNIX_EPOCH)
				.unwrap()
				.as_secs();
			std::fs::write(
				"/tmp/spacedrive-file-transfer-test/bob_received_confirmation.txt",
				format!("received_and_verified:{}", timestamp),
			)
			.unwrap();
			println!("Bob: Wrote confirmation signal for Alice");
		} else {
			panic!("Bob: File verification failed");
		}
	} else {
		println!(
			"Bob: Only received {}/{} expected files",
			received_files.len(),
			expected_files.len()
		);
		panic!("Bob: Not all files were received");
	}

	println!("Bob: File transfer receiver test completed");
}

/// Main test orchestrator - spawns cargo test subprocesses for file transfer
#[tokio::test]
async fn test_file_transfer() {
	// Clean up any old test files to avoid race conditions
	let _ = std::fs::remove_dir_all("/tmp/spacedrive-file-transfer-test");
	let _ = std::fs::remove_dir_all("/tmp/received_files");
	std::fs::create_dir_all("/tmp/spacedrive-file-transfer-test").unwrap();

	println!("Testing Core file transfer with cargo test subprocess framework");

	let mut runner = CargoTestRunner::for_test_file("file_transfer_test")
		.with_timeout(Duration::from_secs(240)) // 4 minutes for file transfer test
		.add_subprocess("alice", "alice_file_transfer_scenario")
		.add_subprocess("bob", "bob_file_transfer_scenario");

	// Spawn Alice first (sender)
	println!("Starting Alice as file sender...");
	runner
		.spawn_single_process("alice")
		.await
		.expect("Failed to spawn Alice");

	// Bob waits for the pairing code file himself, so he can start at once.
	println!("Starting Bob as file receiver...");
	runner
		.spawn_single_process("bob")
		.await
		.expect("Failed to spawn Bob");

	// Run until both devices successfully complete file transfer using file markers
	let result = runner
		.wait_for_success(|_outputs| {
			let alice_success =
				std::fs::read_to_string("/tmp/spacedrive-file-transfer-test/alice_success.txt")
					.map(|content| content.trim() == "success")
					.unwrap_or(false);
			let bob_success =
				std::fs::read_to_string("/tmp/spacedrive-file-transfer-test/bob_success.txt")
					.map(|content| content.trim() == "success")
					.unwrap_or(false);

			alice_success && bob_success
		})
		.await;

	match result {
		Ok(_) => {
			println!(
				"Cargo test subprocess file transfer test successful with complete file verification!"
			);
		}
		Err(e) => {
			println!("Cargo test subprocess file transfer test failed: {}", e);
			for (name, output) in runner.get_all_outputs() {
				println!("\\n{} output:\\n{}", name, output);
			}
			panic!("Cargo test subprocess file transfer test failed - files were not properly transferred and verified");
		}
	}
}
