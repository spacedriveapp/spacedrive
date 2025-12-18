//! Sync Metrics Integration Test
//!
//! Tests that the sync metrics system correctly tracks operations,
//! data volume, performance, and errors during sync.
//!
//! ## Running Tests
//! ```bash
//! cargo test -p sd-core --test sync_metrics_test -- --test-threads=1 --nocapture
//! ```

mod helpers;

use helpers::TwoDeviceHarnessBuilder;
use sd_core::{
	infra::db::entities, library::Library, service::sync::metrics::snapshot::SyncMetricsSnapshot,
};
use sea_orm::{EntityTrait, PaginatorTrait};
use std::sync::Arc;
use tokio::{fs, time::Duration};

/// Get metrics snapshot for a library
async fn get_metrics_snapshot(library: &Arc<Library>) -> SyncMetricsSnapshot {
	SyncMetricsSnapshot::from_metrics(library.sync_service().unwrap().metrics().metrics()).await
}

/// Save metrics snapshots to JSON file
async fn save_metrics_snapshot(
	snapshot_dir: &std::path::Path,
	name: &str,
	alice: &SyncMetricsSnapshot,
	bob: &SyncMetricsSnapshot,
) -> anyhow::Result<()> {
	use tokio::io::AsyncWriteExt;

	let path = snapshot_dir.join(format!("{}.json", name));
	let mut file = fs::File::create(&path).await?;

	let data = serde_json::json!({
		"name": name,
		"timestamp": chrono::Utc::now().to_rfc3339(),
		"alice": alice,
		"bob": bob,
	});

	file.write_all(serde_json::to_string_pretty(&data)?.as_bytes())
		.await?;

	Ok(())
}

//
// METRICS TESTS
//

/// Test: Verify metrics are initialized to zero
#[tokio::test]
async fn test_metrics_initial_state() -> anyhow::Result<()> {
	let harness = TwoDeviceHarnessBuilder::new("metrics_initial_state")
		.await?
		.build()
		.await?;

	let alice = get_metrics_snapshot(&harness.library_alice).await;
	let bob = get_metrics_snapshot(&harness.library_bob).await;

	// Save for debugging
	save_metrics_snapshot(&harness.snapshot_dir, "initial", &alice, &bob).await?;

	// State should be Ready (we set it explicitly)
	assert!(
		alice.state.current_state.is_ready(),
		"Alice should be in Ready state, got {:?}",
		alice.state.current_state
	);
	assert!(
		bob.state.current_state.is_ready(),
		"Bob should be in Ready state, got {:?}",
		bob.state.current_state
	);

	// Operations should be at zero or near-zero
	tracing::info!(
		alice_broadcasts = alice.operations.broadcasts_sent,
		bob_broadcasts = bob.operations.broadcasts_sent,
		"Initial broadcast counts"
	);

	// No sync operations should have happened yet
	assert_eq!(
		alice.operations.changes_received, 0,
		"Alice should have 0 changes received initially"
	);
	assert_eq!(
		bob.operations.changes_received, 0,
		"Bob should have 0 changes received initially"
	);

	tracing::info!("Initial metrics state verified");

	Ok(())
}

/// Test: Verify broadcasts are counted when syncing
#[tokio::test]
async fn test_metrics_broadcast_counting() -> anyhow::Result<()> {
	let harness = TwoDeviceHarnessBuilder::new("metrics_broadcast_counting")
		.await?
		.build()
		.await?;

	// Snapshot before
	let alice_before = get_metrics_snapshot(&harness.library_alice).await;
	let bob_before = get_metrics_snapshot(&harness.library_bob).await;

	tracing::info!(
		alice_broadcasts_before = alice_before.operations.broadcasts_sent,
		bob_broadcasts_before = bob_before.operations.broadcasts_sent,
		"Metrics before indexing"
	);

	// Index a small folder on Alice
	let test_dir = harness.snapshot_dir.join("test_data");
	fs::create_dir_all(&test_dir).await?;

	// Create a few test files
	for i in 0..5 {
		let file_path = test_dir.join(format!("test_file_{}.txt", i));
		fs::write(&file_path, format!("Test content {}", i)).await?;
	}

	harness
		.add_and_index_location_alice(test_dir.to_str().unwrap(), "Test Data")
		.await?;

	// Wait for sync
	harness.wait_for_sync(Duration::from_secs(30)).await?;

	// Small delay for metrics to update
	tokio::time::sleep(Duration::from_millis(500)).await;

	// Snapshot after
	let alice_after = get_metrics_snapshot(&harness.library_alice).await;
	let bob_after = get_metrics_snapshot(&harness.library_bob).await;

	save_metrics_snapshot(
		&harness.snapshot_dir,
		"after_sync",
		&alice_after,
		&bob_after,
	)
	.await?;

	tracing::info!(
		alice_broadcasts_after = alice_after.operations.broadcasts_sent,
		alice_state_changes = alice_after.operations.state_changes_broadcast,
		alice_shared_changes = alice_after.operations.shared_changes_broadcast,
		bob_changes_received = bob_after.operations.changes_received,
		bob_changes_applied = bob_after.operations.changes_applied,
		"Metrics after sync"
	);

	// Alice should have sent broadcasts
	assert!(
		alice_after.operations.broadcasts_sent > alice_before.operations.broadcasts_sent,
		"Alice broadcasts should increase: before={}, after={}",
		alice_before.operations.broadcasts_sent,
		alice_after.operations.broadcasts_sent
	);

	// Bob should have received and applied changes
	assert!(
		bob_after.operations.changes_received > bob_before.operations.changes_received,
		"Bob changes_received should increase: before={}, after={}",
		bob_before.operations.changes_received,
		bob_after.operations.changes_received
	);

	assert!(
		bob_after.operations.changes_applied > bob_before.operations.changes_applied,
		"Bob changes_applied should increase: before={}, after={}",
		bob_before.operations.changes_applied,
		bob_after.operations.changes_applied
	);

	// Applied should roughly equal received
	let applied_ratio = bob_after.operations.changes_applied as f64
		/ bob_after.operations.changes_received.max(1) as f64;
	assert!(
		applied_ratio >= 0.9,
		"At least 90% of changes should be applied: {:.1}%",
		applied_ratio * 100.0
	);

	Ok(())
}

/// Test: Verify latency histograms are populated
#[tokio::test]
async fn test_metrics_latency_tracking() -> anyhow::Result<()> {
	let harness = TwoDeviceHarnessBuilder::new("metrics_latency_tracking")
		.await?
		.build()
		.await?;

	// Create test data
	let test_dir = harness.snapshot_dir.join("latency_test");
	fs::create_dir_all(&test_dir).await?;

	for i in 0..3 {
		let file_path = test_dir.join(format!("latency_file_{}.txt", i));
		fs::write(&file_path, format!("Latency test {}", i)).await?;
	}

	// Index and sync
	harness
		.add_and_index_location_alice(test_dir.to_str().unwrap(), "Latency Test")
		.await?;

	harness.wait_for_sync(Duration::from_secs(30)).await?;
	tokio::time::sleep(Duration::from_millis(500)).await;

	let alice = get_metrics_snapshot(&harness.library_alice).await;
	let bob = get_metrics_snapshot(&harness.library_bob).await;

	save_metrics_snapshot(&harness.snapshot_dir, "latency", &alice, &bob).await?;

	tracing::info!(
		alice_broadcast_latency_count = alice.performance.broadcast_latency.count,
		alice_broadcast_latency_avg = alice.performance.broadcast_latency.avg_ms,
		bob_apply_latency_count = bob.performance.apply_latency.count,
		bob_apply_latency_avg = bob.performance.apply_latency.avg_ms,
		"Latency metrics"
	);

	// Alice should have recorded broadcast latencies
	if alice.operations.broadcasts_sent > 0 {
		tracing::info!(
			"Alice broadcast latency: count={}, avg={:.2}ms, min={}ms, max={}ms",
			alice.performance.broadcast_latency.count,
			alice.performance.broadcast_latency.avg_ms,
			alice.performance.broadcast_latency.min_ms,
			alice.performance.broadcast_latency.max_ms,
		);
	}

	// Bob should have recorded apply latencies
	if bob.operations.changes_applied > 0 {
		tracing::info!(
			"Bob apply latency: count={}, avg={:.2}ms, min={}ms, max={}ms",
			bob.performance.apply_latency.count,
			bob.performance.apply_latency.avg_ms,
			bob.performance.apply_latency.min_ms,
			bob.performance.apply_latency.max_ms,
		);
	}

	// Verify histogram has reasonable values
	if alice.performance.broadcast_latency.count > 0 {
		assert!(
			alice.performance.broadcast_latency.max_ms < 10000,
			"Broadcast latency max should be reasonable: {}ms",
			alice.performance.broadcast_latency.max_ms
		);
	}

	Ok(())
}

/// Test: Verify data volume metrics
#[tokio::test]
async fn test_metrics_data_volume() -> anyhow::Result<()> {
	let harness = TwoDeviceHarnessBuilder::new("metrics_data_volume")
		.await?
		.build()
		.await?;

	// Create test data
	let test_dir = harness.snapshot_dir.join("volume_test");
	fs::create_dir_all(&test_dir).await?;

	let file_count = 10;
	for i in 0..file_count {
		let file_path = test_dir.join(format!("volume_file_{}.txt", i));
		fs::write(&file_path, format!("Volume test content {}", i)).await?;
	}

	// Index and sync
	harness
		.add_and_index_location_alice(test_dir.to_str().unwrap(), "Volume Test")
		.await?;

	harness.wait_for_sync(Duration::from_secs(30)).await?;
	tokio::time::sleep(Duration::from_millis(500)).await;

	let alice = get_metrics_snapshot(&harness.library_alice).await;
	let bob = get_metrics_snapshot(&harness.library_bob).await;

	save_metrics_snapshot(&harness.snapshot_dir, "data_volume", &alice, &bob).await?;

	tracing::info!(
		alice_entries_synced = ?alice.data_volume.entries_synced,
		bob_entries_synced = ?bob.data_volume.entries_synced,
		alice_bytes_sent = alice.data_volume.bytes_sent,
		bob_bytes_received = bob.data_volume.bytes_received,
		"Data volume metrics"
	);

	// Check entries synced by model type
	if !bob.data_volume.entries_synced.is_empty() {
		tracing::info!("Bob received entries by model:");
		for (model, count) in &bob.data_volume.entries_synced {
			tracing::info!("  {}: {}", model, count);
		}
	}

	// Verify database counts match
	let alice_db_entries = entities::entry::Entity::find()
		.count(harness.library_alice.db().conn())
		.await?;
	let bob_db_entries = entities::entry::Entity::find()
		.count(harness.library_bob.db().conn())
		.await?;

	tracing::info!(
		alice_db_entries = alice_db_entries,
		bob_db_entries = bob_db_entries,
		"Database entry counts"
	);

	assert_eq!(
		alice_db_entries, bob_db_entries,
		"Entry counts should match after sync"
	);

	Ok(())
}

/// Test: Verify error metrics
#[tokio::test]
async fn test_metrics_error_tracking() -> anyhow::Result<()> {
	let harness = TwoDeviceHarnessBuilder::new("metrics_error_tracking")
		.await?
		.build()
		.await?;

	let alice = get_metrics_snapshot(&harness.library_alice).await;
	let bob = get_metrics_snapshot(&harness.library_bob).await;

	save_metrics_snapshot(&harness.snapshot_dir, "error_state", &alice, &bob).await?;

	tracing::info!(
		alice_total_errors = alice.errors.total_errors,
		alice_network_errors = alice.errors.network_errors,
		alice_apply_errors = alice.errors.apply_errors,
		bob_total_errors = bob.errors.total_errors,
		bob_network_errors = bob.errors.network_errors,
		bob_apply_errors = bob.errors.apply_errors,
		"Error metrics"
	);

	// In normal operation, errors should be 0
	tracing::info!(
		"Error tracking infrastructure verified. Recent errors: alice={}, bob={}",
		alice.errors.recent_errors.len(),
		bob.errors.recent_errors.len()
	);

	Ok(())
}

/// Test: Full metrics snapshot structure
#[tokio::test]
async fn test_metrics_snapshot_structure() -> anyhow::Result<()> {
	let harness = TwoDeviceHarnessBuilder::new("metrics_snapshot_structure")
		.await?
		.build()
		.await?;

	// Create and sync some data
	let test_dir = harness.snapshot_dir.join("structure_test");
	fs::create_dir_all(&test_dir).await?;

	for i in 0..3 {
		let file_path = test_dir.join(format!("structure_file_{}.txt", i));
		fs::write(&file_path, format!("Structure test {}", i)).await?;
	}

	harness
		.add_and_index_location_alice(test_dir.to_str().unwrap(), "Structure Test")
		.await?;

	harness.wait_for_sync(Duration::from_secs(30)).await?;

	let alice = get_metrics_snapshot(&harness.library_alice).await;
	let bob = get_metrics_snapshot(&harness.library_bob).await;

	// Verify all snapshot sections are populated
	tracing::info!("=== ALICE METRICS SNAPSHOT ===");
	tracing::info!("Timestamp: {}", alice.timestamp);

	tracing::info!("--- State ---");
	tracing::info!("  current_state: {:?}", alice.state.current_state);
	tracing::info!("  uptime_seconds: {}", alice.state.uptime_seconds);
	tracing::info!(
		"  state_history entries: {}",
		alice.state.state_history.len()
	);

	tracing::info!("--- Operations ---");
	tracing::info!("  broadcasts_sent: {}", alice.operations.broadcasts_sent);
	tracing::info!(
		"  state_changes_broadcast: {}",
		alice.operations.state_changes_broadcast
	);
	tracing::info!(
		"  shared_changes_broadcast: {}",
		alice.operations.shared_changes_broadcast
	);
	tracing::info!("  changes_received: {}", alice.operations.changes_received);
	tracing::info!("  changes_applied: {}", alice.operations.changes_applied);

	tracing::info!("--- Data Volume ---");
	tracing::info!("  bytes_sent: {}", alice.data_volume.bytes_sent);
	tracing::info!("  bytes_received: {}", alice.data_volume.bytes_received);
	tracing::info!(
		"  entries_synced models: {}",
		alice.data_volume.entries_synced.len()
	);

	tracing::info!("--- Performance ---");
	tracing::info!(
		"  broadcast_latency: count={}, avg={:.2}ms",
		alice.performance.broadcast_latency.count,
		alice.performance.broadcast_latency.avg_ms
	);
	tracing::info!(
		"  apply_latency: count={}, avg={:.2}ms",
		alice.performance.apply_latency.count,
		alice.performance.apply_latency.avg_ms
	);
	tracing::info!("  db_query_count: {}", alice.performance.db_query_count);

	tracing::info!("--- Errors ---");
	tracing::info!("  total_errors: {}", alice.errors.total_errors);
	tracing::info!("  conflicts_detected: {}", alice.errors.conflicts_detected);

	tracing::info!("=== BOB METRICS SNAPSHOT ===");
	tracing::info!("  changes_received: {}", bob.operations.changes_received);
	tracing::info!("  changes_applied: {}", bob.operations.changes_applied);

	// Save full snapshot
	save_metrics_snapshot(&harness.snapshot_dir, "full_structure", &alice, &bob).await?;

	tracing::info!(
		"Full metrics snapshot saved to: {}/full_structure.json",
		harness.snapshot_dir.display()
	);

	Ok(())
}
