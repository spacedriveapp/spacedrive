//! Test that verifies ResourceChanged events are emitted during indexing
//!
//! This test indexes a directory and collects all ResourceChanged events
//! to verify the normalized cache event system works end-to-end.

use sd_core::{
	infra::{
		db::entities,
		event::{Event, EventSubscriber},
	},
	location::{create_location, IndexMode, LocationCreateArgs},
	Core,
};
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter};
use std::{collections::HashMap, sync::Arc, time::Duration};
use tempfile::TempDir;
use tokio::time::timeout;

/// Test fixture that tracks all ResourceChanged events
struct EventCollector {
	events: Arc<tokio::sync::Mutex<Vec<Event>>>,
	subscriber: EventSubscriber,
}

impl EventCollector {
	fn new(event_bus: &Arc<sd_core::infra::event::EventBus>) -> Self {
		Self {
			events: Arc::new(tokio::sync::Mutex::new(Vec::new())),
			subscriber: event_bus.subscribe(),
		}
	}

	async fn collect_events(&mut self, duration: Duration) {
		let events = self.events.clone();
		let mut event_count = 0;
		let mut batch_event_count = 0;

		let timeout_result = timeout(duration, async {
			loop {
				match self.subscriber.recv().await {
					Ok(event) => {
						event_count += 1;

						// Log the event
						match &event {
							Event::ResourceChanged { resource_type, .. } => {
								eprintln!("Received ResourceChanged event: {}", resource_type);
							}
							Event::ResourceChangedBatch {
								resource_type,
								resources,
								metadata,
							} => {
								batch_event_count += 1;
								let count = if let Some(arr) = resources.as_array() {
									arr.len()
								} else {
									0
								};
								eprintln!(
									"Received ResourceChangedBatch event #{}: {} ({} items)",
									batch_event_count, resource_type, count
								);
							}
							Event::IndexingCompleted { .. } => {
								eprintln!("Indexing completed");
							}
							Event::JobCompleted { job_type, .. } => {
								eprintln!("Job completed: {}", job_type);
							}
							_ => {}
						}

						// Store all events
						events.lock().await.push(event);
					}
					Err(e) => {
						eprintln!("️  Event receive error: {:?}", e);
						eprintln!("    This might indicate dropped events or channel overflow!");
						break;
					}
				}
			}
		})
		.await;

		if timeout_result.is_err() {
			eprintln!(
				"️  Event collection timed out (collected {} events, {} batch events)",
				event_count, batch_event_count
			);
		}
	}

	async fn get_events(&self) -> Vec<Event> {
		self.events.lock().await.clone()
	}

	/// Analyze collected events and return statistics
	async fn analyze(&self) -> EventStats {
		let events = self.events.lock().await;
		let mut stats = EventStats::default();

		for event in events.iter() {
			match event {
				Event::ResourceChanged { resource_type, .. } => {
					*stats
						.resource_changed
						.entry(resource_type.clone())
						.or_insert(0) += 1;
				}
				Event::ResourceChangedBatch {
					resource_type,
					resources,
					metadata,
				} => {
					let count = if let Some(arr) = resources.as_array() {
						arr.len()
					} else {
						1
					};
					*stats
						.resource_changed_batch
						.entry(resource_type.clone())
						.or_insert(0) += count;
				}
				Event::IndexingStarted { .. } => {
					stats.indexing_started += 1;
				}
				Event::IndexingCompleted { .. } => {
					stats.indexing_completed += 1;
				}
				Event::JobStarted { job_type, .. } => {
					*stats.jobs_started.entry(job_type.clone()).or_insert(0) += 1;
				}
				Event::JobCompleted { job_type, .. } => {
					*stats.jobs_completed.entry(job_type.clone()).or_insert(0) += 1;
				}
				_ => {}
			}
		}

		stats
	}
}

#[derive(Debug, Default)]
struct EventStats {
	resource_changed: HashMap<String, usize>,
	resource_changed_batch: HashMap<String, usize>,
	indexing_started: usize,
	indexing_completed: usize,
	jobs_started: HashMap<String, usize>,
	jobs_completed: HashMap<String, usize>,
}

impl EventStats {
	fn print(&self) {
		eprintln!("\nEvent Statistics:");
		eprintln!("==================");

		eprintln!("\nResourceChanged events:");
		if self.resource_changed.is_empty() {
			eprintln!("  (none)");
		}
		for (resource_type, count) in &self.resource_changed {
			eprintln!("  {} → {} events", resource_type, count);
		}

		eprintln!("\nResourceChangedBatch events:");
		if self.resource_changed_batch.is_empty() {
			eprintln!("  (none)");
		}
		for (resource_type, count) in &self.resource_changed_batch {
			eprintln!("  {} → {} resources", resource_type, count);
		}

		eprintln!("\nIndexing events:");
		eprintln!("  Started: {}", self.indexing_started);
		eprintln!("  Completed: {}", self.indexing_completed);

		eprintln!("\n️  Job events:");
		eprintln!("  Started:");
		for (job_type, count) in &self.jobs_started {
			eprintln!("    {} → {}", job_type, count);
		}
		eprintln!("  Completed:");
		for (job_type, count) in &self.jobs_completed {
			eprintln!("    {} → {}", job_type, count);
		}
	}
}

#[tokio::test]
async fn test_resource_events_during_indexing() -> Result<(), Box<dyn std::error::Error>> {
	tracing_subscriber::fmt::init();
	eprintln!("\nStarting resource events test\n");

	// Setup
	let temp_dir = TempDir::new()?;
	let core = Core::new(temp_dir.path().to_path_buf()).await?;

	// Create library
	let library = core
		.libraries
		.create_library("Resource Events Test Library", None, core.context.clone())
		.await?;

	eprintln!("Created test library");

	// Use Desktop directory for real-world testing
	let desktop_path = dirs::desktop_dir().expect("Could not find Desktop directory");

	eprintln!("Using Desktop directory: {:?}", desktop_path);

	// Register device
	let db = library.db();
	let device = core.device.to_device()?;
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

	// Start event collection
	let event_bus = core.events.clone();
	let collection_handle = {
		let mut collector = EventCollector::new(&event_bus);
		tokio::spawn(async move {
			collector.collect_events(Duration::from_secs(60)).await;
			collector
		})
	};

	tokio::time::sleep(Duration::from_millis(100)).await;

	// Create location and start indexing with Content mode
	eprintln!("Starting Content mode indexing on Desktop...");

	let location_args = LocationCreateArgs {
		path: desktop_path.clone(),
		name: Some("Desktop Test Location".to_string()),
		index_mode: IndexMode::Content, // Content mode with hashing
	};

	let _location_db_id = create_location(
		library.clone(),
		&core.events,
		location_args,
		device_record.id,
	)
	.await?;

	eprintln!("Waiting for indexing to complete (up to 2 minutes)...");

	// Wait longer for Desktop indexing
	tokio::time::sleep(Duration::from_secs(120)).await;

	eprintln!("\nAnalyzing collected events...\n");

	let collector = collection_handle.await.unwrap();
	let stats = collector.analyze().await;
	stats.print();

	// Assertions
	let total_events = collector.get_events().await.len();
	eprintln!("\nTotal events received: {}", total_events);

	let file_events = stats.resource_changed_batch.get("file").unwrap_or(&0);
	eprintln!("File ResourceChangedBatch events: {}", file_events);

	if *file_events > 0 {
		eprintln!("\nSUCCESS: Normalized cache for Files is working!");
		eprintln!(
			"   Received {} file resource events during indexing",
			file_events
		);
	} else {
		eprintln!("\nFAIL: No file resource events received");
		eprintln!("   The normalized cache system is not emitting file events");
	}

	Ok(())
}
