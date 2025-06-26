//! Core pairing test using the new flexible test framework
//!
//! Uses the SimpleTestRunner with real Core scenarios instead of separate binaries

use sd_core_new::test_framework::SimpleTestRunner;
use std::time::Duration;
use tokio::process::Command;

#[tokio::test]
async fn test_core_pairing_subprocess() {
	const PAIRING_CODE_PATH: &str = "/tmp/spacedrive-pairing-test/pairing_code.txt";

	// Clean up stale pairing code file from previous test runs
	// This prevents Bob from reading old data and fixes the file I/O race condition
	if std::path::Path::new(PAIRING_CODE_PATH).exists() {
		let _ = std::fs::remove_file(PAIRING_CODE_PATH);
		println!("🧹 Cleaned up stale pairing code file");
	}

	println!("🧪 Testing Core pairing methods using new test framework");

	let mut runner = SimpleTestRunner::new()
		.with_timeout(Duration::from_secs(90))
		.add_process("alice")
		.add_process("bob");

	// Start Alice as initiator
	runner
		.spawn_process("alice", |data_dir| {
			let mut cmd = Command::new("cargo");
			cmd.args(&[
				"run",
				"--bin",
				"test_core",
				"--",
				"--mode",
				"initiator",
				"--data-dir",
				data_dir.to_str().unwrap(),
				"--device-name",
				"Alice's Test Device",
			]);
			cmd
		})
		.await
		.expect("Failed to spawn Alice process");

	// Wait for Alice to initialize and generate pairing code
	tokio::time::sleep(Duration::from_secs(2)).await;

	// Start Bob as joiner
	runner
		.spawn_process("bob", |data_dir| {
			let mut cmd = Command::new("cargo");
			cmd.args(&[
				"run",
				"--bin",
				"test_core",
				"--",
				"--mode",
				"joiner",
				"--data-dir",
				data_dir.to_str().unwrap(),
				"--device-name",
				"Bob's Test Device",
			]);
			cmd
		})
		.await
		.expect("Failed to spawn Bob process");

	// Wait for both to succeed
	let result =
		runner
			.wait_until(|outputs| {
				let alice_success =
					outputs
						.get("alice")
						.map(|out| {
							out.contains("PAIRING_SUCCESS: Alice's Test Device connected to Bob successfully")
						})
						.unwrap_or(false);
				let bob_success =
					outputs
						.get("bob")
						.map(|out| {
							out.contains("PAIRING_SUCCESS: Bob's Test Device connected to Alice successfully")
						})
						.unwrap_or(false);

				alice_success && bob_success
			})
			.await;

	// Clean up
	runner.kill_all().await;

	match result {
		Ok(_) => {
			println!("🎉 Core pairing test successful with mutual device recognition!");
		}
		Err(e) => {
			println!("❌ Pairing test failed: {}", e);
			// for (name, output) in runner.get_all_outputs() {
			// 	println!("\n{} output:\n{}", name, output);
			// }
			panic!("Pairing test failed - devices did not properly recognize each other");
		}
	}
}
