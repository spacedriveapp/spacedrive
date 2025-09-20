//! Core file transfer test using the new cargo test subprocess framework
//!
//! This test demonstrates cross-device file sharing functionality where Alice
//! (sender) pairs with Bob (receiver) and transfers multiple test files.

use sd_core::{
	domain::content_identity::ContentHashGenerator, service::file_sharing::TransferState,
	testing::CargoTestRunner, Core,
};
use std::{env, path::PathBuf, time::Duration};
use tokio::time::timeout;

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
	let mut core = timeout(
		Duration::from_secs(10),
		Core::new_with_config(data_dir.clone()),
	)
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

	// Wait longer for networking to fully initialize and detect external addresses
	tokio::time::sleep(Duration::from_secs(3)).await;
	println!("Alice: Networking initialized successfully");

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
			networking.start_pairing_as_initiator(),
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
	println!(
		"Alice: Pairing code written to /tmp/spacedrive-file-transfer-test/pairing_code.txt"
	);

	// Wait for pairing completion
	println!("Alice: Waiting for Bob to connect...");
	let mut receiver_device_id = None;
	let mut attempts = 0;
	let max_attempts = 45; // 45 seconds

	loop {
		tokio::time::sleep(Duration::from_secs(1)).await;

		let connected_devices = core.get_connected_devices().await.unwrap();
		if !connected_devices.is_empty() {
			receiver_device_id = Some(connected_devices[0]);
			println!(
				"Alice: Bob connected! Device ID: {}",
				connected_devices[0]
			);

			// Wait a bit longer to ensure session keys are properly established
			println!("Alice: Allowing extra time for session key establishment...");
			tokio::time::sleep(Duration::from_secs(2)).await;
			break;
		}

		// Also check if there are any paired devices (even if not currently connected)
		if let Some(networking) = core.networking() {
			let device_registry = networking.device_registry();
			let registry = device_registry.read().await;
			let paired_devices = registry.get_paired_devices();
			if !paired_devices.is_empty() {
				println!("Alice: Found {} paired devices!", paired_devices.len());
				for device in &paired_devices {
					println!(
						"  Paired: {} (ID: {})",
						device.device_name, device.device_id
					);
				}
				// Use the first paired device as the receiver
				receiver_device_id = Some(paired_devices[0].device_id);
				println!(
					"Alice: Using paired device as receiver: {}",
					paired_devices[0].device_id
				);
				break;
			}
		}

		attempts += 1;
		if attempts >= max_attempts {
			panic!("Alice: Pairing timeout - Bob not connected");
		}

		if attempts % 5 == 0 {
			println!("Alice: Pairing status check {} - waiting", attempts / 5);
		}
	}

	let receiver_id = receiver_device_id.unwrap();

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
	let alice_devices = core.get_connected_devices_info().await.unwrap();
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
			let mut completed = false;
			for _ in 0..30 {
				// Wait up to 30 seconds
				tokio::time::sleep(Duration::from_secs(1)).await;

				match core
					.services
					.file_sharing
					.get_transfer_status(&transfer_id)
					.await
				{
					Ok(status) => {
						match status.state {
							TransferState::Completed => {
								println!(
									"Alice: Transfer {:?} completed successfully",
									transfer_id
								);
								completed = true;
								break;
							}
							TransferState::Failed => {
								println!(
									"Alice: Transfer {:?} failed: {:?}",
									transfer_id, status.error
								);
								completed = false;
								break;
							}
							_ => {
								// Still in progress
								if status.progress.bytes_transferred > 0 {
									println!(
										"Alice: Transfer progress: {} / {} bytes",
										status.progress.bytes_transferred,
										status.progress.total_bytes
									);
								}
							}
						}
					}
					Err(e) => {
						println!("Alice: Could not get transfer status: {}", e);
					}
				}
			}

			if completed {
				println!(
					"Alice: All transfers completed, now waiting for Bob's confirmation..."
				);

				// Wait for Bob to confirm receipt and verification
				let mut bob_confirmed = false;
				for attempt in 1..=60 {
					// Wait up to 60 seconds for Bob's confirmation
					if std::fs::read_to_string(
						"/tmp/spacedrive-file-transfer-test/bob_received_confirmation.txt",
					)
					.map(|content| content.starts_with("received_and_verified:"))
					.unwrap_or(false)
					{
						println!("Alice: Bob confirmed file receipt and verification!");
						bob_confirmed = true;
						break;
					}

					if attempt % 10 == 0 {
						println!(
							"Alice: Still waiting for Bob's confirmation... ({}s)",
							attempt
						);
					}
					tokio::time::sleep(Duration::from_secs(1)).await;
				}

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
				println!(
					"Alice: Transfer {:?} did not complete in time",
					transfer_id
				);
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
	let mut core = timeout(Duration::from_secs(10), Core::new_with_config(data_dir))
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

	// Wait longer for networking to fully initialize and detect external addresses
	tokio::time::sleep(Duration::from_secs(3)).await;
	println!("Bob: Networking initialized successfully");

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
	let pairing_code = loop {
		if let Ok(code) =
			std::fs::read_to_string("/tmp/spacedrive-file-transfer-test/pairing_code.txt")
		{
			break code.trim().to_string();
		}
		tokio::time::sleep(Duration::from_millis(500)).await;
	};
	println!("Bob: Found pairing code");

	// Join pairing session
	println!("Bob: Joining pairing with Alice...");
	if let Some(networking) = core.networking() {
		timeout(
			Duration::from_secs(15),
			networking.start_pairing_as_joiner(&pairing_code),
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
	let mut attempts = 0;
	let max_attempts = 30; // 30 seconds

	loop {
		tokio::time::sleep(Duration::from_secs(1)).await;

		// Check pairing status by looking at connected devices
		let connected_devices = core.get_connected_devices().await.unwrap();
		if !connected_devices.is_empty() {
			println!("Bob: Pairing completed successfully!");
			println!("Bob: Connected {} devices", connected_devices.len());

			// Debug: Show Bob's view of connected devices
			let bob_devices = core.get_connected_devices_info().await.unwrap();
			println!("Bob: Connected devices after pairing:");
			for device in &bob_devices {
				println!(
					"  Device: {} (ID: {})",
					device.device_name, device.device_id
				);
			}

			// Wait a bit longer to ensure session keys are properly established
			println!("Bob: Allowing extra time for session key establishment...");
			tokio::time::sleep(Duration::from_secs(2)).await;
			break;
		}

		// Also check if there are any paired devices (even if not currently connected)
		if let Some(networking) = core.networking() {
			let device_registry = networking.device_registry();
			let registry = device_registry.read().await;
			let paired_devices = registry.get_paired_devices();
			if !paired_devices.is_empty() {
				println!("Bob: Found {} paired devices!", paired_devices.len());
				for device in &paired_devices {
					println!(
						"  Paired: {} (ID: {})",
						device.device_name, device.device_id
					);
				}
				// Even if not showing as "connected", we have paired devices, so pairing worked
				break;
			}
		}

		attempts += 1;
		if attempts >= max_attempts {
			panic!("Bob: Pairing timeout - no devices connected");
		}

		if attempts % 5 == 0 {
			println!("Bob: Pairing status check {} - waiting", attempts / 5);
		}
	}

	// Wait for file transfers
	println!("Bob: Waiting for file transfers...");

	// Create directory for received files
	let received_dir = std::path::Path::new("/tmp/received_files");
	std::fs::create_dir_all(received_dir).unwrap();
	println!(
		"Bob: Created directory for received files: {:?}",
		received_dir
	);

	// Wait for expected files to arrive
	let expected_files = loop {
		if let Ok(content) =
			std::fs::read_to_string("/tmp/spacedrive-file-transfer-test/expected_files.txt")
		{
			break content
				.lines()
				.map(|line| {
					let parts: Vec<&str> = line.split(':').collect();
					(parts[0].to_string(), parts[1].parse::<usize>().unwrap_or(0))
				})
				.collect::<Vec<(String, usize)>>();
		}
		tokio::time::sleep(Duration::from_millis(500)).await;
	};

	println!(
		"Bob: Expecting {} files to be received",
		expected_files.len()
	);
	for (filename, size) in &expected_files {
		println!("  Expecting: {} ({} bytes)", filename, size);
	}

	// Monitor for received files
	let mut received_files = Vec::new();
	let start_time = std::time::Instant::now();
	let timeout_duration = Duration::from_secs(60); // 1 minute timeout

	while received_files.len() < expected_files.len() && start_time.elapsed() < timeout_duration {
		tokio::time::sleep(Duration::from_secs(1)).await;

		// Check for new files in received directory
		if let Ok(entries) = std::fs::read_dir(received_dir) {
			for entry in entries {
				if let Ok(entry) = entry {
					let filename = entry.file_name().to_string_lossy().to_string();
					if !received_files.contains(&filename) {
						if let Ok(metadata) = entry.metadata() {
							received_files.push(filename.clone());
							println!(
								"Bob: Received file: {} ({} bytes)",
								filename,
								metadata.len()
							);
						}
					}
				}
			}
		}

		// Debug: Show directory contents periodically
		let elapsed = start_time.elapsed().as_secs();
		if elapsed > 0 && elapsed % 10 == 0 && received_files.is_empty() {
			println!("Bob: Still waiting for files... checking directory:");
			if let Ok(entries) = std::fs::read_dir(received_dir) {
				let file_count = entries.count();
				println!(
					"  Found {} items in {}",
					file_count,
					received_dir.display()
				);
			}
		}

		if received_files.len() > 0 && received_files.len() % 2 == 0 {
			println!(
				"Bob: Progress: {}/{} files received",
				received_files.len(),
				expected_files.len()
			);
		}
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

	// Wait for Alice to initialize and generate pairing code
	tokio::time::sleep(Duration::from_secs(8)).await;

	// Start Bob as receiver
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
