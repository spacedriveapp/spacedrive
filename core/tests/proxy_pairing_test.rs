//! Proxy pairing test using the cargo test subprocess framework
//!
//! This is a STRICT test that demonstrates proxy/vouching-based pairing:
//! 1. Alice pairs with Carol (direct pairing) - VERIFIED
//! 2. Alice pairs with Bob (direct pairing) - VERIFIED
//! 3. Alice auto-vouches Bob to Carol (proxy pairing) - VERIFIED
//! 4. Carol auto-accepts the vouch - VERIFIED
//! 5. Bob receives ProxyPairingComplete - VERIFIED
//!
//! The test FAILS if:
//! - Bob does not appear in Carol's paired devices list (Carol panics)
//! - Carol does not appear in Bob's paired devices list (Bob panics)
//! - Any device times out waiting for proxy pairing (60 second limit)
//!
//! Config: auto_vouch_to_all=true, auto_accept_vouched=true
//!
//! The roles coordinate through marker files and each stays up until the
//! peers it exchanged messages with have written their success marker, so
//! no exit can cut a vouch, an acceptance or a completion still in flight.

use sd_core::testing::CargoTestRunner;
use sd_core::Core;
use std::env;
use std::path::PathBuf;
use std::time::Duration;
use tokio::time::timeout;

#[path = "helpers/wait.rs"]
mod wait;
use wait::{wait_for_file, wait_until};

const TEST_DIR: &str = "/tmp/spacedrive-proxy-pairing-test";

fn marker(name: &str) -> String {
	format!("{TEST_DIR}/{name}")
}

/// The names of every device in the paired registry.
async fn paired_names(core: &Core) -> Vec<String> {
	let Some(networking) = core.networking() else {
		return vec![];
	};
	networking
		.device_registry()
		.read()
		.await
		.get_paired_devices()
		.into_iter()
		.map(|device| device.device_name)
		.collect()
}

/// Wait until a device with `name` is in the paired registry and return its id.
async fn wait_for_paired_name(core: &Core, name: &str, deadline: Duration) -> uuid::Uuid {
	let found = wait_until(&format!("{name} in paired devices"), deadline, || async {
		let networking = core.networking()?;
		let paired = networking
			.device_registry()
			.read()
			.await
			.get_paired_devices();
		paired
			.iter()
			.find(|device| device.device_name == name)
			.map(|device| device.device_id)
	})
	.await;
	match found {
		Ok(id) => id,
		Err(e) => panic!("{e}; paired devices: {:?}", paired_names(core).await),
	}
}

/// Alice's scenario - pairs with Carol first, then Bob, then vouches Bob to Carol
#[tokio::test]
#[ignore]
async fn alice_proxy_pairing_scenario() {
	if env::var("TEST_ROLE").unwrap_or_default() != "alice" {
		return;
	}

	let data_dir = PathBuf::from(marker("alice"));
	let device_name = "Alice's Test Device";

	println!("Alice: Starting proxy pairing test");
	println!("Alice: Data dir: {:?}", data_dir);

	println!("Alice: Initializing Core...");
	let mut core = timeout(Duration::from_secs(10), Core::new(data_dir))
		.await
		.unwrap()
		.unwrap();
	println!("Alice: Core initialized");

	core.device.set_name(device_name.to_string()).unwrap();

	println!("Alice: Enabling auto-vouch for testing...");
	{
		let mut config = core.config.write().await;
		config.proxy_pairing.auto_vouch_to_all = true;
		config.save().unwrap();
	}
	println!("Alice: Auto-vouch enabled");

	println!("Alice: Initializing networking...");
	timeout(Duration::from_secs(10), core.init_networking())
		.await
		.unwrap()
		.unwrap();
	println!("Alice: Networking initialized");

	let networking = core.networking().expect("Networking not initialized");

	println!("\n=== PHASE 1: Alice pairs with Carol ===");
	let (pairing_code_carol, _) = timeout(
		Duration::from_secs(15),
		networking.start_pairing_as_initiator(false),
	)
	.await
	.unwrap()
	.unwrap();
	println!("Alice: Pairing code for Carol: {}", pairing_code_carol);
	std::fs::write(marker("pairing_code_carol.txt"), &pairing_code_carol).unwrap();

	println!("Alice: Waiting for Carol to pair...");
	let carol_id =
		wait_for_paired_name(&core, "Carol's Test Device", Duration::from_secs(45)).await;
	println!("Alice: Carol paired successfully! (ID: {carol_id})");
	std::fs::write(marker("alice_carol_paired.txt"), "success").unwrap();

	println!("\n=== PHASE 2: Alice pairs with Bob ===");
	let (pairing_code_bob, _) = timeout(
		Duration::from_secs(15),
		networking.start_pairing_as_initiator(false),
	)
	.await
	.unwrap()
	.unwrap();
	println!("Alice: Pairing code for Bob: {}", pairing_code_bob);
	std::fs::write(marker("pairing_code_bob.txt"), &pairing_code_bob).unwrap();

	println!("Alice: Waiting for Bob to pair...");
	let bob_id = wait_for_paired_name(&core, "Bob's Test Device", Duration::from_secs(45)).await;
	println!("Alice: Bob paired successfully! (ID: {bob_id})");
	std::fs::write(marker("alice_bob_paired.txt"), "success").unwrap();

	println!("\n=== PHASE 3: Alice vouches Bob to Carol ===");
	println!(
		"Alice: Auto-vouch enabled - ProxyPairingRequest for Bob ({bob_id}) goes to Carol ({carol_id})"
	);

	// Alice relays Carol's acceptance to Bob as ProxyPairingComplete, so she
	// stays up until both have stored each other.
	println!("Alice: Waiting for Carol and Bob to confirm the proxy pairing...");
	wait_for_file(marker("carol_success.txt"), Duration::from_secs(90))
		.await
		.expect("Alice: Carol never confirmed the proxy pairing");
	wait_for_file(marker("bob_success.txt"), Duration::from_secs(90))
		.await
		.expect("Alice: Bob never confirmed the proxy pairing");

	std::fs::write(marker("alice_success.txt"), "success").unwrap();
	println!("Alice: Test completed successfully");
}

/// Carol's scenario - pairs with Alice first, then accepts vouch for Bob
#[tokio::test]
#[ignore]
async fn carol_proxy_pairing_scenario() {
	if env::var("TEST_ROLE").unwrap_or_default() != "carol" {
		return;
	}

	let data_dir = PathBuf::from(marker("carol"));
	let device_name = "Carol's Test Device";

	println!("Carol: Starting proxy pairing test");
	println!("Carol: Data dir: {:?}", data_dir);

	println!("Carol: Initializing Core...");
	let mut core = timeout(Duration::from_secs(10), Core::new(data_dir))
		.await
		.unwrap()
		.unwrap();
	println!("Carol: Core initialized");

	core.device.set_name(device_name.to_string()).unwrap();
	println!("Carol: Auto-accept proxy pairing enabled (default)");

	println!("Carol: Initializing networking...");
	timeout(Duration::from_secs(10), core.init_networking())
		.await
		.unwrap()
		.unwrap();
	println!("Carol: Networking initialized");

	let networking = core.networking().expect("Networking not initialized");

	println!("\n=== PHASE 1: Carol pairs with Alice ===");
	println!("Carol: Waiting for pairing code from Alice...");
	let pairing_code = wait_for_file(marker("pairing_code_carol.txt"), Duration::from_secs(60))
		.await
		.expect("Carol: Alice never wrote a pairing code");
	println!("Carol: Found pairing code");

	timeout(
		Duration::from_secs(15),
		networking.start_pairing_as_joiner(&pairing_code, false),
	)
	.await
	.unwrap()
	.unwrap();
	println!("Carol: Joined pairing with Alice");

	let alice_id =
		wait_for_paired_name(&core, "Alice's Test Device", Duration::from_secs(30)).await;
	println!("Carol: Paired with Alice successfully! (ID: {alice_id})");

	println!("\n=== PHASE 2: Carol receives proxy pairing for Bob ===");
	println!("Carol: Should receive ProxyPairingRequest and auto-accept");
	let bob_id = wait_for_paired_name(&core, "Bob's Test Device", Duration::from_secs(60)).await;
	println!("Carol: Bob found via proxy! (ID: {bob_id})");
	println!("Carol: Proxy pairing SUCCESS - Bob paired via vouching!");
	std::fs::write(marker("carol_success.txt"), "success").unwrap();

	// Carol's acceptance travels Carol -> Alice -> Bob; stay up until Bob
	// reports it landed.
	wait_for_file(marker("bob_success.txt"), Duration::from_secs(90))
		.await
		.expect("Carol: Bob never confirmed the proxy pairing");
	println!("Carol: Test completed");
}

/// Bob's scenario - pairs with Alice, then gets vouched to Carol
#[tokio::test]
#[ignore]
async fn bob_proxy_pairing_scenario() {
	if env::var("TEST_ROLE").unwrap_or_default() != "bob" {
		return;
	}

	let data_dir = PathBuf::from(marker("bob"));
	let device_name = "Bob's Test Device";

	println!("Bob: Starting proxy pairing test");
	println!("Bob: Data dir: {:?}", data_dir);

	println!("Bob: Initializing Core...");
	let mut core = timeout(Duration::from_secs(10), Core::new(data_dir))
		.await
		.unwrap()
		.unwrap();
	println!("Bob: Core initialized");

	core.device.set_name(device_name.to_string()).unwrap();

	println!("Bob: Initializing networking...");
	timeout(Duration::from_secs(10), core.init_networking())
		.await
		.unwrap()
		.unwrap();
	println!("Bob: Networking initialized");

	let networking = core.networking().expect("Networking not initialized");

	println!("\n=== PHASE 1: Bob waits for Alice-Carol pairing ===");
	wait_for_file(marker("alice_carol_paired.txt"), Duration::from_secs(90))
		.await
		.expect("Bob: Alice and Carol never paired");
	println!("Bob: Alice and Carol are paired");

	println!("\n=== PHASE 2: Bob pairs with Alice ===");
	let pairing_code = wait_for_file(marker("pairing_code_bob.txt"), Duration::from_secs(60))
		.await
		.expect("Bob: Alice never wrote a pairing code");
	println!("Bob: Found pairing code");

	timeout(
		Duration::from_secs(15),
		networking.start_pairing_as_joiner(&pairing_code, false),
	)
	.await
	.unwrap()
	.unwrap();
	println!("Bob: Joined pairing with Alice");

	let alice_id =
		wait_for_paired_name(&core, "Alice's Test Device", Duration::from_secs(30)).await;
	println!("Bob: Paired with Alice successfully! (ID: {alice_id})");

	println!("\n=== PHASE 3: Bob receives proxy pairing confirmation ===");
	println!("Bob: STRICT CHECK - Carol must appear in paired devices via ProxyPairingComplete");
	let carol_id =
		wait_for_paired_name(&core, "Carol's Test Device", Duration::from_secs(60)).await;
	println!("Bob: Carol found via proxy! (ID: {carol_id})");
	println!("Bob: Proxy pairing SUCCESS - Carol paired via vouching!");
	std::fs::write(marker("bob_success.txt"), "success").unwrap();

	wait_for_file(marker("carol_success.txt"), Duration::from_secs(90))
		.await
		.expect("Bob: Carol never confirmed the proxy pairing");
	println!("Bob: Test completed");
}

/// Main test orchestrator - spawns three devices
#[tokio::test]
async fn test_proxy_pairing() {
	println!("Testing proxy/vouching pairing with three devices");

	let _ = std::fs::remove_dir_all(TEST_DIR);
	std::fs::create_dir_all(TEST_DIR).unwrap();

	let mut runner = CargoTestRunner::for_test_file("proxy_pairing_test")
		.with_timeout(Duration::from_secs(300))
		.add_subprocess("alice", "alice_proxy_pairing_scenario")
		.add_subprocess("carol", "carol_proxy_pairing_scenario")
		.add_subprocess("bob", "bob_proxy_pairing_scenario");

	// Carol and Bob wait for Alice's marker files themselves, so all three
	// start at once.
	for name in ["alice", "carol", "bob"] {
		runner
			.spawn_single_process(name)
			.await
			.unwrap_or_else(|e| panic!("Failed to spawn {name}: {e}"));
	}

	let result = runner
		.wait_for_success(|_outputs| {
			["alice", "carol", "bob"].iter().all(|name| {
				std::fs::read_to_string(marker(&format!("{name}_success.txt")))
					.map(|content| content.trim() == "success")
					.unwrap_or(false)
			})
		})
		.await;

	match result {
		Ok(_) => {
			println!("\nPROXY PAIRING TEST PASSED");
			println!("   Alice paired with Carol (direct)");
			println!("   Alice paired with Bob (direct)");
			println!("   Alice auto-vouched Bob to Carol");
			println!("   Carol received and accepted Bob's vouch");
			println!("   Bob received ProxyPairingComplete");
			println!("   Bob and Carol are now paired via proxy vouching");
		}
		Err(e) => {
			println!("\nPROXY PAIRING TEST FAILED: {}", e);
			println!("\nThis means the vouching protocol did not complete successfully.");
			println!("Check the logs above for where the protocol stopped.");
			for (name, output) in runner.get_all_outputs() {
				println!("\n=== {} OUTPUT ===\n{}", name.to_uppercase(), output);
			}
			panic!("Proxy pairing test failed - vouching protocol did not work");
		}
	}
}
