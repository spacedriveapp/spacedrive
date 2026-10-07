//! Peer rows of the R8 source runtime acceptance matrix
//! (`docs/core/acceptance/source-runtime.md`): two daemons in separate
//! processes, paired over loopback, replicating a source.
//!
//! Alice owns a source. Bob replicates it, then asks ten more times while
//! Alice is unchanged and must transfer nothing: same generation, same
//! loaded arena, same sync time. Alice then writes real files and re-walks,
//! and Bob's next passes must land the newer generation with every committed
//! record present.

use sd_core::{
	ops::sources::track::action::track_and_index, service::mounts::peer, testing::CargoTestRunner,
	Core,
};
use std::{env, path::PathBuf, time::Duration};
use tokio::time::timeout;

const TEST_DIR: &str = "/tmp/spacedrive-source-replication-test";

fn marker(name: &str) -> PathBuf {
	PathBuf::from(TEST_DIR).join(name)
}

async fn wait_for_marker(name: &str, limit: Duration) {
	let started = std::time::Instant::now();
	while !marker(name).exists() {
		assert!(
			started.elapsed() < limit,
			"timed out waiting for marker {name}"
		);
		tokio::time::sleep(Duration::from_millis(250)).await;
	}
}

/// Files Alice's source holds at the start.
const INITIAL: [&str; 5] = [
	"first.txt",
	"second.txt",
	"third.txt",
	"notes/fourth.md",
	"notes/fifth.md",
];
/// Files Alice adds after Bob has converged.
const LATER: [&str; 3] = ["later-one.txt", "later-two.txt", "notes/later-three.md"];

/// Alice: owns the source, pairs as initiator, then writes after Bob converges.
#[tokio::test]
#[ignore]
async fn alice_replication_scenario() {
	if env::var("TEST_ROLE").unwrap_or_default() != "alice" {
		return;
	}
	env::set_var("SPACEDRIVE_TEST_DIR", TEST_DIR);
	let data_dir = PathBuf::from(TEST_DIR).join("alice");
	let source_root = PathBuf::from(TEST_DIR).join("alice-source");
	std::fs::create_dir_all(source_root.join("notes")).unwrap();
	for name in INITIAL {
		std::fs::write(source_root.join(name), name).unwrap();
	}

	println!("Alice: starting core");
	let mut core = timeout(Duration::from_secs(10), Core::new(data_dir))
		.await
		.unwrap()
		.unwrap();
	core.device
		.set_name("Alice's Replication Device".to_string())
		.unwrap();
	let library = core
		.libraries
		.create_library("Alice Replication Library", None, core.context.clone())
		.await
		.unwrap();

	println!("Alice: tracking the source");
	let tracked = track_and_index(&library, &core.context, source_root.clone(), false)
		.await
		.unwrap();
	wait_for_walk(&library, tracked.job_id).await;
	std::fs::write(marker("alice_source_id.txt"), tracked.id.to_string()).unwrap();

	timeout(Duration::from_secs(10), core.init_networking())
		.await
		.unwrap()
		.unwrap();
	tokio::time::sleep(Duration::from_secs(3)).await;

	let networking = core.networking().expect("networking");
	let (pairing_code, _) = timeout(
		Duration::from_secs(15),
		networking.start_pairing_as_initiator(false),
	)
	.await
	.unwrap()
	.unwrap();
	std::fs::write(marker("pairing_code.txt"), &pairing_code).unwrap();
	println!("Alice: pairing code written");

	let started = std::time::Instant::now();
	loop {
		tokio::time::sleep(Duration::from_secs(1)).await;
		let connected = core.services.device.get_connected_devices().await.unwrap();
		if !connected.is_empty() {
			println!("Alice: Bob connected");
			break;
		}
		assert!(
			started.elapsed() < Duration::from_secs(60),
			"Alice: Bob never connected"
		);
	}

	// Bob says he has converged and watched ten unchanged passes.
	wait_for_marker("bob_converged.txt", Duration::from_secs(120)).await;
	println!("Alice: writing new files and re-walking");
	for name in LATER {
		std::fs::write(source_root.join(name), name).unwrap();
	}
	let rewalk = track_and_index(&library, &core.context, source_root.clone(), false)
		.await
		.unwrap();
	wait_for_walk(&library, rewalk.job_id).await;
	std::fs::write(marker("alice_wrote.txt"), "wrote").unwrap();

	wait_for_marker("bob_success.txt", Duration::from_secs(120)).await;
	std::fs::write(marker("alice_success.txt"), "success").unwrap();
	println!("Alice: done");
}

async fn wait_for_walk(
	library: &std::sync::Arc<sd_core::library::Library>,
	job: Option<uuid::Uuid>,
) {
	use sd_core::infra::job::types::JobId;
	let Some(job) = job else { return };
	if let Some(walk) = library.jobs().get_job(JobId(job)).await {
		walk.wait().await.unwrap();
	}
}

/// Bob: pairs as joiner and replicates Alice's source.
#[tokio::test]
#[ignore]
async fn bob_replication_scenario() {
	if env::var("TEST_ROLE").unwrap_or_default() != "bob" {
		return;
	}
	env::set_var("SPACEDRIVE_TEST_DIR", TEST_DIR);
	let data_dir = PathBuf::from(TEST_DIR).join("bob");

	println!("Bob: starting core");
	let mut core = timeout(Duration::from_secs(10), Core::new(data_dir))
		.await
		.unwrap()
		.unwrap();
	core.device
		.set_name("Bob's Replication Device".to_string())
		.unwrap();
	let _library = core
		.libraries
		.create_library("Bob Replication Library", None, core.context.clone())
		.await
		.unwrap();
	timeout(Duration::from_secs(10), core.init_networking())
		.await
		.unwrap()
		.unwrap();
	tokio::time::sleep(Duration::from_secs(3)).await;

	let pairing_code = loop {
		if let Ok(code) = std::fs::read_to_string(marker("pairing_code.txt")) {
			break code.trim().to_string();
		}
		tokio::time::sleep(Duration::from_millis(500)).await;
	};
	let networking = core.networking().expect("networking");
	timeout(
		Duration::from_secs(15),
		networking.start_pairing_as_joiner(&pairing_code, false),
	)
	.await
	.unwrap()
	.unwrap();
	println!("Bob: joined pairing");

	let started = std::time::Instant::now();
	let alice = loop {
		tokio::time::sleep(Duration::from_secs(1)).await;
		let connected = core.services.device.get_connected_devices().await.unwrap();
		if let Some(device) = connected.first() {
			println!("Bob: connected to Alice {device}");
			break *device;
		}
		assert!(
			started.elapsed() < Duration::from_secs(60),
			"Bob: Alice never connected"
		);
	};

	let source_id: uuid::Uuid = std::fs::read_to_string(marker("alice_source_id.txt"))
		.unwrap()
		.trim()
		.parse()
		.unwrap();
	let source_root = PathBuf::from(TEST_DIR).join("alice-source");

	// First pass: the replica arrives with every initial record.
	let share = {
		let started = std::time::Instant::now();
		loop {
			match peer::sync_device(&core.context, alice, "alice".to_string()).await {
				Ok(count) => println!("Bob: sync pass replicated {count} source(s)"),
				Err(err) => println!("Bob: sync pass failed: {err}"),
			}
			if let Some(share) = peer::remote_share(source_id).await {
				let index = share.index.read().await;
				if INITIAL
					.iter()
					.all(|name| index.has_entry(&source_root.join(name)))
				{
					drop(index);
					break share;
				}
			}
			assert!(
				started.elapsed() < Duration::from_secs(60),
				"Bob: the replica never arrived with every initial record"
			);
			tokio::time::sleep(Duration::from_secs(1)).await;
		}
	};
	println!(
		"Bob: converged at generation {} (synced_at {})",
		share.generation, share.synced_at_secs
	);
	assert_ne!(share.generation, 0, "the owner describes its artifact");

	// R8 "Unchanged owner for ten intervals": ten more passes move nothing.
	for pass in 1..=10 {
		peer::sync_device(&core.context, alice, "alice".to_string())
			.await
			.expect("sync pass");
		let again = peer::remote_share(source_id).await.expect("share stays");
		assert_eq!(
			again.generation, share.generation,
			"pass {pass}: the generation moved with no writes on the owner"
		);
		assert_eq!(
			again.synced_at_secs, share.synced_at_secs,
			"pass {pass}: a transfer happened with no writes on the owner"
		);
		assert!(
			std::sync::Arc::ptr_eq(&again.index, &share.index),
			"pass {pass}: the arena was reconstructed with no writes on the owner"
		);
		tokio::time::sleep(Duration::from_millis(500)).await;
	}
	println!("Bob: ten unchanged passes transferred nothing");
	std::fs::write(marker("bob_converged.txt"), "converged").unwrap();

	// R8 "Continuous real writes": the owner's new records reach the
	// replica at a newer generation, with nothing committed lost.
	wait_for_marker("alice_wrote.txt", Duration::from_secs(120)).await;
	let started = std::time::Instant::now();
	loop {
		if let Err(err) = peer::sync_device(&core.context, alice, "alice".to_string()).await {
			println!("Bob: sync pass failed: {err}");
		}
		let newer = peer::remote_share(source_id).await.expect("share stays");
		let index = newer.index.read().await;
		let complete = INITIAL
			.iter()
			.chain(LATER.iter())
			.all(|name| index.has_entry(&source_root.join(name)));
		if complete {
			assert_ne!(
				newer.generation, share.generation,
				"new records arrived under the old generation"
			);
			println!(
				"Bob: newest generation {} holds every record",
				newer.generation
			);
			break;
		}
		assert!(
			started.elapsed() < Duration::from_secs(60),
			"Bob: the owner's writes never reached the replica"
		);
		drop(index);
		tokio::time::sleep(Duration::from_secs(1)).await;
	}

	std::fs::write(marker("bob_success.txt"), "success").unwrap();
	println!("Bob: done");
}

/// Orchestrator: spawns Alice and Bob and waits for both markers.
#[tokio::test]
async fn test_source_replication() {
	let _ = std::fs::remove_dir_all(TEST_DIR);
	std::fs::create_dir_all(TEST_DIR).unwrap();

	let mut runner = CargoTestRunner::for_test_file("source_replication_test")
		.with_timeout(Duration::from_secs(300))
		.add_subprocess("alice", "alice_replication_scenario")
		.add_subprocess("bob", "bob_replication_scenario");

	runner
		.spawn_single_process("alice")
		.await
		.expect("Failed to spawn Alice");
	tokio::time::sleep(Duration::from_secs(8)).await;
	runner
		.spawn_single_process("bob")
		.await
		.expect("Failed to spawn Bob");

	let result = runner
		.wait_for_success(|_| {
			marker("alice_success.txt").exists() && marker("bob_success.txt").exists()
		})
		.await;

	if let Err(e) = result {
		println!("Source replication test failed: {e}");
		for (name, output) in runner.get_all_outputs() {
			println!("\n{name} output:\n{output}");
		}
		panic!("source replication test failed");
	}
}
