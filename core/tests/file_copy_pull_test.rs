//! PULL transfer test - copying files FROM a remote device TO the local device
//!
//! This test validates the bidirectional file transfer capability by having Bob
//! pull files from Alice's device (the reverse of the standard PUSH operation).
//! Alice hosts files, Bob initiates a PULL request to copy them locally.

use sd_core::{
	domain::addressing::{SdPath, SdPathBatch},
	ops::files::copy::{action::FileCopyAction, CopyOptions},
	testing::CargoTestRunner,
	Core,
};
use std::{env, path::PathBuf, time::Duration};
use tokio::time::timeout;

/// Alice's role in PULL test - file host (source device)
#[tokio::test]
#[ignore] // Only run when explicitly called via subprocess
async fn alice_pull_source_scenario() {
	if env::var("TEST_ROLE").unwrap_or_default() != "alice" {
		return;
	}

	env::set_var("SPACEDRIVE_TEST_DIR", "/tmp/spacedrive-pull-test");

	let data_dir = PathBuf::from("/tmp/spacedrive-pull-test/alice");
	let device_name = "Alice's Test Device";

	println!("Alice: Starting as PULL source (file host)");
	println!("Alice: Data dir: {:?}", data_dir);

	// Initialize Core
	println!("Alice: Initializing Core...");
	let mut core = timeout(Duration::from_secs(10), Core::new(data_dir.clone()))
		.await
		.unwrap()
		.unwrap();
	println!("Alice: Core initialized successfully");

	core.device.set_name(device_name.to_string()).unwrap();

	println!("Alice: Initializing networking...");
	timeout(Duration::from_secs(10), core.init_networking())
		.await
		.unwrap()
		.unwrap();

	tokio::time::sleep(Duration::from_secs(3)).await;
	println!("Alice: Networking initialized successfully");

	// Create a library
	println!("Alice: Creating library...");
	let _library = core
		.libraries
		.create_library("Alice PULL Source Library", None, core.context.clone())
		.await
		.unwrap();
	println!("Alice: Library created");

	// Create test files that Bob will pull (directory must exist before adding as allowed path)
	println!("Alice: Creating test files for Bob to PULL...");
	let test_files_dir = data_dir.join("files_for_bob");
	std::fs::create_dir_all(&test_files_dir).unwrap();

	let test_files = vec![
		(
			"pull_test1.txt",
			"This file was pulled from Alice's device!",
		),
		(
			"pull_test2.txt",
			"Bidirectional transfer test - PULL direction",
		),
		(
			"pull_test3.json",
			r#"{"test": "pull-transfer", "direction": "alice->bob", "method": "PULL"}"#,
		),
	];

	for (filename, content) in &test_files {
		let file_path = test_files_dir.join(filename);
		std::fs::write(&file_path, content).unwrap();
		println!("  Created: {} ({} bytes)", filename, content.len());
	}

	// Write file info for Bob
	let file_info: Vec<String> = test_files
		.iter()
		.map(|(name, content)| {
			format!(
				"{}:{}:{}",
				name,
				content.len(),
				test_files_dir.join(name).display()
			)
		})
		.collect();
	std::fs::create_dir_all("/tmp/spacedrive-pull-test").unwrap();
	std::fs::write(
		"/tmp/spacedrive-pull-test/source_files.txt",
		file_info.join("\n"),
	)
	.unwrap();

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
				ft_handler.add_allowed_path(test_files_dir.clone());
				println!("Alice: Added {} as allowed path", test_files_dir.display());
			}
		}
	}

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

	std::fs::write("/tmp/spacedrive-pull-test/pairing_code.txt", &pairing_code).unwrap();
	println!("Alice: Pairing code written");

	// Wait for Bob to connect
	println!("Alice: Waiting for Bob to connect...");
	let mut attempts = 0;
	let max_attempts = 45;

	loop {
		tokio::time::sleep(Duration::from_secs(1)).await;

		let connected_devices = core
			.services
			.device
			.get_connected_devices_info()
			.await
			.unwrap();
		if !connected_devices.is_empty() {
			println!(
				"Alice: Bob connected! Device: {} ({})",
				connected_devices[0].device_name, connected_devices[0].device_id
			);
			tokio::time::sleep(Duration::from_secs(2)).await;
			break;
		}

		attempts += 1;
		if attempts >= max_attempts {
			panic!("Alice: Pairing timeout - Bob not connected");
		}

		if attempts % 5 == 0 {
			println!("Alice: Pairing status check {} - waiting", attempts / 5);
		}
	}

	// Write ready signal for Bob
	std::fs::write("/tmp/spacedrive-pull-test/alice_ready.txt", "ready").unwrap();
	println!("Alice: Ready for Bob to initiate PULL requests");

	// Wait for Bob to complete PULL transfers
	println!("Alice: Waiting for Bob to complete PULL transfers...");
	let mut bob_completed = false;
	for attempt in 1..=90 {
		if std::fs::read_to_string("/tmp/spacedrive-pull-test/bob_pull_success.txt")
			.map(|content| content.starts_with("success"))
			.unwrap_or(false)
		{
			println!("Alice: Bob completed PULL transfers successfully!");
			bob_completed = true;
			break;
		}

		if attempt % 10 == 0 {
			println!(
				"Alice: Still waiting for Bob's PULL completion... ({}s)",
				attempt
			);
		}
		tokio::time::sleep(Duration::from_secs(1)).await;
	}

	if bob_completed {
		println!("PULL_TEST_SUCCESS: Alice successfully served files for PULL");
		std::fs::write("/tmp/spacedrive-pull-test/alice_success.txt", "success").unwrap();
	} else {
		panic!("Alice: Bob did not complete PULL transfers within timeout");
	}

	println!("Alice: PULL source test completed");

	// Cleanup: shutdown core to release file descriptors
	core.shutdown()
		.await
		.expect("Failed to shutdown Alice core");
}

/// Bob's role in PULL test - initiates PULL to get files from Alice
#[tokio::test]
#[ignore] // Only run when explicitly called via subprocess
async fn bob_pull_receiver_scenario() {
	if env::var("TEST_ROLE").unwrap_or_default() != "bob" {
		return;
	}

	env::set_var("SPACEDRIVE_TEST_DIR", "/tmp/spacedrive-pull-test");

	let data_dir = PathBuf::from("/tmp/spacedrive-pull-test/bob");
	let device_name = "Bob's Test Device";

	println!("Bob: Starting as PULL initiator (will pull files from Alice)");
	println!("Bob: Data dir: {:?}", data_dir);

	// Initialize Core
	println!("Bob: Initializing Core...");
	let mut core = timeout(Duration::from_secs(10), Core::new(data_dir.clone()))
		.await
		.unwrap()
		.unwrap();
	println!("Bob: Core initialized successfully");

	core.device.set_name(device_name.to_string()).unwrap();

	println!("Bob: Initializing networking...");
	timeout(Duration::from_secs(10), core.init_networking())
		.await
		.unwrap()
		.unwrap();

	tokio::time::sleep(Duration::from_secs(3)).await;
	println!("Bob: Networking initialized");

	// Create a library
	println!("Bob: Creating library...");
	let library = core
		.libraries
		.create_library("Bob PULL Library", None, core.context.clone())
		.await
		.unwrap();
	let library_id = library.id();
	println!("Bob: Library created (ID: {})", library_id);

	// Wait for Alice's pairing code
	println!("Bob: Looking for pairing code from Alice...");
	let pairing_code = loop {
		if let Ok(code) = std::fs::read_to_string("/tmp/spacedrive-pull-test/pairing_code.txt") {
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
	let mut attempts = 0;
	let max_attempts = 30;

	loop {
		tokio::time::sleep(Duration::from_secs(1)).await;

		let connected_devices = core
			.services
			.device
			.get_connected_devices_info()
			.await
			.unwrap();
		if !connected_devices.is_empty() {
			println!(
				"Bob: Pairing completed! Connected to {} ({})",
				connected_devices[0].device_name, connected_devices[0].device_id
			);
			tokio::time::sleep(Duration::from_secs(2)).await;
			break;
		}

		attempts += 1;
		if attempts >= max_attempts {
			panic!("Bob: Pairing timeout - no devices connected");
		}
	}

	// Wait for Alice to be ready
	println!("Bob: Waiting for Alice to be ready...");
	loop {
		if std::fs::read_to_string("/tmp/spacedrive-pull-test/alice_ready.txt")
			.map(|content| content.starts_with("ready"))
			.unwrap_or(false)
		{
			break;
		}
		tokio::time::sleep(Duration::from_millis(500)).await;
	}
	println!("Bob: Alice is ready, reading source file info...");

	// Read source files info
	let source_files_info = loop {
		if let Ok(content) = std::fs::read_to_string("/tmp/spacedrive-pull-test/source_files.txt") {
			break content
				.lines()
				.map(|line| {
					let parts: Vec<&str> = line.split(':').collect();
					(
						parts[0].to_string(),
						parts[1].parse::<usize>().unwrap_or(0),
						parts[2].to_string(),
					)
				})
				.collect::<Vec<(String, usize, String)>>();
		}
		tokio::time::sleep(Duration::from_millis(500)).await;
	};

	println!(
		"Bob: Found {} files to PULL from Alice",
		source_files_info.len()
	);
	for (filename, size, path) in &source_files_info {
		println!("  Will PULL: {} ({} bytes) from {}", filename, size, path);
	}

	// Create local destination directory
	let pull_dest_dir = data_dir.join("pulled_files");
	std::fs::create_dir_all(&pull_dest_dir).unwrap();
	println!(
		"Bob: Created destination directory: {}",
		pull_dest_dir.display()
	);

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
				ft_handler.add_allowed_path(pull_dest_dir.clone());
				println!("Bob: Added {} as allowed path", pull_dest_dir.display());
			}
		}
	}

	// Get action manager
	let action_manager = core
		.context
		.get_action_manager()
		.await
		.expect("Action manager not initialized");

	// Initiate PULL for each file
	// PULL: source is on Alice (remote), destination is on Bob (local)
	println!("Bob: Initiating PULL transfers...");

	for (i, (filename, _size, remote_path)) in source_files_info.iter().enumerate() {
		println!("Bob: PULL action {} - pulling {}", i + 1, filename);

		// Source SdPath is on Alice's device (remote from Bob's perspective)
		// Device slug is derived from "Alice's Test Device" → "alice-s-test-device"
		let source_sdpath = SdPath::physical(
			"alice-s-test-device".to_string(),
			PathBuf::from(remote_path),
		);

		// Destination SdPath is on Bob's device (local)
		let dest_path = pull_dest_dir.join(filename);
		let dest_sdpath = SdPath::physical("bob-s-test-device".to_string(), &dest_path);

		println!("  Source (remote): {}", source_sdpath.display());
		println!("  Destination (local): {}", dest_sdpath.display());

		// Build and dispatch PULL action
		// The RemoteTransferStrategy should detect this as PULL direction
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

		match action_manager
			.dispatch_library(Some(library_id), copy_action)
			.await
		{
			Ok(output) => {
				println!("Bob: PULL action {} dispatched successfully", i + 1);
				println!("  Output: {:?}", output);
			}
			Err(e) => {
				println!("Bob: PULL action {} failed: {}", i + 1, e);
				panic!("Failed to dispatch PULL action: {}", e);
			}
		}

		tokio::time::sleep(Duration::from_millis(500)).await;
	}

	// Wait for files to arrive
	println!("Bob: Waiting for PULL transfers to complete...");
	let mut received_files = Vec::new();
	let start_time = std::time::Instant::now();
	let timeout_duration = Duration::from_secs(60);

	while received_files.len() < source_files_info.len() && start_time.elapsed() < timeout_duration
	{
		tokio::time::sleep(Duration::from_secs(1)).await;

		if let Ok(entries) = std::fs::read_dir(&pull_dest_dir) {
			for entry in entries {
				if let Ok(entry) = entry {
					let filename = entry.file_name().to_string_lossy().to_string();
					if !received_files.contains(&filename) {
						if let Ok(metadata) = entry.metadata() {
							received_files.push(filename.clone());
							println!(
								"Bob: PULL received: {} ({} bytes)",
								filename,
								metadata.len()
							);

							// Verify size
							if let Some((_, expected_size, _)) = source_files_info
								.iter()
								.find(|(name, _, _)| name == &filename)
							{
								if metadata.len() == *expected_size as u64 {
									println!("  Size verified: {} bytes", metadata.len());
								} else {
									println!(
										"  Size mismatch: expected {}, got {}",
										expected_size,
										metadata.len()
									);
								}
							}
						}
					}
				}
			}
		}

		let elapsed = start_time.elapsed().as_secs();
		if elapsed > 0 && elapsed % 10 == 0 && received_files.len() < source_files_info.len() {
			println!(
				"Bob: PULL progress: {}/{} files received ({}s elapsed)",
				received_files.len(),
				source_files_info.len(),
				elapsed
			);
		}
	}

	// Verify all files were pulled
	if received_files.len() == source_files_info.len() {
		println!("Bob: All files successfully PULLED from Alice!");

		// Verify content
		let mut all_verified = true;
		for (filename, expected_size, _) in &source_files_info {
			let file_path = pull_dest_dir.join(filename);
			if let Ok(content) = std::fs::read(&file_path) {
				if content.len() == *expected_size {
					println!("  {} - verified ({} bytes)", filename, content.len());
				} else {
					println!(
						"  {} - SIZE MISMATCH (expected {}, got {})",
						filename,
						expected_size,
						content.len()
					);
					all_verified = false;
				}
			} else {
				println!("  {} - FAILED TO READ", filename);
				all_verified = false;
			}
		}

		if all_verified {
			std::fs::write(
				"/tmp/spacedrive-pull-test/bob_pull_success.txt",
				format!("success:{}", chrono::Utc::now().timestamp()),
			)
			.unwrap();
			std::fs::write("/tmp/spacedrive-pull-test/bob_success.txt", "success").unwrap();
			println!("PULL_TEST_SUCCESS: Bob successfully PULLED all files from Alice");
		} else {
			panic!("Bob: Some files failed verification");
		}
	} else {
		println!(
			"Bob: Only received {}/{} expected files via PULL",
			received_files.len(),
			source_files_info.len()
		);
		panic!("Bob: Not all files were pulled");
	}

	println!("Bob: PULL test completed");

	// Cleanup: shutdown core to release file descriptors
	core.shutdown().await.expect("Failed to shutdown Bob core");
}

/// Main test orchestrator for PULL operations
#[tokio::test]
async fn test_file_copy_pull() {
	// Clean up old test files
	let _ = std::fs::remove_dir_all("/tmp/spacedrive-pull-test");
	std::fs::create_dir_all("/tmp/spacedrive-pull-test").unwrap();

	println!("Testing bidirectional file copy - PULL direction");
	println!("Alice will host files, Bob will PULL them");

	let mut runner = CargoTestRunner::for_test_file("file_copy_pull_test")
		.with_timeout(Duration::from_secs(180))
		.add_subprocess("alice", "alice_pull_source_scenario")
		.add_subprocess("bob", "bob_pull_receiver_scenario");

	// Start Alice first (file host)
	println!("Starting Alice as PULL source (file host)...");
	runner
		.spawn_single_process("alice")
		.await
		.expect("Failed to spawn Alice");

	// Wait for Alice to initialize and create files
	tokio::time::sleep(Duration::from_secs(8)).await;

	// Start Bob (PULL initiator)
	println!("Starting Bob as PULL initiator...");
	runner
		.spawn_single_process("bob")
		.await
		.expect("Failed to spawn Bob");

	// Wait for both to complete
	let result = runner
		.wait_for_success(|_outputs| {
			let alice_success =
				std::fs::read_to_string("/tmp/spacedrive-pull-test/alice_success.txt")
					.map(|content| content.trim() == "success")
					.unwrap_or(false);
			let bob_success = std::fs::read_to_string("/tmp/spacedrive-pull-test/bob_success.txt")
				.map(|content| content.trim() == "success")
				.unwrap_or(false);

			alice_success && bob_success
		})
		.await;

	match result {
		Ok(_) => {
			println!("PULL transfer test successful! Bidirectional copy works correctly.");
		}
		Err(e) => {
			println!("PULL transfer test failed: {}", e);
			for (name, output) in runner.get_all_outputs() {
				println!("\n{} output:\n{}", name, output);
			}
			panic!("PULL transfer test failed");
		}
	}
}
