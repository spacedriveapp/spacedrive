//! Relay-only pairing test - verifies pairing works exclusively through an Iroh relay
//!
//! The orchestrator runs an iroh relay server in-process and hands its URL
//! to both roles through `SD_RELAY_URL`, so the suite never touches the
//! public n0 relays and CI needs no internet for it. `SD_DISABLE_MDNS`
//! keeps the two cores from finding each other's direct addresses on the
//! host, so the joiner's only way to the initiator is the relay: the pairing
//! code carries the node id, and the core attaches the configured relay to
//! it when it dials. After pairing, the relay's own counters prove the
//! handshake went through it.

use iroh_relay::server::{AccessConfig, RelayConfig, Server, ServerConfig};
use sd_core::service::network::core::{DISABLE_MDNS_ENV, RELAY_URL_ENV};
use sd_core::service::network::protocol::pairing::PairingCode;
use sd_core::testing::CargoTestRunner;
use sd_core::Core;
use std::env;
use std::net::{Ipv4Addr, SocketAddr};
use std::path::PathBuf;
use std::time::Duration;
use tokio::time::timeout;

#[path = "helpers/wait.rs"]
mod wait;
use wait::{wait_for_file, wait_for_paired_device};

const TEST_DIR: &str = "/tmp/spacedrive-relay-only-test";

fn marker(name: &str) -> String {
	format!("{TEST_DIR}/{name}")
}

/// Alice's relay-only pairing scenario - initiator
#[tokio::test]
#[ignore] // Only run when explicitly called via subprocess
async fn alice_relay_only_pairing() {
	if env::var("TEST_ROLE").unwrap_or_default() != "alice" {
		return;
	}
	assert!(
		env::var_os(RELAY_URL_ENV).is_some(),
		"Alice: {RELAY_URL_ENV} must name the test relay"
	);

	let data_dir = PathBuf::from(marker("alice"));
	let device_name = "Alice's Relay Test Device";

	println!("Alice: Starting RELAY-ONLY pairing test");
	println!("Alice: Data dir: {:?}", data_dir);

	println!("Alice: Initializing Core...");
	let mut core = timeout(Duration::from_secs(10), Core::new(data_dir))
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
	println!("Alice: Networking initialized successfully");

	// The initiator awaits the endpoint coming online, which needs the
	// relay connection, before it returns the code.
	println!("Alice: Starting pairing as initiator (FORCE RELAY MODE)...");
	let networking = core.networking().expect("Networking not initialized");
	let (pairing_code, expires_in) = timeout(
		Duration::from_secs(30),
		networking.start_pairing_as_initiator(true),
	)
	.await
	.unwrap()
	.unwrap();
	let short_code = pairing_code
		.split_whitespace()
		.take(3)
		.collect::<Vec<_>>()
		.join(" ");
	println!(
		"Alice: Pairing code generated: {}... (expires in {}s)",
		short_code, expires_in
	);
	println!("Alice: home relay: {:?}", networking.get_relay_url().await);

	// The QR JSON carries the node id the joiner needs to dial through the
	// relay; the BIP39 words alone are for mDNS pairing.
	let pairing_code_obj = networking
		.get_pairing_code_for_current_session()
		.await
		.unwrap()
		.expect("Pairing code should exist");
	let qr_json = pairing_code_obj.to_qr_json();
	println!(
		"Alice: Generated QR JSON (session {}, node {:?})",
		pairing_code_obj.session_id(),
		pairing_code_obj
			.node_id()
			.map(|id| id.fmt_short().to_string())
	);
	std::fs::write(marker("pairing_qr.json"), &qr_json).unwrap();

	println!("Alice: Waiting for relay connection from Bob...");
	let bob = wait_for_paired_device(&core, Duration::from_secs(60))
		.await
		.expect("Alice: Timeout waiting for relay pairing to complete");
	println!("Alice: Pairing completed via RELAY with {bob}");
	std::fs::write(marker("alice_success.txt"), "success").unwrap();

	// Stay up until Bob has stored the pairing too, so Alice's exit cannot
	// cut the exchange on his side.
	wait_for_file(marker("bob_success.txt"), Duration::from_secs(60))
		.await
		.expect("Alice: Bob never reported the pairing");
	println!("Alice: Test complete, exiting");
}

/// Bob's relay-only pairing scenario - joiner
#[tokio::test]
#[ignore] // Only run when explicitly called via subprocess
async fn bob_relay_only_pairing() {
	if env::var("TEST_ROLE").unwrap_or_default() != "bob" {
		return;
	}
	assert!(
		env::var_os(RELAY_URL_ENV).is_some(),
		"Bob: {RELAY_URL_ENV} must name the test relay"
	);

	let data_dir = PathBuf::from(marker("bob"));
	let device_name = "Bob's Relay Test Device";

	println!("Bob: Starting RELAY-ONLY pairing test");
	println!("Bob: Data dir: {:?}", data_dir);

	println!("Bob: Initializing Core...");
	let mut core = timeout(Duration::from_secs(10), Core::new(data_dir))
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
	println!("Bob: Networking initialized successfully");

	println!("Bob: Looking for QR code JSON...");
	let qr_json = wait_for_file(marker("pairing_qr.json"), Duration::from_secs(60))
		.await
		.expect("Bob: Alice never wrote the QR JSON");
	println!("Bob: QR JSON content: {}", qr_json);

	let pairing_code = PairingCode::from_qr_json(&qr_json).unwrap();
	println!("Bob: Parsed session_id: {}", pairing_code.session_id());
	let target = pairing_code
		.node_id()
		.expect("QR JSON must carry the initiator's node id");
	println!("Bob: Target NodeId: {}", target.fmt_short());

	println!("Bob: Joining pairing session (FORCE RELAY MODE)...");
	let networking = core.networking().expect("Networking not initialized");
	timeout(
		Duration::from_secs(45),
		networking.start_pairing_as_joiner_with_code(pairing_code, true),
	)
	.await
	.unwrap()
	.unwrap();
	println!("Bob: Connected to Alice via RELAY");

	println!("Bob: Waiting for pairing to complete...");
	let alice = wait_for_paired_device(&core, Duration::from_secs(30))
		.await
		.expect("Bob: Timeout waiting for pairing to complete");
	println!("Bob: Pairing completed successfully with {alice}");
	std::fs::write(marker("bob_success.txt"), "success").unwrap();

	wait_for_file(marker("alice_success.txt"), Duration::from_secs(60))
		.await
		.expect("Bob: Alice never reported the pairing");
	println!("Bob: Test complete, exiting");
}

/// Main test orchestrator - runs a relay and Alice and Bob in separate subprocesses
#[tokio::test]
async fn test_relay_only_pairing() {
	let _ = std::fs::remove_dir_all(TEST_DIR);
	std::fs::create_dir_all(TEST_DIR).unwrap();

	println!("Starting RELAY-ONLY pairing integration test");

	let relay = Server::spawn(ServerConfig::<(), ()> {
		relay: Some(RelayConfig {
			http_bind_addr: SocketAddr::from((Ipv4Addr::LOCALHOST, 0)),
			tls: None,
			limits: Default::default(),
			key_cache_capacity: Some(1024),
			access: AccessConfig::Everyone,
		}),
		quic: None,
		metrics_addr: None,
	})
	.await
	.expect("relay server");
	let relay_url = format!("http://{}", relay.http_addr().expect("relay bound"));
	println!("Local relay listening at {relay_url}");

	// The roles inherit the environment: both cores use only this relay and
	// no mDNS, so direct addresses are unknown until the relay path is up.
	env::set_var(RELAY_URL_ENV, &relay_url);
	env::set_var(DISABLE_MDNS_ENV, "1");

	let mut runner = CargoTestRunner::for_test_file("relay_only_pairing_test")
		.with_timeout(Duration::from_secs(150))
		.add_subprocess("alice", "alice_relay_only_pairing")
		.add_subprocess("bob", "bob_relay_only_pairing");

	runner.spawn_single_process("alice").await.unwrap();
	// Bob waits for Alice's QR JSON himself.
	runner.spawn_single_process("bob").await.unwrap();

	let result = runner
		.wait_for_success(|_outputs| {
			["alice", "bob"].iter().all(|name| {
				std::fs::read_to_string(marker(&format!("{name}_success.txt")))
					.map(|content| content.trim() == "success")
					.unwrap_or(false)
			})
		})
		.await;
	if let Err(e) = result {
		for (name, output) in runner.get_all_outputs() {
			println!("\n{} output:\n{}", name, output);
		}
		panic!("Relay-only pairing test failed: {e}");
	}

	let metrics = relay.metrics().server.clone();
	println!(
		"Relay counters: accepts={} unique_clients={} packets_relayed={} bytes_relayed={}",
		metrics.accepts.get(),
		metrics.unique_client_keys.get(),
		metrics.send_packets_recv.get(),
		metrics.bytes_recv.get()
	);
	assert!(
		metrics.accepts.get() >= 2,
		"both cores should have connected to the local relay"
	);
	assert!(
		metrics.send_packets_recv.get() > 0,
		"the pairing handshake should have travelled through the local relay"
	);
	relay.shutdown().await.expect("relay shutdown");

	println!("\nRELAY-ONLY PAIRING TEST PASSED");
	println!("Both devices paired using only the local Iroh relay");
}
