//! Initial Backfill Sync Test
//!
//! Tests the scenario where one device indexes completely before the second device connects.
//! This validates backfill behavior and content_id linkage without real-time sync complexity.

mod helpers;

use helpers::{
	create_snapshot_dir, create_test_volume, init_test_tracing, register_device, wait_for_indexing,
	wait_for_sync, MockTransport, TestConfigBuilder,
};
use sd_core::{
	infra::{db::entities, sync::NetworkTransport},
	location::{create_location, IndexMode, LocationCreateArgs},
	service::Service,
	Core,
};
use sea_orm::{ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter};
use std::sync::Arc;
use tokio::{fs, time::Duration};

#[tokio::test]
async fn test_initial_backfill_alice_indexes_first() -> anyhow::Result<()> {
	let snapshot_dir = create_snapshot_dir("backfill_alice_first").await?;
	init_test_tracing("backfill_alice_first", &snapshot_dir)?;

	let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
	let test_root =
		std::path::PathBuf::from(home).join("Library/Application Support/spacedrive/sync_tests");

	let data_dir = test_root.join("data");
	let temp_dir_alice = data_dir.join("alice_backfill");
	let temp_dir_bob = data_dir.join("bob_backfill");
	fs::create_dir_all(&temp_dir_alice).await?;
	fs::create_dir_all(&temp_dir_bob).await?;

	tracing::info!(
		snapshot_dir = %snapshot_dir.display(),
		alice_dir = %temp_dir_alice.display(),
		bob_dir = %temp_dir_bob.display(),
		"Test directories initialized"
	);

	TestConfigBuilder::new(temp_dir_alice.clone()).build()?;
	TestConfigBuilder::new(temp_dir_bob.clone()).build()?;

	tracing::info!("=== Phase 1: Alice indexes location (Bob not connected yet) ===");

	let core_alice = Core::new(temp_dir_alice.clone())
		.await
		.map_err(|e| anyhow::anyhow!("Failed to create Alice core: {}", e))?;
	let device_alice_id = core_alice.device.device_id()?;
	let library_alice = core_alice
		.libraries
		.create_library_no_sync("Backfill Test Library", None, core_alice.context.clone())
		.await?;

	let device_record = entities::device::Entity::find()
		.one(library_alice.db().conn())
		.await?
		.ok_or_else(|| anyhow::anyhow!("Device not found"))?;

	// Use Spacedrive source code for deterministic testing across all environments
	let test_path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
		.parent()
		.unwrap()
		.to_path_buf();
	let location_args = LocationCreateArgs {
		path: test_path.clone(),
		name: Some("spacedrive".to_string()),
		index_mode: IndexMode::Content,
	};

	let location_db_id = create_location(
		library_alice.clone(),
		library_alice.event_bus(),
		location_args,
		device_record.id,
	)
	.await?;

	tracing::info!(location_id = location_db_id, "Location created on Alice");

	wait_for_indexing(&library_alice, location_db_id, Duration::from_secs(120)).await?;

	let alice_entries_after_index = entities::entry::Entity::find()
		.count(library_alice.db().conn())
		.await?;
	let alice_content_after_index = entities::content_identity::Entity::find()
		.count(library_alice.db().conn())
		.await?;

	tracing::info!(
		entries = alice_entries_after_index,
		content_identities = alice_content_after_index,
		"Alice indexing complete"
	);

	// Add volumes to Alice
	tracing::info!("Adding test volumes to Alice");
	create_test_volume(
		&library_alice,
		device_alice_id,
		"test-vol-1",
		"Alice Volume 1",
	)
	.await?;
	create_test_volume(
		&library_alice,
		device_alice_id,
		"test-vol-2",
		"Alice Volume 2",
	)
	.await?;

	let alice_volumes = entities::volume::Entity::find()
		.count(library_alice.db().conn())
		.await?;
	tracing::info!(volumes = alice_volumes, "Alice has tracked volumes");

	tracing::info!("=== Phase 2: Bob connects and starts backfill ===");

	let core_bob = Core::new(temp_dir_bob.clone())
		.await
		.map_err(|e| anyhow::anyhow!("Failed to create Bob core: {}", e))?;
	let device_bob_id = core_bob.device.device_id()?;
	let library_bob = core_bob
		.libraries
		.create_library_no_sync("Backfill Test Library", None, core_bob.context.clone())
		.await?;

	register_device(&library_alice, device_bob_id, "Bob").await?;
	register_device(&library_bob, device_alice_id, "Alice").await?;

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

	tracing::info!("Sync services started - backfill should begin automatically");

	tokio::time::sleep(Duration::from_millis(500)).await;

	tracing::info!("=== Phase 3: Waiting for backfill to complete ===");

	wait_for_sync(&library_alice, &library_bob, Duration::from_secs(60)).await?;

	let bob_entries_final = entities::entry::Entity::find()
		.count(library_bob.db().conn())
		.await?;
	let bob_content_final = entities::content_identity::Entity::find()
		.count(library_bob.db().conn())
		.await?;
	let alice_volumes_final = entities::volume::Entity::find()
		.count(library_alice.db().conn())
		.await?;
	let bob_volumes_final = entities::volume::Entity::find()
		.count(library_bob.db().conn())
		.await?;

	tracing::info!(
		alice_entries = alice_entries_after_index,
		bob_entries = bob_entries_final,
		alice_content = alice_content_after_index,
		bob_content = bob_content_final,
		alice_volumes = alice_volumes_final,
		bob_volumes = bob_volumes_final,
		"=== Final counts ==="
	);

	let entry_diff = (alice_entries_after_index as i64 - bob_entries_final as i64).abs();
	let content_diff = (alice_content_after_index as i64 - bob_content_final as i64).abs();

	assert!(
		entry_diff <= 5,
		"Entry count mismatch after backfill: Alice {}, Bob {} (diff: {})",
		alice_entries_after_index,
		bob_entries_final,
		entry_diff
	);

	assert!(
		content_diff <= 5,
		"Content identity count mismatch after backfill: Alice {}, Bob {} (diff: {})",
		alice_content_after_index,
		bob_content_final,
		content_diff
	);

	// Verify volume sync
	assert_eq!(
		alice_volumes_final, bob_volumes_final,
		"Volume count mismatch after backfill: Alice {}, Bob {}",
		alice_volumes_final, bob_volumes_final
	);

	tracing::info!(
		alice_volumes = alice_volumes_final,
		bob_volumes = bob_volumes_final,
		"Volume sync verification passed"
	);

	// Verify content_id linkage
	let bob_files_linked = entities::entry::Entity::find()
		.filter(entities::entry::Column::Kind.eq(0))
		.filter(entities::entry::Column::ContentId.is_not_null())
		.count(library_bob.db().conn())
		.await?;
	let bob_total_files = entities::entry::Entity::find()
		.filter(entities::entry::Column::Kind.eq(0))
		.count(library_bob.db().conn())
		.await?;

	let linkage_pct = if bob_total_files > 0 {
		(bob_files_linked * 100) / bob_total_files
	} else {
		0
	};

	tracing::info!(
		bob_files_linked = bob_files_linked,
		bob_total_files = bob_total_files,
		linkage_pct = linkage_pct,
		"Bob's content_id linkage after backfill"
	);

	assert!(
		linkage_pct >= 90,
		"Content_id linkage too low on Bob after backfill: {}% (expected >= 90%)",
		linkage_pct
	);

	Ok(())
}

/// Test bidirectional volume sync - both devices should receive each other's volumes
#[tokio::test]
async fn test_bidirectional_volume_sync() -> anyhow::Result<()> {
	let snapshot_dir = create_snapshot_dir("bidirectional_volume_sync").await?;
	init_test_tracing("bidirectional_volume_sync", &snapshot_dir)?;

	let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
	let test_root =
		std::path::PathBuf::from(home).join("Library/Application Support/spacedrive/sync_tests");

	let data_dir = test_root.join("data");
	let temp_dir_alice = data_dir.join("alice_volume_sync");
	let temp_dir_bob = data_dir.join("bob_volume_sync");
	fs::create_dir_all(&temp_dir_alice).await?;
	fs::create_dir_all(&temp_dir_bob).await?;

	tracing::info!("=== Phase 1: Initialize both devices ===");

	TestConfigBuilder::new(temp_dir_alice.clone()).build()?;
	TestConfigBuilder::new(temp_dir_bob.clone()).build()?;

	let core_alice = Core::new(temp_dir_alice.clone())
		.await
		.map_err(|e| anyhow::anyhow!("Failed to create Alice core: {}", e))?;
	let device_alice_id = core_alice.device.device_id()?;
	let library_alice = core_alice
		.libraries
		.create_library_no_sync("Volume Sync Test", None, core_alice.context.clone())
		.await?;

	let core_bob = Core::new(temp_dir_bob.clone())
		.await
		.map_err(|e| anyhow::anyhow!("Failed to create Bob core: {}", e))?;
	let device_bob_id = core_bob.device.device_id()?;
	let library_bob = core_bob
		.libraries
		.create_library_no_sync("Volume Sync Test", None, core_bob.context.clone())
		.await?;

	register_device(&library_alice, device_bob_id, "Bob").await?;
	register_device(&library_bob, device_alice_id, "Alice").await?;

	tracing::info!("=== Phase 2: Create volumes on both devices ===");

	// Alice creates her Macintosh HD
	create_test_volume(
		&library_alice,
		device_alice_id,
		"alice-macos-hd-fingerprint",
		"Macintosh HD",
	)
	.await?;

	// Bob creates his Macintosh HD
	create_test_volume(
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
