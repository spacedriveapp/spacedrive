//! Event System Integration Test
//!
//! Tests the event bus functionality by performing various operations
//! and verifying that the correct events are emitted. This includes:
//! - Core lifecycle events (CoreShutdown)
//! - Library management events (LibraryCreated, LibraryOpened, LibraryClosed)
//! - Location and indexing events (LocationAdded, IndexingStarted)
//! - Job system events (JobProgress, JobCompleted)
//! - Event filtering capabilities (library-specific filtering)
//! - Multiple concurrent subscribers
//! - Custom event emission and handling
//!
//! Note: These tests should be run with --test-threads=1 to avoid
//! potential conflicts between tests

use sd_core::{
	infra::event::{Event, EventFilter},
	location::{create_location, IndexMode, LocationCreateArgs},
	Core,
};
use std::collections::HashSet;
use std::sync::Arc;
use tempfile::TempDir;
use tokio::sync::Mutex;
use tokio::time::{timeout, Duration};

#[tokio::test]
async fn test_core_and_library_events() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
	let temp_dir = TempDir::new()?;

	// Set up event collection
	let collected_events = Arc::new(Mutex::new(Vec::new()));
	let events_clone = collected_events.clone();

	// Initialize core
	let core = Core::new(temp_dir.path().to_path_buf()).await?;

	// Note: CoreStarted is emitted during core initialization, so we won't catch it
	// Start collecting events from now on
	let mut event_subscriber = core.events.subscribe();
	let event_collector = tokio::spawn(async move {
		while let Ok(event) = event_subscriber.recv().await {
			events_clone.lock().await.push(event);
		}
	});

	// Wait a bit for CoreStarted event
	tokio::time::sleep(Duration::from_millis(100)).await;

	// Test 1: Library creation
	let library = core
		.libraries
		.create_library("Test Event Library", None, core.context.clone())
		.await?;
	let library_id = library.id();

	// Wait for events to be processed
	tokio::time::sleep(Duration::from_millis(100)).await;

	// Test 2: Library operations
	let library_path = library.path().to_path_buf();
	drop(library); // Drop the Arc to release the library
	core.libraries.close_library(library_id).await?;
	tokio::time::sleep(Duration::from_millis(100)).await;

	// Open library again by path with context
	let library = core
		.libraries
		.open_library(&library_path, core.context.clone())
		.await?;
	tokio::time::sleep(Duration::from_millis(100)).await;

	// Test 3: Shutdown
	drop(library);
	core.shutdown().await?;

	// Wait for shutdown event to be processed
	tokio::time::sleep(Duration::from_millis(100)).await;

	// Stop event collector
	event_collector.abort();

	// Verify collected events
	let events = collected_events.lock().await;

	// Check for expected events
	let event_types: HashSet<String> = events
		.iter()
		.map(|e| match e {
			Event::CoreStarted => "CoreStarted".to_string(),
			Event::CoreShutdown => "CoreShutdown".to_string(),
			Event::LibraryCreated { .. } => "LibraryCreated".to_string(),
			Event::LibraryOpened { .. } => "LibraryOpened".to_string(),
			Event::LibraryClosed { .. } => "LibraryClosed".to_string(),
			_ => format!("Other({:?})", e),
		})
		.collect();

	println!("Collected events: {:?}", event_types);

	// Verify core events (CoreStarted was emitted before we subscribed)
	assert!(
		event_types.contains("CoreShutdown"),
		"Should emit CoreShutdown event"
	);

	// Verify library events
	assert!(
		event_types.contains("LibraryCreated"),
		"Should emit LibraryCreated event"
	);
	assert!(
		event_types.contains("LibraryClosed"),
		"Should emit LibraryClosed event"
	);
	assert!(
		event_types.contains("LibraryOpened"),
		"Should emit LibraryOpened event"
	);

	Ok(())
}

#[tokio::test]
async fn test_location_and_job_events() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
	let temp_dir = TempDir::new()?;
	let core = Core::new(temp_dir.path().to_path_buf()).await?;

	// Create library
	let library = core
		.libraries
		.create_library("Test Location Events", None, core.context.clone())
		.await?;

	// Set up filtered event collection - only job and indexing events
	let job_events = Arc::new(Mutex::new(Vec::new()));
	let job_events_clone = job_events.clone();

	let mut event_subscriber = core.events.subscribe();
	let event_collector = tokio::spawn(async move {
		while let Ok(event) = event_subscriber.recv().await {
			if event.is_job_event()
				|| matches!(
					event,
					Event::IndexingStarted { .. }
						| Event::IndexingProgress { .. }
						| Event::IndexingCompleted { .. }
						| Event::IndexingFailed { .. }
						| Event::LocationAdded { .. }
				) {
				job_events_clone.lock().await.push(event);
			}
		}
	});

	// Create test location
	let test_location_dir = temp_dir.path().join("test_location");
	tokio::fs::create_dir_all(&test_location_dir).await?;
	tokio::fs::write(test_location_dir.join("test.txt"), "Hello World").await?;

	// Register device
	let db = library.db();
	let device = core.device.to_device()?;

	use sd_core::infra::db::entities;
	use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter};

	let device_record = match entities::device::Entity::find()
		.filter(entities::device::Column::Uuid.eq(device.id))
		.one(db.conn())
		.await?
	{
		Some(existing) => existing,
		None => {
			let device_model: entities::device::ActiveModel = device.into();
			device_model.insert(db.conn()).await?
		}
	};

	// Add location (triggers indexing job)
	let location_args = LocationCreateArgs {
		path: test_location_dir.clone(),
		name: Some("Test Location".to_string()),
		index_mode: IndexMode::Shallow,
	};

	let _location_id = create_location(
		library.clone(),
		&core.events,
		location_args,
		device_record.id,
	)
	.await?;

	// Wait for indexing to complete
	tokio::time::sleep(Duration::from_secs(2)).await;

	// Stop collector and check events
	event_collector.abort();

	let events = job_events.lock().await;
	let event_types: Vec<String> = events
		.iter()
		.map(|e| match e {
			Event::JobQueued { .. } => "JobQueued".to_string(),
			Event::JobStarted { .. } => "JobStarted".to_string(),
			Event::JobProgress { .. } => "JobProgress".to_string(),
			Event::JobCompleted { .. } => "JobCompleted".to_string(),
			Event::IndexingStarted { .. } => "IndexingStarted".to_string(),
			Event::IndexingProgress { .. } => "IndexingProgress".to_string(),
			Event::IndexingCompleted { .. } => "IndexingCompleted".to_string(),
			Event::LocationAdded { .. } => "LocationAdded".to_string(),
			_ => format!("Other({:?})", e),
		})
		.collect();

	println!("Job-related events: {:?}", event_types);

	// Verify job events (Note: JobQueued might not be emitted if job starts immediately)
	assert!(
		event_types.contains(&"JobQueued".to_string())
			|| event_types.contains(&"JobStarted".to_string())
			|| event_types.contains(&"JobProgress".to_string())
			|| event_types.contains(&"JobCompleted".to_string()),
		"Should emit at least one job event"
	);

	// Verify location event
	assert!(
		event_types.contains(&"LocationAdded".to_string()),
		"Should emit LocationAdded event"
	);

	// Cleanup
	let lib_id = library.id();
	core.libraries.close_library(lib_id).await?;
	drop(library);
	core.shutdown().await?;

	Ok(())
}

#[tokio::test]
async fn test_event_filtering() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
	let temp_dir = TempDir::new()?;
	let core = Core::new(temp_dir.path().to_path_buf()).await?;

	// Create two libraries
	let library1 = core
		.libraries
		.create_library("Library 1", None, core.context.clone())
		.await?;
	let lib1_id = library1.id();

	let library2 = core
		.libraries
		.create_library("Library 2", None, core.context.clone())
		.await?;
	let lib2_id = library2.id();

	// Set up filtered event collection - only library1 events
	let lib1_events = Arc::new(Mutex::new(Vec::new()));
	let lib1_events_clone = lib1_events.clone();

	let event_subscriber = core.events.subscribe();
	let event_collector = tokio::spawn(async move {
		let mut subscriber = event_subscriber;
		loop {
			match timeout(
				Duration::from_millis(100),
				subscriber.recv_filtered(|e| e.is_for_library(lib1_id)),
			)
			.await
			{
				Ok(Ok(event)) => {
					lib1_events_clone.lock().await.push(event);
				}
				_ => break,
			}
		}
	});

	// Perform operations on both libraries
	core.libraries.close_library(lib1_id).await?;
	core.libraries.close_library(lib2_id).await?;

	// Wait and stop collector
	tokio::time::sleep(Duration::from_millis(500)).await;
	event_collector.abort();

	// Check filtered events
	let events = lib1_events.lock().await;
	for event in events.iter() {
		match event {
			Event::LibraryCreated { id, .. } | Event::LibraryClosed { id, .. } => {
				assert_eq!(id, &lib1_id, "Should only receive library1 events");
			}
			_ => {}
		}
	}

	// Cleanup
	drop(library1);
	drop(library2);
	core.shutdown().await?;

	Ok(())
}

#[tokio::test]
async fn test_concurrent_event_subscribers() -> Result<(), Box<dyn std::error::Error + Send + Sync>>
{
	let temp_dir = TempDir::new()?;
	let core = Core::new(temp_dir.path().to_path_buf()).await?;

	// Create multiple subscribers
	let subscriber1_events = Arc::new(Mutex::new(Vec::new()));
	let subscriber2_events = Arc::new(Mutex::new(Vec::new()));
	let subscriber3_events = Arc::new(Mutex::new(Vec::new()));

	let events1 = subscriber1_events.clone();
	let events2 = subscriber2_events.clone();
	let events3 = subscriber3_events.clone();

	let mut sub1 = core.events.subscribe();
	let mut sub2 = core.events.subscribe();
	let mut sub3 = core.events.subscribe();

	// Start collectors
	let collector1 = tokio::spawn(async move {
		while let Ok(event) = sub1.recv().await {
			if matches!(event, Event::LibraryCreated { .. }) {
				events1.lock().await.push(event);
			}
		}
	});

	let collector2 = tokio::spawn(async move {
		while let Ok(event) = sub2.recv().await {
			if matches!(event, Event::LibraryCreated { .. }) {
				events2.lock().await.push(event);
			}
		}
	});

	let collector3 = tokio::spawn(async move {
		while let Ok(event) = sub3.recv().await {
			if matches!(event, Event::LibraryCreated { .. }) {
				events3.lock().await.push(event);
			}
		}
	});

	// Create a library (should be received by all subscribers)
	let library = core
		.libraries
		.create_library("Broadcast Test", None, core.context.clone())
		.await?;

	// Wait for events
	tokio::time::sleep(Duration::from_millis(200)).await;

	// Stop collectors
	collector1.abort();
	collector2.abort();
	collector3.abort();

	// Verify all subscribers received the event
	assert_eq!(
		subscriber1_events.lock().await.len(),
		1,
		"Subscriber 1 should receive event"
	);
	assert_eq!(
		subscriber2_events.lock().await.len(),
		1,
		"Subscriber 2 should receive event"
	);
	assert_eq!(
		subscriber3_events.lock().await.len(),
		1,
		"Subscriber 3 should receive event"
	);

	// Cleanup
	let lib_id = library.id();
	core.libraries.close_library(lib_id).await?;
	drop(library);
	core.shutdown().await?;

	Ok(())
}

#[tokio::test]
async fn test_custom_events() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
	let temp_dir = TempDir::new()?;
	let core = Core::new(temp_dir.path().to_path_buf()).await?;

	let collected_events = Arc::new(Mutex::new(Vec::new()));
	let events_clone = collected_events.clone();

	let mut event_subscriber = core.events.subscribe();
	let event_collector = tokio::spawn(async move {
		while let Ok(event) = event_subscriber.recv().await {
			if matches!(event, Event::Custom { .. }) {
				events_clone.lock().await.push(event);
			}
		}
	});

	// Emit custom events
	let custom_data = serde_json::json!({
		"action": "test_action",
		"value": 42,
		"message": "Custom event test"
	});

	core.events.emit(Event::Custom {
		event_type: "test_event".to_string(),
		data: custom_data.clone(),
	});

	// Wait for event processing
	tokio::time::sleep(Duration::from_millis(100)).await;

	// Stop collector
	event_collector.abort();

	// Verify custom event
	let events = collected_events.lock().await;
	assert_eq!(events.len(), 1, "Should receive one custom event");

	if let Event::Custom { event_type, data } = &events[0] {
		assert_eq!(event_type, "test_event");
		assert_eq!(data, &custom_data);
	} else {
		panic!("Expected custom event");
	}

	// Test subscriber count
	let subscriber_count = core.events.subscriber_count();
	println!("Event subscribers: {}", subscriber_count);
	assert!(subscriber_count > 0, "Should have active subscribers");

	core.shutdown().await?;

	Ok(())
}
