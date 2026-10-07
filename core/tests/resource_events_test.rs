//! Test that verifies ResourceChanged events are emitted during indexing
//!
//! This test indexes a directory and collects all ResourceChanged events
//! to verify the normalized cache event system works end-to-end.

use sd_core::{
	infra::{
		action::LibraryAction,
		event::{Event, EventSubscriber},
	},
	ops::sources::track::{TrackSourceAction, TrackSourceInput},
	Core,
};
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

	fn events(&self) -> Arc<tokio::sync::Mutex<Vec<Event>>> {
		self.events.clone()
	}
}

impl EventStats {
	/// Analyze collected events and return statistics
	fn from_events(events: &[Event]) -> EventStats {
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
async fn test_resource_events_during_indexing(
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
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

	// Index a seeded temp directory so the test does not depend on the
	// machine having a Desktop folder or on what it contains.
	let source_dir = TempDir::new()?;
	let source_path = source_dir.path().to_path_buf();
	for i in 0..20 {
		tokio::fs::write(
			source_path.join(format!("file_{i:02}.txt")),
			format!("resource events test file {i}\n"),
		)
		.await?;
	}
	let nested = source_path.join("nested");
	tokio::fs::create_dir_all(&nested).await?;
	for i in 0..5 {
		tokio::fs::write(
			nested.join(format!("nested_{i}.md")),
			format!("# nested {i}\n"),
		)
		.await?;
	}

	eprintln!("Using source directory: {:?}", source_path);

	// Start event collection
	let mut collector = EventCollector::new(&core.events);
	let collected = collector.events();
	let collection_handle = tokio::spawn(async move {
		collector.collect_events(Duration::from_secs(60)).await;
	});

	tokio::time::sleep(Duration::from_millis(100)).await;

	// Tracking dispatches the walk, then the hashing pass behind it
	eprintln!("Tracking the source...");

	TrackSourceAction::from_input(TrackSourceInput {
		path: source_path.clone(),
		name: Some("Resource Events Test Source".to_string()),
		overrides: Default::default(),
	})?
	.execute(library.clone(), core.context.clone())
	.await?;

	// Wait until a file batch arrives instead of sleeping for a fixed time
	eprintln!("Waiting for file resource events (up to 60s)...");
	let deadline = tokio::time::Instant::now() + Duration::from_secs(60);
	loop {
		let seen_files = collected.lock().await.iter().any(|event| {
			matches!(
				event,
				Event::ResourceChangedBatch { resource_type, .. } if resource_type == "file"
			)
		});
		if seen_files || tokio::time::Instant::now() >= deadline {
			break;
		}
		tokio::time::sleep(Duration::from_millis(250)).await;
	}
	// Let the remaining batches of the walk land before analyzing
	tokio::time::sleep(Duration::from_secs(2)).await;
	collection_handle.abort();

	eprintln!("\nAnalyzing collected events...\n");

	let events = collected.lock().await.clone();
	let stats = EventStats::from_events(&events);
	stats.print();

	eprintln!("\nTotal events received: {}", events.len());

	let file_events = *stats.resource_changed_batch.get("file").unwrap_or(&0);
	eprintln!("File ResourceChangedBatch events: {}", file_events);

	assert!(
		file_events > 0,
		"indexing a tracked source should emit file ResourceChangedBatch events"
	);

	Ok(())
}
