//! Core cross-device file copy test using the test framework
//!
//! Tests the complete file copy workflow: pairing + file transfer

use sd_core_new::test_framework::SimpleTestRunner;
use std::time::Duration;
use tokio::process::Command;

#[tokio::test]
async fn test_cross_device_file_copy() {
	const PAIRING_CODE_PATH: &str = "/tmp/spacedrive-file-copy-test/pairing_code.txt";
	const EXPECTED_FILES_PATH: &str = "/tmp/spacedrive-file-copy-test/expected_files.txt";

	// Clean up stale files from previous test runs
	if std::path::Path::new(PAIRING_CODE_PATH).exists() {
		let _ = std::fs::remove_file(PAIRING_CODE_PATH);
	}
	if std::path::Path::new(EXPECTED_FILES_PATH).exists() {
		let _ = std::fs::remove_file(EXPECTED_FILES_PATH);
	}
	if std::path::Path::new("/tmp/received_files").exists() {
		let _ = std::fs::remove_dir_all("/tmp/received_files");
	}

	println!("🧪 Testing cross-device file copy using Core API and job system");

	let mut runner = SimpleTestRunner::new()
		.with_timeout(Duration::from_secs(120)) // Longer timeout for file transfer
		.add_process("alice")
		.add_process("bob");

	// Start Alice as file copy sender
	runner
		.spawn_process("alice", |data_dir| {
			let mut cmd = Command::new("cargo");
			cmd.args(&[
				"run",
				"--bin",
				"test_core",
				"--",
				"--mode",
				"file_copy_sender",
				"--data-dir",
				data_dir.to_str().unwrap(),
				"--device-name",
				"Alice-FileCopy",
			]);
			cmd
		})
		.await
		.expect("Failed to spawn Alice process");

	// Wait for Alice to initialize and generate pairing code
	tokio::time::sleep(Duration::from_secs(3)).await;

	// Start Bob as file copy receiver
	runner
		.spawn_process("bob", |data_dir| {
			let mut cmd = Command::new("cargo");
			cmd.args(&[
				"run",
				"--bin",
				"test_core",
				"--",
				"--mode",
				"file_copy_receiver",
				"--data-dir",
				data_dir.to_str().unwrap(),
				"--device-name",
				"Bob-FileCopy",
			]);
			cmd
		})
		.await
		.expect("Failed to spawn Bob process");

	// Wait for file copy to complete
	let result = runner
		.wait_until(|outputs| {
			let alice_success = outputs
				.get("alice")
				.map(|out| out.contains("FILE_COPY_SUCCESS: Alice-FileCopy completed file transfer"))
				.unwrap_or(false);
			let bob_success = outputs
				.get("bob")
				.map(|out| out.contains("FILE_COPY_SUCCESS: Bob-FileCopy verified all received files"))
				.unwrap_or(false);

			alice_success && bob_success
		})
		.await;

	// Clean up
	runner.kill_all().await;

	match result {
		Ok(_) => {
			println!("🎉 Cross-device file copy test successful!");
			println!("   ✅ Device pairing completed");
			println!("   ✅ File transfer initiated via job system");
			println!("   ✅ Files transferred and verified");
		}
		Err(e) => {
			println!("❌ File copy test failed: {}", e);
			for (name, output) in runner.get_all_outputs() {
				println!("\n{} output:\n{}", name, output);
			}
			panic!("Cross-device file copy test failed");
		}
	}
}