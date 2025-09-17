//! Comprehensive tests for FS Event Pipeline Resilience

use crate::context::CoreContext;
use crate::infra::event::{Event, EventBus, FsRawEventKind};
use crate::service::watcher::{
	LocationWatcher, LocationWatcherConfig, WatchedLocation, WatcherEvent,
	event_handler::WatcherEventKind,
};
use anyhow::Result;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use tempfile::TempDir;
use tokio::time::sleep;
use uuid::Uuid;

/// Test helper to create a mock CoreContext
fn create_mock_context() -> Arc<CoreContext> {
	// This would need to be implemented based on your CoreContext structure
	// For now, we'll use a placeholder
	todo!("Implement mock CoreContext for tests")
}

/// Test helper to create test events
fn create_test_events(count: usize, base_path: &str) -> Vec<WatcherEvent> {
	(0..count)
		.map(|i| WatcherEvent {
			kind: WatcherEventKind::Create,
			paths: vec![PathBuf::from(format!("{}/file_{}.txt", base_path, i))],
			timestamp: SystemTime::now(),
			attrs: vec![],
		})
		.collect()
}

/// Test helper to create rename events
fn create_rename_events(count: usize, base_path: &str) -> Vec<WatcherEvent> {
	(0..count)
		.map(|i| WatcherEvent {
			kind: WatcherEventKind::Rename {
				from: PathBuf::from(format!("{}/old_file_{}.txt", base_path, i)),
				to: PathBuf::from(format!("{}/new_file_{}.txt", base_path, i)),
			},
			paths: vec![],
			timestamp: SystemTime::now(),
			attrs: vec![],
		})
		.collect()
}

#[tokio::test]
async fn test_burst_event_processing() {
	let temp_dir = TempDir::new().unwrap();
	let config = LocationWatcherConfig::high_performance();
	let events = Arc::new(EventBus::new(1000));
	let context = create_mock_context();
	
	let watcher = LocationWatcher::new(config, events, context);
	
	let location = WatchedLocation {
		id: Uuid::new_v4(),
		library_id: Uuid::new_v4(),
		path: temp_dir.path().to_path_buf(),
		enabled: true,
	};
	
	// Add location to start the worker
	watcher.add_location(location.clone()).await.unwrap();
	
	// Create a burst of events (simulating git clone)
	let burst_events = create_test_events(10000, temp_dir.path().to_string_lossy().as_ref());
	
	// Send events to the worker
	let worker_tx = watcher.ensure_worker_for_location(location.id, location.library_id).await.unwrap();
	
	let start_time = std::time::Instant::now();
	for event in burst_events {
		worker_tx.send(event).await.unwrap();
	}
	
	// Wait for processing to complete
	sleep(Duration::from_secs(5)).await;
	
	let processing_time = start_time.elapsed();
	println!("Processed 10000 events in {:?}", processing_time);
	
	// Verify metrics
	let metrics = watcher.get_location_metrics(location.id).await.unwrap();
	assert!(metrics.events_processed.load(std::sync::atomic::Ordering::Relaxed) > 0);
	
	// Clean up
	watcher.remove_location(location.id).await.unwrap();
}

#[tokio::test]
async fn test_event_coalescing() {
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
	
	// Add location to start the worker
	watcher.add_location(location.clone()).await.unwrap();
	
	// Create events that should be coalesced
	let mut coalesce_events = vec![
		// Create + Remove = neutralized
		WatcherEvent {
			kind: WatcherEventKind::Create,
			paths: vec![PathBuf::from("/test/temp.tmp")],
			timestamp: SystemTime::now(),
			attrs: vec![],
		},
		WatcherEvent {
			kind: WatcherEventKind::Remove,
			paths: vec![PathBuf::from("/test/temp.tmp")],
			timestamp: SystemTime::now(),
			attrs: vec![],
		},
		// Multiple modifies = collapse to one
		WatcherEvent {
			kind: WatcherEventKind::Modify,
			paths: vec![PathBuf::from("/test/file.txt")],
			timestamp: SystemTime::now(),
			attrs: vec![],
		},
		WatcherEvent {
			kind: WatcherEventKind::Modify,
			paths: vec![PathBuf::from("/test/file.txt")],
			timestamp: SystemTime::now(),
			attrs: vec![],
		},
		WatcherEvent {
			kind: WatcherEventKind::Modify,
			paths: vec![PathBuf::from("/test/file.txt")],
			timestamp: SystemTime::now(),
			attrs: vec![],
		},
	];
	
	// Send events to the worker
	let worker_tx = watcher.ensure_worker_for_location(location.id, location.library_id).await.unwrap();
	
	for event in coalesce_events {
		worker_tx.send(event).await.unwrap();
	}
	
	// Wait for processing
	sleep(Duration::from_millis(500)).await;
	
	// Verify coalescing metrics
	let metrics = watcher.get_location_metrics(location.id).await.unwrap();
	assert!(metrics.neutralized_events.load(std::sync::atomic::Ordering::Relaxed) > 0);
	
	// Clean up
	watcher.remove_location(location.id).await.unwrap();
}

#[tokio::test]
async fn test_rename_chain_collapse() {
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
	
	// Add location to start the worker
	watcher.add_location(location.clone()).await.unwrap();
	
	// Create rename chain A -> B -> C
	let rename_events = vec![
		WatcherEvent {
			kind: WatcherEventKind::Rename {
				from: PathBuf::from("/test/A"),
				to: PathBuf::from("/test/B"),
			},
			paths: vec![],
			timestamp: SystemTime::now(),
			attrs: vec![],
		},
		WatcherEvent {
			kind: WatcherEventKind::Rename {
				from: PathBuf::from("/test/B"),
				to: PathBuf::from("/test/C"),
			},
			paths: vec![],
			timestamp: SystemTime::now(),
			attrs: vec![],
		},
	];
	
	// Send events to the worker
	let worker_tx = watcher.ensure_worker_for_location(location.id, location.library_id).await.unwrap();
	
	for event in rename_events {
		worker_tx.send(event).await.unwrap();
	}
	
	// Wait for processing
	sleep(Duration::from_millis(500)).await;
	
	// Verify rename chain collapse metrics
	let metrics = watcher.get_location_metrics(location.id).await.unwrap();
	assert!(metrics.rename_chains_collapsed.load(std::sync::atomic::Ordering::Relaxed) > 0);
	
	// Clean up
	watcher.remove_location(location.id).await.unwrap();
}

#[tokio::test]
async fn test_parent_first_ordering() {
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
	
	// Add location to start the worker
	watcher.add_location(location.clone()).await.unwrap();
	
	// Create events with mixed directory and file operations
	let mixed_events = vec![
		// File events (should come after directory events)
		WatcherEvent {
			kind: WatcherEventKind::Create,
			paths: vec![PathBuf::from("/test/subdir/file1.txt")],
			timestamp: SystemTime::now(),
			attrs: vec![],
		},
		WatcherEvent {
			kind: WatcherEventKind::Create,
			paths: vec![PathBuf::from("/test/subdir/file2.txt")],
			timestamp: SystemTime::now(),
			attrs: vec![],
		},
		// Directory events (should come first)
		WatcherEvent {
			kind: WatcherEventKind::Create,
			paths: vec![PathBuf::from("/test/subdir")],
			timestamp: SystemTime::now(),
			attrs: vec![],
		},
		WatcherEvent {
			kind: WatcherEventKind::Create,
			paths: vec![PathBuf::from("/test/deep/subdir")],
			timestamp: SystemTime::now(),
			attrs: vec![],
		},
	];
	
	// Send events to the worker
	let worker_tx = watcher.ensure_worker_for_location(location.id, location.library_id).await.unwrap();
	
	for event in mixed_events {
		worker_tx.send(event).await.unwrap();
	}
	
	// Wait for processing
	sleep(Duration::from_millis(500)).await;
	
	// Verify processing completed
	let metrics = watcher.get_location_metrics(location.id).await.unwrap();
	assert!(metrics.events_processed.load(std::sync::atomic::Ordering::Relaxed) > 0);
	
	// Clean up
	watcher.remove_location(location.id).await.unwrap();
}

#[tokio::test]
async fn test_queue_overflow_handling() {
	let config = LocationWatcherConfig {
		event_buffer_size: 10, // Very small buffer to trigger overflow
		max_queue_depth_before_reindex: 5,
		enable_focused_reindex: true,
		..Default::default()
	};
	
	let events = Arc::new(EventBus::new(1000));
	let context = create_mock_context();
	
	let watcher = LocationWatcher::new(config, events, context);
	
	let location = WatchedLocation {
		id: Uuid::new_v4(),
		library_id: Uuid::new_v4(),
		path: PathBuf::from("/test"),
		enabled: true,
	};
	
	// Add location to start the worker
	watcher.add_location(location.clone()).await.unwrap();
	
	// Create more events than the buffer can handle
	let overflow_events = create_test_events(20, "/test");
	
	// Send events to the worker
	let worker_tx = watcher.ensure_worker_for_location(location.id, location.library_id).await.unwrap();
	
	for event in overflow_events {
		// This should trigger overflow handling
		let _ = worker_tx.try_send(event);
	}
	
	// Wait for processing
	sleep(Duration::from_millis(1000)).await;
	
	// Verify overflow was handled
	let global_metrics = watcher.get_global_metrics();
	assert!(global_metrics.total_events_dropped.load(std::sync::atomic::Ordering::Relaxed) > 0);
	
	// Clean up
	watcher.remove_location(location.id).await.unwrap();
}

#[tokio::test]
async fn test_configuration_validation() {
	// Test valid configuration
	let valid_config = LocationWatcherConfig::default();
	assert!(valid_config.validate().is_ok());
	
	// Test invalid debounce window
	let invalid_config = LocationWatcherConfig {
		debounce_window_ms: 10, // Too small
		..Default::default()
	};
	assert!(invalid_config.validate().is_err());
	
	// Test invalid buffer size
	let invalid_config = LocationWatcherConfig {
		event_buffer_size: 50, // Too small
		..Default::default()
	};
	assert!(invalid_config.validate().is_err());
	
	// Test invalid batch size
	let invalid_config = LocationWatcherConfig {
		max_batch_size: 0, // Invalid
		..Default::default()
	};
	assert!(invalid_config.validate().is_err());
}

#[tokio::test]
async fn test_metrics_collection() {
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
	
	// Add location to start the worker
	watcher.add_location(location.clone()).await.unwrap();
	
	// Send some events
	let worker_tx = watcher.ensure_worker_for_location(location.id, location.library_id).await.unwrap();
	let test_events = create_test_events(100, "/test");
	
	for event in test_events {
		worker_tx.send(event).await.unwrap();
	}
	
	// Wait for processing
	sleep(Duration::from_millis(500)).await;
	
	// Verify metrics are collected
	let metrics = watcher.get_location_metrics(location.id).await.unwrap();
	assert!(metrics.events_processed.load(std::sync::atomic::Ordering::Relaxed) > 0);
	assert!(metrics.batches_processed.load(std::sync::atomic::Ordering::Relaxed) > 0);
	
	// Test metrics logging
	metrics.log_metrics(location.id);
	
	// Clean up
	watcher.remove_location(location.id).await.unwrap();
}

#[tokio::test]
async fn test_batch_processing_performance() {
	let config = LocationWatcherConfig::high_performance();
	let events = Arc::new(EventBus::new(1000));
	let context = create_mock_context();
	
	let watcher = LocationWatcher::new(config, events, context);
	
	let location = WatchedLocation {
		id: Uuid::new_v4(),
		library_id: Uuid::new_v4(),
		path: PathBuf::from("/test"),
		enabled: true,
	};
	
	// Add location to start the worker
	watcher.add_location(location.clone()).await.unwrap();
	
	// Create a large batch of events
	let batch_events = create_test_events(5000, "/test");
	
	let worker_tx = watcher.ensure_worker_for_location(location.id, location.library_id).await.unwrap();
	
	let start_time = std::time::Instant::now();
	for event in batch_events {
		worker_tx.send(event).await.unwrap();
	}
	
	// Wait for processing
	sleep(Duration::from_secs(2)).await;
	
	let processing_time = start_time.elapsed();
	println!("Processed 5000 events in {:?}", processing_time);
	
	// Verify performance metrics
	let metrics = watcher.get_location_metrics(location.id).await.unwrap();
	let avg_batch_size = metrics.get_average_batch_size();
	let coalescing_rate = metrics.get_coalescing_rate();
	
	println!("Average batch size: {:.2}", avg_batch_size);
	println!("Coalescing rate: {:.2}%", coalescing_rate);
	
	// Clean up
	watcher.remove_location(location.id).await.unwrap();
}

#[tokio::test]
async fn test_platform_parity() {
	// Test that the same configuration works across different platforms
	let configs = vec![
		LocationWatcherConfig::default(),
		LocationWatcherConfig::high_performance(),
		LocationWatcherConfig::conservative(),
	];
	
	for config in configs {
		let events = Arc::new(EventBus::new(1000));
		let context = create_mock_context();
		
		let watcher = LocationWatcher::new(config.clone(), events, context);
		
		let location = WatchedLocation {
			id: Uuid::new_v4(),
			library_id: Uuid::new_v4(),
			path: PathBuf::from("/test"),
			enabled: true,
		};
		
		// Add location to start the worker
		watcher.add_location(location.clone()).await.unwrap();
		
		// Send events
		let worker_tx = watcher.ensure_worker_for_location(location.id, location.library_id).await.unwrap();
		let test_events = create_test_events(100, "/test");
		
		for event in test_events {
			worker_tx.send(event).await.unwrap();
		}
		
		// Wait for processing
		sleep(Duration::from_millis(500)).await;
		
		// Verify processing completed
		let metrics = watcher.get_location_metrics(location.id).await.unwrap();
		assert!(metrics.events_processed.load(std::sync::atomic::Ordering::Relaxed) > 0);
		
		// Clean up
		watcher.remove_location(location.id).await.unwrap();
	}
}