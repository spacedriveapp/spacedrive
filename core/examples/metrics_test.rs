//! Example demonstrating FS Event Pipeline Metrics Collection

use anyhow::Result;
use sd_core::context::CoreContext;
use sd_core::infra::event::EventBus;
use sd_core::service::watcher::{LocationWatcher, LocationWatcherConfig, WatchedLocation};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tempfile::TempDir;
use tokio::time::sleep;
use uuid::Uuid;

#[tokio::main]
async fn main() -> Result<()> {
	// Set up logging to see metrics
	tracing_subscriber::fmt()
		.with_env_filter("sd_core::service::watcher=debug,info")
		.init();

	println!("=== FS Event Pipeline Metrics Test ===");

	// Create a temporary directory for testing
	let temp_dir = TempDir::new()?;

	// Create configuration with frequent metrics logging for testing
	let config = LocationWatcherConfig {
		debounce_window_ms: 100,       // Short debounce for testing
		metrics_log_interval_ms: 2000, // Log metrics every 2 seconds
		enable_metrics: true,
		..Default::default()
	};

	println!("Configuration:");
	println!("  - Debounce window: {}ms", config.debounce_window_ms);
	println!("  - Metrics interval: {}ms", config.metrics_log_interval_ms);
	println!("  - Event buffer size: {}", config.event_buffer_size);
	println!("  - Max batch size: {}", config.max_batch_size);

	// Create watcher
	let events = Arc::new(EventBus::new(1000));
	let context = create_mock_context();
	let watcher = LocationWatcher::new(config, events, context);

	// Start the watcher
	watcher.start().await?;

	// Create a watched location
	let location = WatchedLocation {
		id: Uuid::new_v4(),
		library_id: Uuid::new_v4(),
		path: temp_dir.path().to_path_buf(),
		enabled: true,
	};

	println!("\nAdding location: {}", location.path.display());
	watcher.add_location(location.clone()).await?;

	// Simulate file operations
	println!("\nSimulating file operations...");

	// Create some files
	for i in 0..10 {
		let file_path = temp_dir.path().join(format!("file_{}.txt", i));
		std::fs::write(&file_path, format!("Content {}", i))?;
		println!("  Created: {}", file_path.display());
	}

	// Wait a bit for processing
	sleep(Duration::from_millis(500)).await;

	// Modify some files
	for i in 0..5 {
		let file_path = temp_dir.path().join(format!("file_{}.txt", i));
		std::fs::write(&file_path, format!("Modified content {}", i))?;
		println!("  Modified: {}", file_path.display());
	}

	// Wait for metrics to be logged
	println!("\nWaiting for metrics to be logged...");
	sleep(Duration::from_secs(3)).await;

	// Manually trigger metrics logging
	println!("\nManually triggering metrics logging:");
	watcher.log_metrics_now().await;

	// Show global metrics
	let global_metrics = watcher.get_global_metrics();
	println!("\n=== Global Metrics ===");
	println!(
		"Total events received: {}",
		global_metrics
			.total_events_received
			.load(std::sync::atomic::Ordering::Relaxed)
	);
	println!(
		"Total workers created: {}",
		global_metrics
			.total_workers_created
			.load(std::sync::atomic::Ordering::Relaxed)
	);

	// Show location-specific metrics
	if let Some(location_metrics) = watcher.get_location_metrics(location.id).await {
		println!("\n=== Location Metrics ===");
		println!(
			"Events processed: {}",
			location_metrics
				.events_processed
				.load(std::sync::atomic::Ordering::Relaxed)
		);
		println!(
			"Events coalesced: {}",
			location_metrics
				.events_coalesced
				.load(std::sync::atomic::Ordering::Relaxed)
		);
		println!(
			"Batches processed: {}",
			location_metrics
				.batches_processed
				.load(std::sync::atomic::Ordering::Relaxed)
		);
		println!(
			"Average batch size: {:.2}",
			location_metrics.get_average_batch_size()
		);
		println!(
			"Coalescing rate: {:.2}%",
			location_metrics.get_coalescing_rate()
		);
		println!(
			"Max queue depth: {}",
			location_metrics
				.max_queue_depth
				.load(std::sync::atomic::Ordering::Relaxed)
		);
	}

	// Clean up
	watcher.remove_location(location.id).await?;
	watcher.stop().await?;

	println!("\n=== Test Completed ===");
	Ok(())
}

/// Create a mock CoreContext for testing
fn create_mock_context() -> Arc<CoreContext> {
	// This is a placeholder - in a real implementation, you'd need to create
	// a proper CoreContext with database connections, etc.
	todo!("Implement mock CoreContext for metrics testing")
}

