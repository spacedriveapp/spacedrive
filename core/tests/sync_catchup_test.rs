//! Catch-up runs once per peer.
//!
//! Alice is the existing member: her copy holds a volume and starts Ready.
//! Bob's copy was just created from the shared library id, so it carries the
//! first-backfill marker, starts Uninitialized, and backfills from Alice.
//! Alice receives no device-owned rows from Bob, so she has no per-resource
//! watermark for him; before the per-peer catch-up record existed that made
//! Bob a new peer on every loop iteration and Alice caught up with him
//! forever (SPAC-12 bug 2).

mod helpers;

use helpers::{
	create_snapshot_dir, create_test_volume, init_test_tracing, register_device, wait_until,
	MockTransport, TestConfigBuilder, TestDataDir,
};
use sd_core::{
	infra::{
		db::entities,
		sync::{ChangeType, NetworkTransport, SharedChangeEntry, SystemTimeSource, HLC},
	},
	service::{sync::state::DeviceSyncState, Service},
	Core,
};
use sea_orm::{ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter};
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

	// Alice's copy is the original. create_library_with_id marks a copy as
	// waiting for its first backfill; clearing that is what a library created
	// on this device, or one that already backfilled, looks like on disk.
	{
		let sync_db = sea_orm::Database::connect(format!(
			"sqlite://{}?mode=rwc",
			library_alice.path().join("sync.db").display()
		))
		.await?;
		sd_core::infra::sync::SyncStateStore::mark_initial_backfill_complete(&sync_db).await?;
	}

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
	assert!(
		sync_alice.peer_sync().state().await.is_ready(),
		"the existing member must start Ready, not {:?}",
		sync_alice.peer_sync().state().await
	);
	assert!(
		!sync_bob.peer_sync().state().await.is_ready(),
		"the joiner must start Uninitialized, not {:?}",
		sync_bob.peer_sync().state().await
	);
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

	let sessions = |sync: &sd_core::service::sync::SyncService| {
		sync.metrics()
			.metrics()
			.operations
			.backfill_sessions_completed
			.load(Ordering::Relaxed)
	};
	// Bob's backfill counts itself after he turns Ready, and Alice's one
	// catch-up with Bob runs on the next sync loop tick (5 s by default);
	// wait for both instead of guessing when the tick lands.
	wait_until(
		"one session on each side",
		Duration::from_secs(20),
		|| async { (sessions(sync_alice) >= 1 && sessions(sync_bob) >= 1).then_some(()) },
	)
	.await?;
	let alice_before = sessions(sync_alice);
	let bob_before = sessions(sync_bob);
	assert_eq!(
		alice_before, 1,
		"Alice ran {} sessions before settling; expected exactly one catch-up with Bob",
		alice_before
	);
	assert_eq!(
		bob_before, 1,
		"Bob ran {} sessions before settling; expected exactly his backfill",
		bob_before
	);

	// A fixed window is the only way to observe that nothing happens: three
	// loop ticks with no new session on either side.
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

/// Changes received during an incremental catch-up are buffered, and the
/// catch-up then returns to Ready without going through
/// `transition_to_ready`, which is the only other place the buffer is
/// drained. Ending the catch-up must apply them, or they are lost.
#[tokio::test]
async fn test_changes_buffered_during_catch_up_are_applied() -> anyhow::Result<()> {
	let test_data = TestDataDir::new("catch_up_buffer")?;
	let dir = test_data.core_data_path();
	TestConfigBuilder::new(dir.clone()).build()?;

	let core = Core::new(dir)
		.await
		.map_err(|e| anyhow::anyhow!("Failed to create core: {}", e))?;
	let device_id = core.device.device_id()?;
	let library = core
		.libraries
		.create_library_no_sync("Catch Up Buffer", None, core.context.clone())
		.await?;
	library
		.init_sync_service(
			device_id,
			MockTransport::new_single(device_id) as Arc<dyn NetworkTransport>,
		)
		.await?;
	let peer_sync = library.sync_service().unwrap().peer_sync();

	let peer = Uuid::new_v4();
	peer_sync
		.set_state_for_test(DeviceSyncState::CatchingUp { buffered_count: 0 })
		.await;

	let space_uuid = Uuid::new_v4();
	let now = chrono::Utc::now();
	peer_sync
		.on_shared_change_received(SharedChangeEntry {
			hlc: HLC::now(peer, &SystemTimeSource),
			model_type: "space".to_string(),
			record_uuid: space_uuid,
			change_type: ChangeType::Insert,
			data: serde_json::json!({
				"uuid": space_uuid,
				"name": "From peer",
				"icon": "Planet",
				"color": "#000000",
				"order": 0,
				"created_at": now,
				"updated_at": now,
			}),
		})
		.await?;

	let space_count = || async {
		entities::space::Entity::find()
			.filter(entities::space::Column::Uuid.eq(space_uuid))
			.count(library.db().conn())
			.await
	};
	assert_eq!(
		space_count().await?,
		0,
		"the change must be buffered while catching up"
	);

	peer_sync.finish_catch_up(peer, "catchup_completed").await;

	assert!(peer_sync.state().await.is_ready());
	assert_eq!(
		space_count().await?,
		1,
		"the change buffered during catch-up was never applied"
	);

	Ok(())
}
