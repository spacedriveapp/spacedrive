//! File transfer test with daemon restart to test session key persistence
//!
//! This test demonstrates that session keys survive a daemon restart.
//! Alice and Bob pair, then Alice restarts her daemon, and finally transfers files.

use sd_core::{domain::content_identity::ContentHashGenerator, testing::CargoTestRunner, Core};
use std::{env, path::PathBuf, time::Duration};
use tokio::time::timeout;

#[path = "helpers/wait.rs"]
mod wait;
use wait::{
	wait_for_connected_device, wait_for_dir_entries, wait_for_file, wait_for_file_matching,
	wait_for_paired_device, wait_for_peer, wait_for_transfer,
};

/// Alice's scenario - pairs, restarts, then sends files
#[tokio::test]
#[ignore]
async fn alice_restart_scenario() {
	if env::var("TEST_ROLE").unwrap_or_default() != "alice" {
		return;
	}

	env::set_var("SPACEDRIVE_TEST_DIR", "/tmp/spacedrive-restart-test");

	let data_dir = PathBuf::from("/tmp/spacedrive-restart-test/alice");
	let device_name = "Alice's Test Device";

	println!("Alice: Starting Core for initial pairing");
	let mut core = timeout(Duration::from_secs(10), Core::new(data_dir.clone()))
		.await
		.unwrap()
		.unwrap();
	core.device.set_name(device_name.to_string()).unwrap();

	println!("Alice: Initializing networking for pairing...");
	timeout(Duration::from_secs(10), core.init_networking())
		.await
		.unwrap()
		.unwrap();

	// Create library
	println!("Alice: Creating library...");
	let _library = core
		.libraries
		.create_library("Alice Transfer Library", None, core.context.clone())
		.await
		.unwrap();

	// Start pairing
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

	println!("Alice: Pairing code generated (expires in {}s)", expires_in);

	// Write pairing code for Bob
	std::fs::create_dir_all("/tmp/spacedrive-restart-test").unwrap();
	std::fs::write(
		"/tmp/spacedrive-restart-test/pairing_code.txt",
		&pairing_code,
	)
	.unwrap();

	// Wait for Bob to connect
	println!("Alice: Waiting for Bob to connect...");
	let receiver_id = wait_for_paired_device(&core, Duration::from_secs(45))
		.await
		.expect("Bob never connected");
	println!("Alice: Pairing complete with device {}", receiver_id);

	// Signal that pairing is complete
	std::fs::write("/tmp/spacedrive-restart-test/alice_paired.txt", "paired").unwrap();

	// === RESTART SIMULATION ===
	println!("Alice: ========== RESTARTING DAEMON ==========");
	println!("Alice: Shutting down Core to simulate daemon restart...");
	core.shutdown().await.expect("shutdown");

	println!("Alice: Starting fresh Core instance with same data dir...");
	let mut core = timeout(Duration::from_secs(10), Core::new(data_dir.clone()))
		.await
		.unwrap()
		.unwrap();
	core.device.set_name(device_name.to_string()).unwrap();

	println!("Alice: Re-initializing networking...");
	timeout(Duration::from_secs(10), core.init_networking())
		.await
		.unwrap()
		.unwrap();

	// Re-create library (required for job dispatch)
	println!("Alice: Re-creating library...");
	let _library = core
		.libraries
		.create_library("Alice Transfer Library", None, core.context.clone())
		.await
		.unwrap();

	// Check if paired devices were loaded from persistence
	println!("Alice: Checking if paired devices were loaded after restart...");
	if let Some(networking) = core.networking() {
		let device_registry = networking.device_registry();
		let registry = device_registry.read().await;
		let paired_devices = registry.get_paired_devices();
		println!(
			"Alice: Found {} paired devices after restart",
			paired_devices.len()
		);
		for device in &paired_devices {
			println!("  - {} (ID: {})", device.device_name, device.device_id);
		}

		if paired_devices.is_empty() {
			panic!("Alice: FAILED - No paired devices loaded after restart!");
		}
		if !paired_devices.iter().any(|d| d.device_id == receiver_id) {
			panic!(
				"Alice: FAILED - Bob's device {} not found in paired devices!",
				receiver_id
			);
		}
	}

	println!("Alice: ========== RESTART COMPLETE ==========");

	// Create test files
	println!("Alice: Creating test files for transfer...");
	let test_files_dir = data_dir.join("test_files");
	std::fs::create_dir_all(&test_files_dir).unwrap();

	let medium_content = "B".repeat(2048); // 2KB
	let test_files = vec![
		("restart_test_1.txt", "File sent after restart #1"),
		("restart_test_2.txt", medium_content.as_str()),
	];

	let mut source_paths = Vec::new();
	for (filename, content) in &test_files {
		let file_path = test_files_dir.join(filename);
		std::fs::write(&file_path, content).unwrap();
		match ContentHashGenerator::generate_content_hash(&file_path).await {
			Ok(checksum) => {
				println!(
					"  Created: {} ({} bytes, checksum: {})",
					filename,
					content.len(),
					checksum
				);
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

	// Write file list for Bob
	let file_list: Vec<String> = test_files
		.iter()
		.map(|(name, content)| format!("{}:{}", name, content.len()))
		.collect();
	std::fs::write(
		"/tmp/spacedrive-restart-test/expected_files.txt",
		file_list.join("\n"),
	)
	.unwrap();

	// The transfer dials on demand, so a reconnection that has not happened
	// yet is not fatal; waiting for one first keeps the send off the slow path.
	if wait_for_peer(&core, receiver_id, Duration::from_secs(15))
		.await
		.is_ok()
	{
		println!("Alice: Bob reconnected after restart");
	} else {
		println!("Alice: Bob not reconnected yet, the transfer will dial");
	}

	// Initiate file transfer AFTER restart
	println!("Alice: Initiating file transfer after restart...");
	println!("Alice: Sending files to device ID: {}", receiver_id);

	let transfer_results = core
		.services
		.file_sharing
		.share_with_device(
			source_paths,
			receiver_id,
			Some(PathBuf::from("/tmp/received_files_restart")),
		)
		.await;

	match transfer_results {
		Ok(transfer_id) => {
			println!("Alice: File transfer initiated successfully!");

			// Wait for transfer to complete
			let completed =
				match wait_for_transfer(&core, &transfer_id, Duration::from_secs(30)).await {
					Ok(true) => {
						println!("Alice: Transfer completed successfully");
						true
					}
					Ok(false) => panic!("Alice: File transfer failed after restart"),
					Err(_) => false,
				};

			if !completed {
				panic!("Alice: Transfer did not complete in time");
			}

			// Wait for Bob's confirmation
			println!("Alice: Waiting for Bob's confirmation...");
			if wait_for_file_matching(
				"/tmp/spacedrive-restart-test/bob_received_confirmation.txt",
				Duration::from_secs(60),
				|content| content.starts_with("received_and_verified:"),
			)
			.await
			.is_ok()
			{
				println!("Alice: Bob confirmed file receipt!");
				std::fs::write("/tmp/spacedrive-restart-test/alice_success.txt", "success")
					.unwrap();
				println!("RESTART_TEST_SUCCESS: Files transferred successfully after restart!");
				return;
			}

			panic!("Alice: Bob did not confirm file receipt");
		}
		Err(e) => {
			panic!(
				"Alice: File transfer initiation failed after restart: {}",
				e
			);
		}
	}
}

/// Bob's scenario - pairs and waits for files
#[tokio::test]
#[ignore]
async fn bob_restart_scenario() {
	if env::var("TEST_ROLE").unwrap_or_default() != "bob" {
		return;
	}

	env::set_var("SPACEDRIVE_TEST_DIR", "/tmp/spacedrive-restart-test");

	let data_dir = PathBuf::from("/tmp/spacedrive-restart-test/bob");
	let device_name = "Bob's Test Device";

	println!("Bob: Starting Core");
	let mut core = timeout(Duration::from_secs(10), Core::new(data_dir))
		.await
		.unwrap()
		.unwrap();
	core.device.set_name(device_name.to_string()).unwrap();

	println!("Bob: Initializing networking...");
	timeout(Duration::from_secs(10), core.init_networking())
		.await
		.unwrap()
		.unwrap();

	// Create library
	println!("Bob: Creating library...");
	let _library = core
		.libraries
		.create_library("Bob Transfer Library", None, core.context.clone())
		.await
		.unwrap();

	// Wait for Alice's pairing code
	println!("Bob: Looking for pairing code from Alice...");
	let pairing_code = wait_for_file(
		"/tmp/spacedrive-restart-test/pairing_code.txt",
		Duration::from_secs(60),
	)
	.await
	.expect("Bob: Alice never wrote a pairing code");

	// Join pairing
	println!("Bob: Joining pairing with Alice...");
	if let Some(networking) = core.networking() {
		timeout(
			Duration::from_secs(15),
			networking.start_pairing_as_joiner(&pairing_code, false),
		)
		.await
		.unwrap()
		.unwrap();
	}

	// Wait for pairing to complete
	println!("Bob: Waiting for pairing to complete...");
	wait_for_connected_device(&core, Duration::from_secs(30))
		.await
		.expect("Bob: Pairing timeout - no devices connected");
	println!("Bob: Pairing completed!");

	// Wait for Alice to finish pairing
	wait_for_file(
		"/tmp/spacedrive-restart-test/alice_paired.txt",
		Duration::from_secs(60),
	)
	.await
	.expect("Bob: Alice never confirmed pairing");
	println!("Bob: Alice confirmed pairing complete");

	println!("Bob: Waiting for Alice to restart and send files...");

	// Create directory for received files and allow it as a transfer destination
	let received_dir = std::path::Path::new("/tmp/received_files_restart");
	std::fs::create_dir_all(received_dir).unwrap();
	if let Some(networking) = core.networking() {
		let protocol_registry = networking.protocol_registry();
		let registry = protocol_registry.read().await;
		if let Some(handler) = registry.get_handler("file_transfer") {
			if let Some(ft_handler) =
				handler
					.as_any()
					.downcast_ref::<sd_core::service::network::protocol::FileTransferProtocolHandler>(
					) {
				ft_handler.add_allowed_path(received_dir.to_path_buf());
				println!("Bob: Added {} as allowed path", received_dir.display());
			}
		}
	}

	// Wait for expected files list
	let expected_files = wait_for_file(
		"/tmp/spacedrive-restart-test/expected_files.txt",
		Duration::from_secs(120),
	)
	.await
	.expect("Bob: Alice never wrote the expected file list")
	.lines()
	.map(|line| {
		let parts: Vec<&str> = line.split(':').collect();
		(parts[0].to_string(), parts[1].parse::<usize>().unwrap_or(0))
	})
	.collect::<Vec<(String, usize)>>();

	println!("Bob: Expecting {} files", expected_files.len());

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

	// Verify files
	if received_files.len() == expected_files.len() {
		println!("Bob: All files received!");

		let mut verification_success = true;
		for (expected_name, expected_size) in &expected_files {
			let received_path = received_dir.join(expected_name);
			if received_path.exists() {
				if let Ok(metadata) = std::fs::metadata(&received_path) {
					if metadata.len() == *expected_size as u64 {
						match ContentHashGenerator::generate_content_hash(&received_path).await {
							Ok(checksum) => {
								println!(
									"Bob: Verified: {} (checksum: {})",
									expected_name, checksum
								);
							}
							Err(e) => {
								println!("Bob: Checksum error for {}: {}", expected_name, e);
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
				}
			} else {
				println!("Bob: File not found: {}", expected_name);
				verification_success = false;
			}
		}

		if verification_success {
			println!("RESTART_TEST_SUCCESS: Bob verified all files after Alice's restart");
			std::fs::write("/tmp/spacedrive-restart-test/bob_success.txt", "success").unwrap();

			let timestamp = std::time::SystemTime::now()
				.duration_since(std::time::UNIX_EPOCH)
				.unwrap()
				.as_secs();
			std::fs::write(
				"/tmp/spacedrive-restart-test/bob_received_confirmation.txt",
				format!("received_and_verified:{}", timestamp),
			)
			.unwrap();

			// The sender's job still awaits the final ack for the last file;
			// stay up until Alice reports it, or the ack dies with the process.
			wait_for_file(
				"/tmp/spacedrive-restart-test/alice_success.txt",
				Duration::from_secs(60),
			)
			.await
			.expect("Bob: Alice never reported the transfer complete");
		} else {
			panic!("Bob: File verification failed");
		}
	} else {
		panic!(
			"Bob: Only received {}/{} files",
			received_files.len(),
			expected_files.len()
		);
	}
}

/// Test orchestrator
#[tokio::test]
async fn test_file_transfer_with_restart() {
	// Clean up
	let _ = std::fs::remove_dir_all("/tmp/spacedrive-restart-test");
	let _ = std::fs::remove_dir_all("/tmp/received_files_restart");
	std::fs::create_dir_all("/tmp/spacedrive-restart-test").unwrap();

	println!("Testing file transfer with daemon restart");

	let mut runner = CargoTestRunner::for_test_file("file_transfer_with_restart_test")
		.with_timeout(Duration::from_secs(300)) // 5 minutes
		.add_subprocess("alice", "alice_restart_scenario")
		.add_subprocess("bob", "bob_restart_scenario");

	// Start Alice
	println!("Starting Alice...");
	runner
		.spawn_single_process("alice")
		.await
		.expect("Failed to spawn Alice");

	// Bob waits for the pairing code file himself, so he can start at once.
	// Start Bob
	println!("Starting Bob...");
	runner
		.spawn_single_process("bob")
		.await
		.expect("Failed to spawn Bob");

	// Wait for success
	let result = runner
		.wait_for_success(|_outputs| {
			let alice_success =
				std::fs::read_to_string("/tmp/spacedrive-restart-test/alice_success.txt")
					.map(|content| content.trim() == "success")
					.unwrap_or(false);
			let bob_success =
				std::fs::read_to_string("/tmp/spacedrive-restart-test/bob_success.txt")
					.map(|content| content.trim() == "success")
					.unwrap_or(false);

			alice_success && bob_success
		})
		.await;

	match result {
		Ok(_) => {
			println!("SUCCESS: File transfer worked after daemon restart!");
		}
		Err(e) => {
			println!("FAILED: File transfer failed after restart: {}", e);
			for (name, output) in runner.get_all_outputs() {
				println!("\n{} output:\n{}", name, output);
			}
			panic!("File transfer with restart test failed");
		}
	}
}

const KILL_TEST_DIR: &str = "/tmp/spacedrive-kill-receive-test";
const KILL_RECEIVED_DIR: &str = "/tmp/received_files_kill";
const KILL_FILE_NAME: &str = "big.bin";
const KILL_OLD_CONTENTS: &[u8] = b"old contents, kept until the new file verifies";
/// Large enough that the receive is still in flight when the receiver dies.
const KILL_FILE_SIZE: usize = 256 * 1024 * 1024;

fn kill_marker(name: &str) -> PathBuf {
	PathBuf::from(KILL_TEST_DIR).join(name)
}

/// Names in the received directory, split into staging partials and the rest.
fn received_entries() -> (Vec<String>, Vec<String>) {
	let names: Vec<String> = std::fs::read_dir(KILL_RECEIVED_DIR)
		.unwrap()
		.filter_map(|entry| entry.ok())
		.map(|entry| entry.file_name().to_string_lossy().to_string())
		.collect();
	names
		.into_iter()
		.partition(|name| name.ends_with(".sdpart"))
}

async fn allow_kill_received_dir(core: &Core) {
	let networking = core.networking().expect("networking initialized");
	let protocol_registry = networking.protocol_registry();
	let registry = protocol_registry.read().await;
	let handler = registry
		.get_handler("file_transfer")
		.expect("file transfer handler registered");
	handler
		.as_any()
		.downcast_ref::<sd_core::service::network::protocol::FileTransferProtocolHandler>()
		.expect("file transfer handler type")
		.add_allowed_path(PathBuf::from(KILL_RECEIVED_DIR));
}

/// Alice sends a large file, and sends it again once Bob is back.
#[tokio::test]
#[ignore]
async fn alice_kill_receive_scenario() {
	if env::var("TEST_ROLE").unwrap_or_default() != "alice" {
		return;
	}
	env::set_var("SPACEDRIVE_TEST_DIR", KILL_TEST_DIR);
	let data_dir = PathBuf::from(KILL_TEST_DIR).join("alice");

	let mut core = timeout(Duration::from_secs(10), Core::new(data_dir.clone()))
		.await
		.unwrap()
		.unwrap();
	core.device
		.set_name("Alice's Test Device".to_string())
		.unwrap();
	timeout(Duration::from_secs(10), core.init_networking())
		.await
		.unwrap()
		.unwrap();
	let _library = core
		.libraries
		.create_library("Alice Kill Library", None, core.context.clone())
		.await
		.unwrap();

	let (pairing_code, _) = timeout(
		Duration::from_secs(15),
		core.networking()
			.expect("networking initialized")
			.start_pairing_as_initiator(false),
	)
	.await
	.unwrap()
	.unwrap();
	std::fs::write(kill_marker("pairing_code.txt"), &pairing_code).unwrap();
	let receiver_id = wait_for_paired_device(&core, Duration::from_secs(45))
		.await
		.expect("Alice: Bob never connected");
	wait_for_file(kill_marker("bob_paired.txt"), Duration::from_secs(60))
		.await
		.expect("Alice: Bob never confirmed pairing");

	let source_dir = data_dir.join("test_files");
	std::fs::create_dir_all(&source_dir).unwrap();
	let source = source_dir.join(KILL_FILE_NAME);
	let content: Vec<u8> = (0..KILL_FILE_SIZE / 4)
		.flat_map(|i| (i as u32).wrapping_mul(2_654_435_761).to_le_bytes())
		.collect();
	std::fs::write(&source, &content).unwrap();
	std::fs::write(
		kill_marker("expected_hash.txt"),
		blake3::hash(&content).to_hex().as_str(),
	)
	.unwrap();
	drop(content);

	tracing::info!("Alice: Sending the first copy, which Bob will not live to finish");
	core.services
		.file_sharing
		.share_with_device(
			vec![source.clone()],
			receiver_id,
			Some(PathBuf::from(KILL_RECEIVED_DIR)),
		)
		.await
		.expect("Alice: first share failed to start");

	wait_for_file(kill_marker("bob_restarted.txt"), Duration::from_secs(120))
		.await
		.expect("Alice: Bob never came back");
	if wait_for_peer(&core, receiver_id, Duration::from_secs(30))
		.await
		.is_err()
	{
		tracing::info!("Alice: Bob not reconnected yet, the transfer will dial");
	}

	tracing::info!("Alice: Sending the file again to the restarted Bob");
	let transfer_id = core
		.services
		.file_sharing
		.share_with_device(
			vec![source],
			receiver_id,
			Some(PathBuf::from(KILL_RECEIVED_DIR)),
		)
		.await
		.expect("Alice: second share failed to start");
	let completed = wait_for_transfer(&core, &transfer_id, Duration::from_secs(180))
		.await
		.unwrap_or(false);
	assert!(completed, "Alice: the second transfer did not complete");
	std::fs::write(kill_marker("alice_success.txt"), "success").unwrap();

	let _ = wait_for_file(kill_marker("done.txt"), Duration::from_secs(60)).await;
}

/// Bob receives. The orchestrator kills the first instance mid-receive and
/// starts a second one on the same data directory.
#[tokio::test]
#[ignore]
async fn bob_kill_receive_scenario() {
	let role = env::var("TEST_ROLE").unwrap_or_default();
	if role != "bob" && role != "bob_restarted" {
		return;
	}
	env::set_var("SPACEDRIVE_TEST_DIR", KILL_TEST_DIR);
	let data_dir = PathBuf::from(KILL_TEST_DIR).join("bob");

	let mut core = timeout(Duration::from_secs(10), Core::new(data_dir))
		.await
		.unwrap()
		.unwrap();
	core.device
		.set_name("Bob's Test Device".to_string())
		.unwrap();
	timeout(Duration::from_secs(10), core.init_networking())
		.await
		.unwrap()
		.unwrap();
	allow_kill_received_dir(&core).await;

	if role == "bob" {
		let _library = core
			.libraries
			.create_library("Bob Kill Library", None, core.context.clone())
			.await
			.unwrap();
		let pairing_code = wait_for_file(kill_marker("pairing_code.txt"), Duration::from_secs(60))
			.await
			.expect("Bob: Alice never wrote a pairing code");
		timeout(
			Duration::from_secs(15),
			core.networking()
				.expect("networking initialized")
				.start_pairing_as_joiner(&pairing_code, false),
		)
		.await
		.unwrap()
		.unwrap();
		wait_for_paired_device(&core, Duration::from_secs(30))
			.await
			.expect("Bob: pairing timeout");
		std::fs::write(kill_marker("bob_paired.txt"), "paired").unwrap();
	} else {
		std::fs::write(kill_marker("bob_restarted.txt"), "restarted").unwrap();
	}

	let _ = wait_for_file(kill_marker("done.txt"), Duration::from_secs(300)).await;
}

/// A receiver killed mid-transfer leaves the destination as it was, and a
/// fresh send after its restart lands the complete file.
///
/// Peer transfers do not resume, so the restart is expected to remove the
/// partial the kill left and the second send starts from the first byte.
#[tokio::test]
async fn test_file_transfer_kill_mid_receive() {
	let _ = std::fs::remove_dir_all(KILL_TEST_DIR);
	let _ = std::fs::remove_dir_all(KILL_RECEIVED_DIR);
	std::fs::create_dir_all(KILL_TEST_DIR).unwrap();
	std::fs::create_dir_all(KILL_RECEIVED_DIR).unwrap();
	let destination = PathBuf::from(KILL_RECEIVED_DIR).join(KILL_FILE_NAME);
	std::fs::write(&destination, KILL_OLD_CONTENTS).unwrap();

	let mut runner = CargoTestRunner::for_test_file("file_transfer_with_restart_test")
		.with_timeout(Duration::from_secs(300))
		.add_subprocess("alice", "alice_kill_receive_scenario")
		.add_subprocess("bob", "bob_kill_receive_scenario")
		.add_subprocess("bob_restarted", "bob_kill_receive_scenario");
	runner.spawn_single_process("alice").await.unwrap();
	runner.spawn_single_process("bob").await.unwrap();

	// Kill Bob as soon as received bytes exist on disk.
	let started = std::time::Instant::now();
	loop {
		let (partials, _) = received_entries();
		let receiving = partials.iter().any(|name| {
			std::fs::metadata(PathBuf::from(KILL_RECEIVED_DIR).join(name))
				.is_ok_and(|meta| meta.len() > 0)
		});
		if receiving {
			break;
		}
		assert_eq!(
			std::fs::read(&destination).unwrap(),
			KILL_OLD_CONTENTS,
			"the transfer replaced the destination before the kill landed"
		);
		if started.elapsed() > Duration::from_secs(180) {
			runner.kill_all().await;
			panic!("no receive started within 180s");
		}
		tokio::time::sleep(Duration::from_millis(2)).await;
	}
	runner.kill_process("bob").await.unwrap();
	tracing::info!("Orchestrator: killed Bob mid-receive");

	assert_eq!(
		std::fs::read(&destination).unwrap(),
		KILL_OLD_CONTENTS,
		"a killed receive must leave the old destination untouched"
	);
	let (partials, others) = received_entries();
	assert_eq!(others, vec![KILL_FILE_NAME.to_string()]);
	assert_eq!(
		partials.len(),
		1,
		"the kill leaves exactly one hidden partial for the restart to remove"
	);

	runner.spawn_single_process("bob_restarted").await.unwrap();
	wait_for_file(kill_marker("bob_restarted.txt"), Duration::from_secs(60))
		.await
		.expect("restarted Bob never came up");
	let (partials, _) = received_entries();
	assert!(
		partials.is_empty(),
		"the restart must remove the partial the kill left: {:?}",
		partials
	);

	let result = runner
		.wait_for_success(|_| kill_marker("alice_success.txt").exists())
		.await;
	std::fs::write(kill_marker("done.txt"), "done").unwrap();
	if let Err(e) = result {
		panic!("the transfer after the restart did not complete: {}", e);
	}

	let expected_hash = std::fs::read_to_string(kill_marker("expected_hash.txt")).unwrap();
	let received = std::fs::read(&destination).unwrap();
	assert_eq!(received.len(), KILL_FILE_SIZE);
	assert_eq!(
		blake3::hash(&received).to_hex().as_str(),
		expected_hash.trim()
	);
	let (partials, others) = received_entries();
	assert!(partials.is_empty(), "no partial may remain: {:?}", partials);
	assert_eq!(others, vec![KILL_FILE_NAME.to_string()]);
}
