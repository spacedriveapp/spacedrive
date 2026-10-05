//! Catch-up runs once per peer.
//!
//! Alice holds a volume, Bob holds nothing of his own. Both copies were
//! created from the shared library id, so both backfill from each other once
//! and reach Ready. Alice receives no device-owned rows from Bob, so she has
//! no per-resource watermark for him; before the per-peer catch-up record
//! existed that made Bob look like a new peer on every loop iteration and
//! Alice caught up with him forever (SPAC-12 bug 2).

mod helpers;

use helpers::{
	create_snapshot_dir, create_test_volume, init_test_tracing, register_device, MockTransport,
	TestConfigBuilder, TestDataDir,
};
use sd_core::{
	infra::{db::entities, sync::NetworkTransport},
	service::Service,
	Core,
};
use sea_orm::{EntityTrait, PaginatorTrait};
use std::sync::atomic::Ordering;
use std::sync::Arc;
use tokio::time::Duration;
use uuid::Uuid;

#[tokio::test]
async fn test_catch_up_with_empty_peer_runs_once() -> anyhow::Result<()> {
	let snapshot_dir = create_snapshot_dir("catch_up_runs_once").await?;
	init_test_tracing("catch_up_runs_once", &snapshot_dir)?;

	let test_data_alice = TestDataDir::new("catch_up_alice")?;
	let test_data_bob = TestDataDir::new("catch_up_bob")?;
	let dir_alice = test_data_alice.core_data_path();
	let dir_bob = test_data_bob.core_data_path();
	TestConfigBuilder::new(dir_alice.clone()).build()?;
	TestConfigBuilder::new(dir_bob.clone()).build()?;

	let library_id = Uuid::new_v4();

	let core_alice = Core::new(dir_alice.clone())
		.await
		.map_err(|e| anyhow::anyhow!("Failed to create Alice core: {}", e))?;
	core_alice.device.set_name("Alice".to_string())?;
	let device_alice_id = core_alice.device.device_id()?;
	let library_alice = core_alice
		.libraries
		.create_library_with_id(library_id, "Catch Up", None, core_alice.context.clone())
		.await?;

	let core_bob = Core::new(dir_bob.clone())
		.await
		.map_err(|e| anyhow::anyhow!("Failed to create Bob core: {}", e))?;
	core_bob.device.set_name("Bob".to_string())?;
	let device_bob_id = core_bob.device.device_id()?;
	let library_bob = core_bob
		.libraries
		.create_library_with_id(library_id, "Catch Up", None, core_bob.context.clone())
		.await?;

	register_device(&library_alice, device_bob_id, "Bob").await?;
	register_device(&library_bob, device_alice_id, "Alice").await?;

	create_test_volume(&library_alice, device_alice_id, "alice-fp", "Alice HD").await?;

	let (transport_alice, transport_bob) = MockTransport::new_pair(device_alice_id, device_bob_id);
	library_alice
		.init_sync_service(
			device_alice_id,
			transport_alice.clone() as Arc<dyn NetworkTransport>,
		)
		.await?;
	library_bob
		.init_sync_service(
			device_bob_id,
			transport_bob.clone() as Arc<dyn NetworkTransport>,
		)
		.await?;
	transport_alice
		.register_sync_service(
			device_alice_id,
			Arc::downgrade(library_alice.sync_service().unwrap()),
		)
		.await;
	transport_bob
		.register_sync_service(
			device_bob_id,
			Arc::downgrade(library_bob.sync_service().unwrap()),
		)
		.await;

	let sync_alice = library_alice.sync_service().unwrap();
	let sync_bob = library_bob.sync_service().unwrap();
	sync_alice.start().await?;
	sync_bob.start().await?;

	// Wait until Bob has Alice's volume and both sides are Ready.
	let start = tokio::time::Instant::now();
	loop {
		let bob_volumes = entities::volume::Entity::find()
			.count(library_bob.db().conn())
			.await?;
		if bob_volumes == 1
			&& sync_alice.peer_sync().state().await.is_ready()
			&& sync_bob.peer_sync().state().await.is_ready()
		{
			break;
		}
		if start.elapsed() > Duration::from_secs(30) {
			anyhow::bail!(
				"backfill did not settle: bob_volumes={}, alice={:?}, bob={:?}",
				bob_volumes,
				sync_alice.peer_sync().state().await,
				sync_bob.peer_sync().state().await
			);
		}
		tokio::time::sleep(Duration::from_millis(200)).await;
	}

	// Let the loop run a few iterations past the first full backfill. The
	// default sync loop interval is 5 s; one catch-up with Bob right after
	// Alice's own backfill is the ceiling.
	tokio::time::sleep(Duration::from_secs(8)).await;
	let sessions = |sync: &sd_core::service::sync::SyncService| {
		sync.metrics()
			.metrics()
			.operations
			.backfill_sessions_completed
			.load(Ordering::Relaxed)
	};
	let alice_before = sessions(sync_alice);
	let bob_before = sessions(sync_bob);
	assert!(
		alice_before <= 2,
		"Alice ran {} sessions before settling; expected her backfill and at most one catch-up",
		alice_before
	);

	tokio::time::sleep(Duration::from_secs(16)).await;
	let alice_after = sessions(sync_alice);
	let bob_after = sessions(sync_bob);

	assert_eq!(
		alice_before, alice_after,
		"Alice kept catching up with Bob without new events ({} -> {})",
		alice_before, alice_after
	);
	assert_eq!(
		bob_before, bob_after,
		"Bob kept catching up with Alice without new events ({} -> {})",
		bob_before, bob_after
	);
	assert!(
		sync_alice
			.peer_sync()
			.peer_caught_up_at(device_bob_id)
			.await?
			.is_some(),
		"Alice should have recorded her catch-up with Bob"
	);

	// Alice's own volume row is untouched: still one, still hers.
	let alice_volumes = entities::volume::Entity::find()
		.all(library_alice.db().conn())
		.await?;
	assert_eq!(alice_volumes.len(), 1);
	assert_eq!(alice_volumes[0].device_id, device_alice_id);

	Ok(())
}
