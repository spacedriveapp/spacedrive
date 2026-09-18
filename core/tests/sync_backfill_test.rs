//! Initial Backfill Sync Test
//!
//! Tests backfill between two devices that connect after each already holds
//! state, using volumes as the synced model.

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
use sea_orm::{ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter};
use std::sync::Arc;
use tokio::time::Duration;
use uuid::Uuid;

/// Test bidirectional volume sync - both devices should receive each other's volumes
#[tokio::test]
async fn test_bidirectional_volume_sync() -> anyhow::Result<()> {
	let snapshot_dir = create_snapshot_dir("bidirectional_volume_sync").await?;
	init_test_tracing("bidirectional_volume_sync", &snapshot_dir)?;

	// Use TestDataDir helper for proper cross-platform directory management
	let test_data_alice = TestDataDir::new("volume_sync_alice")?;
	let test_data_bob = TestDataDir::new("volume_sync_bob")?;

	let temp_dir_alice = test_data_alice.core_data_path();
	let temp_dir_bob = test_data_bob.core_data_path();

	tracing::info!("=== Phase 1: Initialize both devices ===");

	TestConfigBuilder::new(temp_dir_alice.clone()).build()?;
	TestConfigBuilder::new(temp_dir_bob.clone()).build()?;

	// Generate a shared library UUID for both devices
	let library_id = Uuid::new_v4();

	let core_alice = Core::new(temp_dir_alice.clone())
		.await
		.map_err(|e| anyhow::anyhow!("Failed to create Alice core: {}", e))?;
	let device_alice_id = core_alice.device.device_id()?;
	let library_alice = core_alice
		.libraries
		.create_library_with_id(
			library_id,
			"Volume Sync Test",
			None,
			core_alice.context.clone(),
		)
		.await?;

	let core_bob = Core::new(temp_dir_bob.clone())
		.await
		.map_err(|e| anyhow::anyhow!("Failed to create Bob core: {}", e))?;
	let device_bob_id = core_bob.device.device_id()?;
	let library_bob = core_bob
		.libraries
		.create_library_with_id(
			library_id,
			"Volume Sync Test",
			None,
			core_bob.context.clone(),
		)
		.await?;

	register_device(&library_alice, device_bob_id, "Bob").await?;
	register_device(&library_bob, device_alice_id, "Alice").await?;

	tracing::info!("=== Phase 2: Create volumes on both devices ===");

	// Alice creates her Macintosh HD
	let _ = create_test_volume(
		&library_alice,
		device_alice_id,
		"alice-macos-hd-fingerprint",
		"Macintosh HD",
	)
	.await?;

	// Bob creates his Macintosh HD
	let _ = create_test_volume(
		&library_bob,
		device_bob_id,
		"bob-macos-hd-fingerprint",
		"Macintosh HD",
	)
	.await?;

	let alice_volumes_before = entities::volume::Entity::find()
		.count(library_alice.db().conn())
		.await?;
	let bob_volumes_before = entities::volume::Entity::find()
		.count(library_bob.db().conn())
		.await?;

	tracing::info!(
		alice_volumes = alice_volumes_before,
		bob_volumes = bob_volumes_before,
		"Volumes created on both devices"
	);

	assert_eq!(
		alice_volumes_before, 1,
		"Alice should have 1 volume before sync"
	);
	assert_eq!(
		bob_volumes_before, 1,
		"Bob should have 1 volume before sync"
	);

	tracing::info!("=== Phase 3: Start sync services ===");

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

	library_alice.sync_service().unwrap().start().await?;
	library_bob.sync_service().unwrap().start().await?;

	tracing::info!("Sync services started - backfill should begin");

	tokio::time::sleep(Duration::from_millis(1000)).await;

	tracing::info!("=== Phase 4: Wait for bidirectional sync ===");

	// Wait for sync with simpler logic for volumes
	let start = tokio::time::Instant::now();
	let max_duration = Duration::from_secs(30);
	let mut stable_iterations = 0;

	while start.elapsed() < max_duration {
		let alice_volumes = entities::volume::Entity::find()
			.count(library_alice.db().conn())
			.await?;
		let bob_volumes = entities::volume::Entity::find()
			.count(library_bob.db().conn())
			.await?;

		tracing::debug!(
			alice_volumes = alice_volumes,
			bob_volumes = bob_volumes,
			elapsed_ms = start.elapsed().as_millis(),
			"Checking sync progress"
		);

		if alice_volumes == 2 && bob_volumes == 2 {
			stable_iterations += 1;
			if stable_iterations >= 5 {
				tracing::info!(
					duration_ms = start.elapsed().as_millis(),
					"Bidirectional volume sync complete"
				);
				break;
			}
		} else {
			stable_iterations = 0;
		}

		tokio::time::sleep(Duration::from_millis(100)).await;
	}

	tracing::info!("=== Phase 5: Verify bidirectional sync ===");

	let alice_volumes_final = entities::volume::Entity::find()
		.count(library_alice.db().conn())
		.await?;
	let bob_volumes_final = entities::volume::Entity::find()
		.count(library_bob.db().conn())
		.await?;

	let alice_volumes_list = entities::volume::Entity::find()
		.all(library_alice.db().conn())
		.await?;
	let bob_volumes_list = entities::volume::Entity::find()
		.all(library_bob.db().conn())
		.await?;

	tracing::info!(
		alice_total = alice_volumes_final,
		bob_total = bob_volumes_final,
		alice_devices = ?alice_volumes_list.iter().map(|v| (v.device_id, v.display_name.clone())).collect::<Vec<_>>(),
		bob_devices = ?bob_volumes_list.iter().map(|v| (v.device_id, v.display_name.clone())).collect::<Vec<_>>(),
		"=== Final volume counts ==="
	);

	assert_eq!(
		alice_volumes_final, 2,
		"Alice should have 2 volumes (her own + Bob's), but has {}",
		alice_volumes_final
	);
	assert_eq!(
		bob_volumes_final, 2,
		"Bob should have 2 volumes (his own + Alice's), but has {}",
		bob_volumes_final
	);

	// Verify Alice has both
	let alice_has_own = alice_volumes_list
		.iter()
		.any(|v| v.device_id == device_alice_id);
	let alice_has_bobs = alice_volumes_list
		.iter()
		.any(|v| v.device_id == device_bob_id);

	assert!(alice_has_own, "Alice should have her own volume");
	assert!(alice_has_bobs, "Alice should have Bob's volume");

	// Verify Bob has both
	let bob_has_own = bob_volumes_list
		.iter()
		.any(|v| v.device_id == device_bob_id);
	let bob_has_alices = bob_volumes_list
		.iter()
		.any(|v| v.device_id == device_alice_id);

	assert!(bob_has_own, "Bob should have his own volume");
	assert!(bob_has_alices, "Bob should have Alice's volume");

	tracing::info!("✅ Bidirectional volume sync verified successfully");

	Ok(())
}

/// Test that volume ResourceChanged events are emitted on the receiving device during sync
#[tokio::test]
async fn test_volume_resource_events_on_sync() -> anyhow::Result<()> {
	let snapshot_dir = create_snapshot_dir("volume_resource_events").await?;
	init_test_tracing("volume_resource_events", &snapshot_dir)?;

	let test_data_alice = TestDataDir::new("volume_events_alice")?;
	let test_data_bob = TestDataDir::new("volume_events_bob")?;

	let temp_dir_alice = test_data_alice.core_data_path();
	let temp_dir_bob = test_data_bob.core_data_path();

	tracing::info!("=== Phase 1: Initialize both devices ===");

	TestConfigBuilder::new(temp_dir_alice.clone()).build()?;
	TestConfigBuilder::new(temp_dir_bob.clone()).build()?;

	let library_id = Uuid::new_v4();

	let core_alice = Core::new(temp_dir_alice.clone())
		.await
		.map_err(|e| anyhow::anyhow!("Failed to create Alice core: {}", e))?;
	let device_alice_id = core_alice.device.device_id()?;
	let library_alice = core_alice
		.libraries
		.create_library_with_id(
			library_id,
			"Volume Event Test",
			None,
			core_alice.context.clone(),
		)
		.await?;

	let core_bob = Core::new(temp_dir_bob.clone())
		.await
		.map_err(|e| anyhow::anyhow!("Failed to create Bob core: {}", e))?;
	let device_bob_id = core_bob.device.device_id()?;
	let library_bob = core_bob
		.libraries
		.create_library_with_id(
			library_id,
			"Volume Event Test",
			None,
			core_bob.context.clone(),
		)
		.await?;

	register_device(&library_alice, device_bob_id, "Bob").await?;
	register_device(&library_bob, device_alice_id, "Alice").await?;

	tracing::info!("=== Phase 2: Create volume on Alice only ===");

	// Alice creates a volume
	let alice_volume_uuid = create_test_volume(
		&library_alice,
		device_alice_id,
		"alice-test-volume",
		"Alice's Test Volume",
	)
	.await?;

	tracing::info!(
		volume_uuid = %alice_volume_uuid,
		"Alice created volume"
	);

	tracing::info!("=== Phase 3: Set up event listener on Bob BEFORE sync ===");

	// Subscribe to Bob's event bus for volume ResourceChanged events
	let mut bob_events = library_bob.event_bus().subscribe();
	let volume_event_received = Arc::new(tokio::sync::Mutex::new(false));
	let volume_event_received_clone = volume_event_received.clone();
	let alice_volume_uuid_clone = alice_volume_uuid;

	// Spawn event listener task
	let event_listener = tokio::spawn(async move {
		use sd_core::infra::event::Event;

		tracing::info!("Bob's event listener started, waiting for volume ResourceChanged...");

		while let Ok(event) = bob_events.recv().await {
			tracing::debug!("Bob received event: {:?}", event);

			match event {
				Event::ResourceChangedBatch {
					resource_type,
					resources,
					..
				} => {
					if resource_type == "volume" {
						tracing::info!(
							resource_count = if let serde_json::Value::Array(arr) = &resources {
								arr.len()
							} else {
								0
							},
							"Bob received ResourceChangedBatch for volumes"
						);

						// Check if Alice's volume is in the batch
						if let serde_json::Value::Array(volume_array) = resources {
							for volume_json in volume_array {
								if let Some(uuid_str) =
									volume_json.get("id").and_then(|v| v.as_str())
								{
									if let Ok(volume_id) = Uuid::parse_str(uuid_str) {
										if volume_id == alice_volume_uuid_clone {
											tracing::info!(
												volume_uuid = %volume_id,
												"✅ Bob received ResourceChanged event for Alice's volume!"
											);
											*volume_event_received_clone.lock().await = true;
											return;
										}
									}
								}
							}
						}
					}
				}
				Event::ResourceChanged {
					resource_type,
					resource,
					..
				} => {
					if resource_type == "volume" {
						tracing::info!("Bob received single ResourceChanged for volume");

						if let Some(uuid_str) = resource.get("id").and_then(|v| v.as_str()) {
							if let Ok(volume_id) = Uuid::parse_str(uuid_str) {
								if volume_id == alice_volume_uuid_clone {
									tracing::info!(
										volume_uuid = %volume_id,
										"✅ Bob received ResourceChanged event for Alice's volume!"
									);
									*volume_event_received_clone.lock().await = true;
									return;
								}
							}
						}
					}
				}
				_ => {
					// Ignore other events
				}
			}
		}
	});

	tracing::info!("=== Phase 4: Start sync services ===");

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

	library_alice.sync_service().unwrap().start().await?;
	library_bob.sync_service().unwrap().start().await?;

	tracing::info!("Sync services started - backfill should begin");

	tracing::info!("=== Phase 5: Wait for volume to sync and event to be emitted ===");

	// Wait for Bob to receive the volume in the database
	let start = tokio::time::Instant::now();
	let max_duration = Duration::from_secs(30);

	loop {
		if start.elapsed() > max_duration {
			anyhow::bail!("Timeout waiting for volume to sync to Bob");
		}

		let bob_volume = entities::volume::Entity::find()
			.filter(entities::volume::Column::Uuid.eq(alice_volume_uuid))
			.one(library_bob.db().conn())
			.await?;

		if bob_volume.is_some() {
			tracing::info!("Bob received Alice's volume in database");
			break;
		}

		tokio::time::sleep(Duration::from_millis(100)).await;
	}

	// Give the event system a moment to emit the event after DB insert
	tokio::time::sleep(Duration::from_millis(500)).await;

	// Check if the event was received
	let event_was_received = *volume_event_received.lock().await;

	// Abort the listener task
	event_listener.abort();

	tracing::info!(event_received = event_was_received, "=== Test Result ===");

	assert!(
		event_was_received,
		"Bob should have received a ResourceChanged event for Alice's volume during sync, but didn't"
	);

	tracing::info!(
		"✅ Volume ResourceChanged event was emitted on the receiving device during sync"
	);

	Ok(())
}
