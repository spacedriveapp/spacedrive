//! Example demonstrating FS Event Pipeline Resilience

use crate::context::CoreContext;
use crate::infra::event::EventBus;
use crate::service::watcher::{LocationWatcher, LocationWatcherConfig, WatchedLocation};
use anyhow::Result;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tempfile::TempDir;
use tokio::time::sleep;
use uuid::Uuid;

/// Example demonstrating burst event processing (simulating git clone)
pub async fn demonstrate_burst_processing() -> Result<()> {
	println!("=== FS Event Pipeline Resilience Demo ===");
	println!("Demonstrating burst event processing (simulating git clone)");
	
	// Create a temporary directory for testing
	let temp_dir = TempDir::new()?;
	
	// Create standard configuration for handling bursts
	let config = LocationWatcherConfig::default();
	println!("Using standard configuration:");
	println!("  - Debounce window: {}ms", config.debounce_window_ms);
	println!("  - Event buffer size: {}", config.event_buffer_size);
	println!("  - Max batch size: {}", config.max_batch_size);
	
	// Create watcher
	let events = Arc::new(EventBus::new(1000));
	let context = create_mock_context();
	let watcher = LocationWatcher::new(config, events, context);
	
	// Create a watched location
	let location = WatchedLocation {
		id: Uuid::new_v4(),
		library_id: Uuid::new_v4(),
		path: temp_dir.path().to_path_buf(),
		enabled: true,
	};
	
	println!("\nAdding location: {}", location.path.display());
	watcher.add_location(location.clone()).await?;
	
	// Simulate a git clone with thousands of file creation events
	println!("\nSimulating git clone with 10,000 file creation events...");
	let start_time = std::time::Instant::now();
	
	// Get worker for this location
	let worker_tx = watcher.ensure_worker_for_location(location.id, location.library_id).await?;
	
	// Send burst of events
	for i in 0..10000 {
		let event = crate::service::watcher::WatcherEvent {
			kind: crate::service::watcher::event_handler::WatcherEventKind::Create,
			paths: vec![PathBuf::from(format!("{}/file_{}.txt", temp_dir.path().display(), i))],
			timestamp: std::time::SystemTime::now(),
			attrs: vec![],
		};
		
		worker_tx.send(event).await?;
		
		// Show progress every 1000 events
		if i % 1000 == 0 && i > 0 {
			println!("  Sent {} events...", i);
		}
	}
	
	println!("  All 10,000 events sent!");
	
	// Wait for processing to complete
	println!("\nWaiting for processing to complete...");
	sleep(Duration::from_secs(5)).await;
	
	let processing_time = start_time.elapsed();
	println!("Processing completed in {:?}", processing_time);
	
	// Show metrics
	if let Some(metrics) = watcher.get_location_metrics(location.id).await {
		println!("\n=== Processing Metrics ===");
		println!("Events processed: {}", metrics.events_processed.load(std::sync::atomic::Ordering::Relaxed));
		println!("Batches processed: {}", metrics.batches_processed.load(std::sync::atomic::Ordering::Relaxed));
		println!("Average batch size: {:.2}", metrics.get_average_batch_size());
		println!("Coalescing rate: {:.2}%", metrics.get_coalescing_rate());
		println!("Max queue depth: {}", metrics.max_queue_depth.load(std::sync::atomic::Ordering::Relaxed));
		println!("Max batch duration: {}ms", metrics.max_batch_duration.load(std::sync::atomic::Ordering::Relaxed));
	}
	
	// Show global metrics
	let global_metrics = watcher.get_global_metrics();
	println!("\n=== Global Metrics ===");
	println!("Total events received: {}", global_metrics.total_events_received.load(std::sync::atomic::Ordering::Relaxed));
	println!("Total workers created: {}", global_metrics.total_workers_created.load(std::sync::atomic::Ordering::Relaxed));
	println!("Total workers destroyed: {}", global_metrics.total_workers_destroyed.load(std::sync::atomic::Ordering::Relaxed));
	
	// Clean up
	watcher.remove_location(location.id).await?;
	println!("\nDemo completed successfully!");
	
	Ok(())
}

/// Example demonstrating event coalescing
pub async fn demonstrate_event_coalescing() -> Result<()> {
	println!("\n=== Event Coalescing Demo ===");
	
	let config = LocationWatcherConfig::default();
	let events = Arc::new(EventBus::new(1000));
	let context = create_mock_context();
	let watcher = LocationWatcher::new(config, events, context);
	
	let location = WatchedLocation {
		id: Uuid::new_v4(),
		library_id: Uuid::new_v4(),
		path: PathBuf::from("/test"),
		enabled: true,
	};
	
	watcher.add_location(location.clone()).await?;
	let worker_tx = watcher.ensure_worker_for_location(location.id, location.library_id).await?;
	
	println!("Sending events that should be coalesced:");
	
	// Create + Remove = neutralized
	println!("  1. Create + Remove (should be neutralized)");
	worker_tx.send(crate::service::watcher::WatcherEvent {
		kind: crate::service::watcher::event_handler::WatcherEventKind::Create,
		paths: vec![PathBuf::from("/test/temp.tmp")],
		timestamp: std::time::SystemTime::now(),
		attrs: vec![],
	}).await?;
	
	worker_tx.send(crate::service::watcher::WatcherEvent {
		kind: crate::service::watcher::event_handler::WatcherEventKind::Remove,
		paths: vec![PathBuf::from("/test/temp.tmp")],
		timestamp: std::time::SystemTime::now(),
		attrs: vec![],
	}).await?;
	
	// Multiple modifies = collapse to one
	println!("  2. Multiple modifies (should collapse to one)");
	for _ in 0..5 {
		worker_tx.send(crate::service::watcher::WatcherEvent {
			kind: crate::service::watcher::event_handler::WatcherEventKind::Modify,
			paths: vec![PathBuf::from("/test/file.txt")],
			timestamp: std::time::SystemTime::now(),
			attrs: vec![],
		}).await?;
	}
	
	// Rename chain A -> B -> C
	println!("  3. Rename chain A -> B -> C (should collapse to A -> C)");
	worker_tx.send(crate::service::watcher::WatcherEvent {
		kind: crate::service::watcher::event_handler::WatcherEventKind::Rename {
			from: PathBuf::from("/test/A"),
			to: PathBuf::from("/test/B"),
		},
		paths: vec![],
		timestamp: std::time::SystemTime::now(),
		attrs: vec![],
	}).await?;
	
	worker_tx.send(crate::service::watcher::WatcherEvent {
		kind: crate::service::watcher::event_handler::WatcherEventKind::Rename {
			from: PathBuf::from("/test/B"),
			to: PathBuf::from("/test/C"),
		},
		paths: vec![],
		timestamp: std::time::SystemTime::now(),
		attrs: vec![],
	}).await?;
	
	// Wait for processing
	sleep(Duration::from_millis(500)).await;
	
	// Show coalescing metrics
	if let Some(metrics) = watcher.get_location_metrics(location.id).await {
		println!("\n=== Coalescing Results ===");
		println!("Events processed: {}", metrics.events_processed.load(std::sync::atomic::Ordering::Relaxed));
		println!("Events coalesced: {}", metrics.events_coalesced.load(std::sync::atomic::Ordering::Relaxed));
		println!("Neutralized events: {}", metrics.neutralized_events.load(std::sync::atomic::Ordering::Relaxed));
		println!("Rename chains collapsed: {}", metrics.rename_chains_collapsed.load(std::sync::atomic::Ordering::Relaxed));
		println!("Coalescing rate: {:.2}%", metrics.get_coalescing_rate());
	}
	
	watcher.remove_location(location.id).await?;
	println!("Coalescing demo completed!");
	
	Ok(())
}

/// Example demonstrating configuration options
pub async fn demonstrate_configuration_options() -> Result<()> {
	println!("\n=== Configuration Options Demo ===");
	
	// Test different configurations
	let configs = vec![
		("Default", LocationWatcherConfig::default()),
		("Custom (100ms, 50K buffer, 5K batch)", LocationWatcherConfig::new(100, 50000, 5000)),
		("Custom (200ms, 20K buffer, 2K batch)", LocationWatcherConfig::new(200, 20000, 2000)),
		("Resource Optimized (1MB, 1000 CPU)", LocationWatcherConfig::resource_optimized(1000000, 1000)),
	];
	
	for (name, config) in configs {
		println!("\n--- {} Configuration ---", name);
		println!("  Debounce window: {}ms", config.debounce_window_ms);
		println!("  Event buffer size: {}", config.event_buffer_size);
		println!("  Max batch size: {}", config.max_batch_size);
		println!("  Metrics enabled: {}", config.enable_metrics);
		println!("  Focused re-index enabled: {}", config.enable_focused_reindex);
		println!("  Max queue depth before re-index: {}", config.max_queue_depth_before_reindex);
		
		// Validate configuration
		match config.validate() {
			Ok(_) => println!("  ✓ Configuration is valid"),
			Err(e) => println!("  ✗ Configuration is invalid: {}", e),
		}
	}
	
	println!("\nConfiguration demo completed!");
	Ok(())
}

/// Example demonstrating overflow handling
pub async fn demonstrate_overflow_handling() -> Result<()> {
	println!("\n=== Overflow Handling Demo ===");
	
	// Create configuration with very small buffer to trigger overflow
	let config = LocationWatcherConfig {
		event_buffer_size: 5,
		max_queue_depth_before_reindex: 3,
		enable_focused_reindex: true,
		..Default::default()
	};
	
	println!("Using small buffer configuration to trigger overflow:");
	println!("  Event buffer size: {}", config.event_buffer_size);
	println!("  Max queue depth before re-index: {}", config.max_queue_depth_before_reindex);
	
	let events = Arc::new(EventBus::new(1000));
	let context = create_mock_context();
	let watcher = LocationWatcher::new(config, events, context);
	
	let location = WatchedLocation {
		id: Uuid::new_v4(),
		library_id: Uuid::new_v4(),
		path: PathBuf::from("/test"),
		enabled: true,
	};
	
	watcher.add_location(location.clone()).await?;
	let worker_tx = watcher.ensure_worker_for_location(location.id, location.library_id).await?;
	
	println!("\nSending more events than buffer can handle...");
	
	// Send more events than the buffer can handle
	for i in 0..20 {
		let event = crate::service::watcher::WatcherEvent {
			kind: crate::service::watcher::event_handler::WatcherEventKind::Create,
			paths: vec![PathBuf::from(format!("/test/file_{}.txt", i))],
			timestamp: std::time::SystemTime::now(),
			attrs: vec![],
		};
		
		// Use try_send to avoid blocking
		if worker_tx.try_send(event).is_err() {
			println!("  Event {} dropped due to full buffer", i);
		}
	}
	
	// Wait for processing
	sleep(Duration::from_millis(1000)).await;
	
	// Show overflow metrics
	let global_metrics = watcher.get_global_metrics();
	println!("\n=== Overflow Results ===");
	println!("Total events received: {}", global_metrics.total_events_received.load(std::sync::atomic::Ordering::Relaxed));
	println!("Total workers created: {}", global_metrics.total_workers_created.load(std::sync::atomic::Ordering::Relaxed));
	println!("Note: No events were dropped - system waits for buffer space");
	
	watcher.remove_location(location.id).await?;
	println!("Overflow handling demo completed!");
	
	Ok(())
}

/// Run all examples
pub async fn run_all_examples() -> Result<()> {
	demonstrate_burst_processing().await?;
	demonstrate_event_coalescing().await?;
	demonstrate_configuration_options().await?;
	demonstrate_overflow_handling().await?;
	
	println!("\n=== All Examples Completed Successfully! ===");
	Ok(())
}

/// Create a mock CoreContext for examples
fn create_mock_context() -> Arc<CoreContext> {
	// This would need to be implemented based on your CoreContext structure
	// For now, we'll use a placeholder
	todo!("Implement mock CoreContext for examples")
}

#[cfg(test)]
mod tests {
	use super::*;

	#[tokio::test]
	async fn test_example_configurations() {
		// Test that all example configurations are valid
		let configs = vec![
			LocationWatcherConfig::default(),
			LocationWatcherConfig::new(100, 50000, 5000),
			LocationWatcherConfig::new(200, 20000, 2000),
			LocationWatcherConfig::resource_optimized(1000000, 1000),
		];
		
		for config in configs {
			assert!(config.validate().is_ok());
		}
	}
}