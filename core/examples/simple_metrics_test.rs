//! Simple test for FS Event Pipeline Metrics Collection

use sd_core::service::watcher::metrics::{LocationWorkerMetrics, WatcherMetrics};
use std::sync::Arc;
use std::time::Duration;
use tracing::{info, Level};
use tracing_subscriber;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
	// Set up logging
	tracing_subscriber::fmt().with_max_level(Level::INFO).init();

	println!("=== Simple Metrics Test ===");

	// Test LocationWorkerMetrics
	let worker_metrics = Arc::new(LocationWorkerMetrics::new());

	// Simulate some activity
	for i in 0..100 {
		worker_metrics.record_event_processed();

		// Simulate some coalescing
		if i % 10 == 0 {
			worker_metrics.record_event_coalesced();
		}

		// Simulate batch processing
		if i % 20 == 0 {
			worker_metrics.record_batch_processed(20, Duration::from_millis(50));
		}
	}

	// Record some rename chain collapses
	for _ in 0..5 {
		worker_metrics.record_rename_chain_collapsed();
	}

	// Record some neutralized events
	for _ in 0..3 {
		worker_metrics.record_neutralized_event();
	}

	// Update queue depth
	worker_metrics.update_queue_depth(15);

	// Log the metrics
	println!("\n=== Worker Metrics ===");
	worker_metrics.log_metrics(uuid::Uuid::new_v4());

	// Test WatcherMetrics
	let watcher_metrics = Arc::new(WatcherMetrics::new());

	// Simulate some activity
	for _ in 0..50 {
		watcher_metrics.record_event_received();
	}

	for _ in 0..3 {
		watcher_metrics.record_worker_created();
	}

	watcher_metrics.record_worker_destroyed();
	watcher_metrics.update_total_locations(2);

	// Log the metrics
	println!("\n=== Watcher Metrics ===");
	watcher_metrics.log_metrics();

	println!("\n=== Test Completed Successfully ===");
	Ok(())
}
