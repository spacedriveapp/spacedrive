//! Join an existing library from a second device over the real network stack.
//!
//! Alice owns a library with data. Bob pairs with Alice and runs
//! `library sync-setup --action join-remote`. This is the flow SPAC-12 ran
//! between Jamie's MacBook and the Mac Studio, and it checks the four things
//! that went wrong there: Bob backfills from Alice (not the other way round),
//! Alice's rows are untouched, Bob's copy of the library holds Alice's device
//! row, and once both sides are caught up no further catch-up runs.

use sd_core::infra::db::entities;
use sd_core::testing::CargoTestRunner;
use sd_core::Core;
use sea_orm::{ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter};
use std::env;
use std::path::{Path, PathBuf};
use std::time::Duration;
use tokio::time::timeout;

const TEST_DIR: &str = "/tmp/spacedrive-library-join-test";

fn marker(name: &str) -> PathBuf {
	Path::new(TEST_DIR).join(name)
}

fn write_marker(name: &str, content: &str) {
	std::fs::write(marker(name), content).unwrap();
}

fn read_marker(name: &str) -> Option<String> {
	std::fs::read_to_string(marker(name))
		.ok()
		.map(|s| s.trim().to_string())
}

async fn wait_for_marker(name: &str, max: Duration) -> String {
	let start = tokio::time::Instant::now();
	loop {
		if let Some(value) = read_marker(name) {
			return value;
		}
		if start.elapsed() > max {
			panic!("timed out waiting for marker {}", name);
		}
		tokio::time::sleep(Duration::from_millis(500)).await;
	}
}

fn fail(role: &str, message: String) -> ! {
	write_marker(&format!("{}_error.txt", role), &message);
	panic!("{}: {}", role, message);
}

/// Alice: owns the library, shares nothing explicitly, and must stay the
/// source of truth while Bob joins.
#[tokio::test]
#[ignore]
async fn alice_join_scenario() {
	if env::var("TEST_ROLE").unwrap_or_default() != "alice" {
		return;
	}
	env::set_var("SPACEDRIVE_TEST_DIR", TEST_DIR);

	let mut core = timeout(
		Duration::from_secs(10),
		Core::new(PathBuf::from(TEST_DIR).join("alice")),
	)
	.await
	.unwrap()
	.unwrap();
	core.device.set_name("Alice Device".to_string()).unwrap();
	timeout(Duration::from_secs(10), core.init_networking())
		.await
		.unwrap()
		.unwrap();
	tokio::time::sleep(Duration::from_secs(2)).await;

	let library = core
		.libraries
		.create_library("Join Test Library".to_string(), None, core.context.clone())
		.await
		.unwrap();
	let db = library.db().conn();

	// Data that only Alice has: a second space next to the default one.
	{
		use chrono::Utc;
		use sea_orm::{ActiveModelTrait, Set};
		entities::space::ActiveModel {
			id: sea_orm::ActiveValue::NotSet,
			uuid: Set(uuid::Uuid::new_v4()),
			name: Set("Alice Projects".to_string()),
			icon: Set("Folder".to_string()),
			color: Set("#FF0000".to_string()),
			order: Set(1),
			created_at: Set(Utc::now().into()),
			updated_at: Set(Utc::now().into()),
		}
		.insert(db)
		.await
		.unwrap();
	}
	let spaces_before = entities::space::Entity::find().all(db).await.unwrap();
	assert_eq!(spaces_before.len(), 2);
	let default_space_before = spaces_before
		.iter()
		.find(|s| s.name == "Default")
		.cloned()
		.expect("default space");

	write_marker("library_id.txt", &library.id().to_string());
	write_marker(
		"alice_device_id.txt",
		&core.device.device_id().unwrap().to_string(),
	);

	let networking = core.networking().expect("networking");
	let (pairing_code, _) = timeout(
		Duration::from_secs(15),
		networking.start_pairing_as_initiator(false),
	)
	.await
	.unwrap()
	.unwrap();
	write_marker("pairing_code.txt", &pairing_code);

	// Wait for Bob to join the library: his device row lands in our table.
	let bob_device_id = {
		let start = tokio::time::Instant::now();
		loop {
			if start.elapsed() > Duration::from_secs(90) {
				fail("alice", "Bob never registered in Alice's library".into());
			}
			let devices = entities::device::Entity::find().all(db).await.unwrap();
			if let Some(bob) = devices
				.iter()
				.find(|d| d.uuid != core.device.device_id().unwrap())
			{
				break bob.uuid;
			}
			tokio::time::sleep(Duration::from_millis(500)).await;
		}
	};
	println!("Alice: Bob {} joined the library", bob_device_id);
	write_marker("alice_saw_bob.txt", &bob_device_id.to_string());

	// Give sync time to settle on both sides, then check nothing moved.
	wait_for_marker("bob_success.txt", Duration::from_secs(120)).await;

	let sync = library.sync_service().expect("sync service");
	let sessions_a = sync
		.metrics()
		.metrics()
		.operations
		.backfill_sessions_completed
		.load(std::sync::atomic::Ordering::Relaxed);
	tokio::time::sleep(Duration::from_secs(15)).await;
	let sessions_b = sync
		.metrics()
		.metrics()
		.operations
		.backfill_sessions_completed
		.load(std::sync::atomic::Ordering::Relaxed);

	let spaces_after = entities::space::Entity::find().all(db).await.unwrap();
	let default_space_after = spaces_after
		.iter()
		.find(|s| s.name == "Default")
		.cloned()
		.expect("default space");

	let mut problems = Vec::new();
	if spaces_after.len() != 2 {
		problems.push(format!(
			"Alice's space count changed from 2 to {}",
			spaces_after.len()
		));
	}
	if default_space_after.updated_at != default_space_before.updated_at {
		problems.push(format!(
			"Alice's Default space was overwritten by sync (updated_at {} -> {})",
			default_space_before.updated_at, default_space_after.updated_at
		));
	}
	if sessions_b != sessions_a {
		problems.push(format!(
			"Alice kept catching up without new events ({} -> {} sessions in 15s)",
			sessions_a, sessions_b
		));
	}
	if sessions_a > 1 {
		problems.push(format!(
			"Alice ran {} backfill/catch-up sessions against an empty joiner",
			sessions_a
		));
	}
	if !problems.is_empty() {
		fail("alice", problems.join("; "));
	}

	write_marker("alice_success.txt", "success");
	println!("Alice: Test completed");
}

/// Bob: pairs, joins Alice's library, and must end up with her state.
#[tokio::test]
#[ignore]
async fn bob_join_scenario() {
	if env::var("TEST_ROLE").unwrap_or_default() != "bob" {
		return;
	}
	env::set_var("SPACEDRIVE_TEST_DIR", TEST_DIR);

	let mut core = timeout(
		Duration::from_secs(10),
		Core::new(PathBuf::from(TEST_DIR).join("bob")),
	)
	.await
	.unwrap()
	.unwrap();
	core.device.set_name("Bob Device".to_string()).unwrap();
	timeout(Duration::from_secs(10), core.init_networking())
		.await
		.unwrap()
		.unwrap();
	tokio::time::sleep(Duration::from_secs(2)).await;

	let library_id: uuid::Uuid = wait_for_marker("library_id.txt", Duration::from_secs(30))
		.await
		.parse()
		.unwrap();
	let alice_device_id: uuid::Uuid =
		wait_for_marker("alice_device_id.txt", Duration::from_secs(30))
			.await
			.parse()
			.unwrap();
	let pairing_code = wait_for_marker("pairing_code.txt", Duration::from_secs(30)).await;

	let networking = core.networking().expect("networking");
	timeout(
		Duration::from_secs(15),
		networking.start_pairing_as_joiner(&pairing_code, false),
	)
	.await
	.unwrap()
	.unwrap();

	let start = tokio::time::Instant::now();
	loop {
		if start.elapsed() > Duration::from_secs(30) {
			fail("bob", "pairing timeout".into());
		}
		let connected = core.services.device.get_connected_devices().await.unwrap();
		if connected.contains(&alice_device_id) {
			break;
		}
		tokio::time::sleep(Duration::from_millis(500)).await;
	}
	println!("Bob: paired with Alice");

	// Bob needs any local library to address the action; mirror the CLI.
	let scratch = core
		.libraries
		.create_library("Bob Scratch".to_string(), None, core.context.clone())
		.await
		.unwrap();

	use sd_core::infra::action::CoreAction;
	use sd_core::ops::network::sync_setup::{
		LibrarySyncAction, LibrarySyncSetupAction, LibrarySyncSetupInput,
	};

	let input = LibrarySyncSetupInput {
		local_device_id: core.device.device_id().unwrap(),
		remote_device_id: alice_device_id,
		local_library_id: scratch.id(),
		remote_library_id: Some(library_id),
		action: LibrarySyncAction::JoinRemoteLibrary {
			remote_library_id: library_id,
			remote_library_name: "Join Test Library".to_string(),
		},
		leader_device_id: alice_device_id,
	};
	let started = tokio::time::Instant::now();
	let output = match LibrarySyncSetupAction::from_input(input)
		.unwrap()
		.execute(core.context.clone())
		.await
	{
		Ok(output) => output,
		Err(e) => fail("bob", format!("join-remote failed: {:?}", e)),
	};
	println!(
		"Bob: join-remote returned in {:?}: {:?}",
		started.elapsed(),
		output
	);
	if !output.success || !output.devices_registered {
		fail(
			"bob",
			format!("join-remote reported failure: {}", output.message),
		);
	}

	let library = match core.libraries.get_library(library_id).await {
		Some(library) => library,
		None => fail("bob", "joined library is not open locally".into()),
	};
	let db = library.db().conn();

	// The join must leave Alice's device row in Bob's copy right away: the
	// action reports devices_registered, so the row has to be there.
	let alice_rows = entities::device::Entity::find()
		.filter(entities::device::Column::Uuid.eq(alice_device_id))
		.count(db)
		.await
		.unwrap();
	if alice_rows != 1 {
		fail(
			"bob",
			"join-remote returned devices_registered but Alice's device row is missing".into(),
		);
	}

	// Backfill brings Alice's spaces across.
	let start = tokio::time::Instant::now();
	loop {
		let names: Vec<String> = entities::space::Entity::find()
			.all(db)
			.await
			.unwrap()
			.into_iter()
			.map(|s| s.name)
			.collect();
		if names.iter().any(|n| n == "Alice Projects") {
			println!("Bob: backfilled Alice's spaces: {:?}", names);
			break;
		}
		if start.elapsed() > Duration::from_secs(60) {
			fail(
				"bob",
				format!("Bob never received Alice's spaces, has {:?}", names),
			);
		}
		tokio::time::sleep(Duration::from_millis(500)).await;
	}

	let sync = library.sync_service().expect("sync service");
	let start = tokio::time::Instant::now();
	loop {
		if sync.peer_sync().state().await.is_ready() {
			break;
		}
		if start.elapsed() > Duration::from_secs(30) {
			fail("bob", "Bob never reached Ready after backfill".into());
		}
		tokio::time::sleep(Duration::from_millis(500)).await;
	}

	write_marker("bob_success.txt", "success");
	wait_for_marker("alice_success.txt", Duration::from_secs(60)).await;
	println!("Bob: Test completed");
}

#[tokio::test]
async fn test_join_remote_library_backfills_joiner() {
	let _ = std::fs::remove_dir_all(TEST_DIR);
	std::fs::create_dir_all(TEST_DIR).unwrap();

	let mut runner = CargoTestRunner::for_test_file("library_join_test")
		.with_timeout(Duration::from_secs(240))
		.add_subprocess("alice", "alice_join_scenario")
		.add_subprocess("bob", "bob_join_scenario");

	runner.spawn_single_process("alice").await.unwrap();
	tokio::time::sleep(Duration::from_secs(8)).await;
	runner.spawn_single_process("bob").await.unwrap();

	let result = runner
		.wait_for_success(|_| {
			for role in ["alice", "bob"] {
				if let Some(error) = read_marker(&format!("{}_error.txt", role)) {
					println!("{} failed: {}", role, error);
					return false;
				}
			}
			read_marker("alice_success.txt").is_some() && read_marker("bob_success.txt").is_some()
		})
		.await;

	if let Err(e) = result {
		for role in ["alice", "bob"] {
			if let Some(error) = read_marker(&format!("{}_error.txt", role)) {
				println!("{} error: {}", role, error);
			}
		}
		for (name, output) in runner.get_all_outputs() {
			println!("\n{} output:\n{}", name, output);
		}
		panic!("library join test failed: {}", e);
	}
}
