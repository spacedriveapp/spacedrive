//! Shared event collector for tests
//!
//! Provides utilities for collecting and analyzing events from the event bus

use sd_core::infra::event::{Event, EventBus, EventSubscriber};
use std::{collections::HashMap, path::Path, sync::Arc};
use tokio::time::Duration;

/// Event collector that tracks all events during tests
pub struct EventCollector {
	events: Arc<tokio::sync::Mutex<Vec<Event>>>,
	subscriber: EventSubscriber,
	capture_data: bool,
}

impl EventCollector {
	/// Create a new event collector subscribed to the given event bus
	///
	/// By default, only collects events for statistics. Use `with_capture()` to also
	/// capture full event data for debugging.
	pub fn new(event_bus: &Arc<EventBus>) -> Self {
		Self {
			events: Arc::new(tokio::sync::Mutex::new(Vec::new())),
			subscriber: event_bus.subscribe(),
			capture_data: false,
		}
	}

	/// Create a new event collector that captures full event data for debugging
	pub fn with_capture(event_bus: &Arc<EventBus>) -> Self {
		Self {
			events: Arc::new(tokio::sync::Mutex::new(Vec::new())),
			subscriber: event_bus.subscribe(),
			capture_data: true,
		}
	}

	/// Collect events for the specified duration
	pub async fn collect_events(&mut self, duration: Duration) {
		let events = self.events.clone();

		let timeout_result = tokio::time::timeout(duration, async {
			loop {
				match self.subscriber.recv().await {
					Ok(event) => {
						// Filter out library statistics noise
						let should_include = match &event {
							Event::ResourceChanged { resource_type, .. } => {
								resource_type != "library"
							}
							Event::LibraryStatisticsUpdated { .. } => false,
							_ => true,
						};

						if should_include {
							events.lock().await.push(event);
						}
					}
					Err(_) => break,
				}
			}
		})
		.await;

		if timeout_result.is_err() {
			tracing::debug!("Event collection timed out after {:?}", duration);
		}
	}

	/// Get all collected events
	pub async fn get_events(&self) -> Vec<Event> {
		self.events.lock().await.clone()
	}

	/// Analyze collected events and return statistics
	pub async fn analyze(&self) -> EventStats {
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
					..
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
				Event::EntryCreated { .. } => {
					stats.entries_created += 1;
				}
				Event::EntryModified { .. } => {
					stats.entries_modified += 1;
				}
				Event::EntryDeleted { .. } => {
					stats.entries_deleted += 1;
				}
				Event::EntryMoved { .. } => {
					stats.entries_moved += 1;
				}
				_ => {}
			}
		}

		stats
	}

	/// Write collected events to a JSON file
	pub async fn write_to_file(&self, path: &Path) -> anyhow::Result<()> {
		let events = self.events.lock().await;
		let json = serde_json::to_string_pretty(&*events)?;
		tokio::fs::write(path, json).await?;
		Ok(())
	}

	/// Print all collected events to stderr for debugging
	pub async fn print_events(&self) {
		let events = self.events.lock().await;
		eprintln!("\n=== Collected Events ({}) ===", events.len());
		for (i, event) in events.iter().enumerate() {
			eprintln!("\n[{}] {}", i + 1, event.variant_name());
			match event {
				Event::ResourceChanged {
					resource_type,
					resource,
					metadata,
				} => {
					eprintln!("  Type: {}", resource_type);
					eprintln!(
						"  Resource: {}",
						serde_json::to_string_pretty(resource).unwrap_or_default()
					);
					if let Some(meta) = metadata {
						eprintln!("  Paths: {} affected", meta.affected_paths.len());
					}
				}
				Event::ResourceChangedBatch {
					resource_type,
					resources,
					metadata,
				} => {
					let count = resources.as_array().map(|a| a.len()).unwrap_or(0);
					eprintln!("  Type: {}", resource_type);
					eprintln!("  Resources: {} items", count);
					eprintln!(
						"  Resources JSON:\n{}",
						serde_json::to_string_pretty(resources).unwrap_or_default()
					);
					if let Some(meta) = metadata {
						eprintln!("  Paths: {} affected", meta.affected_paths.len());
					}
				}
				Event::IndexingStarted { location_id } => {
					eprintln!("  Location: {}", location_id);
				}
				Event::IndexingCompleted {
					location_id,
					total_files,
					total_dirs,
				} => {
					eprintln!("  Location: {}", location_id);
					eprintln!("  Files: {}, Dirs: {}", total_files, total_dirs);
				}
				Event::JobStarted {
					job_id, job_type, ..
				} => {
					eprintln!("  Job: {} ({})", job_type, job_id);
				}
				Event::JobCompleted {
					job_id,
					job_type,
					output,
					..
				} => {
					eprintln!("  Job: {} ({})", job_type, job_id);
					eprintln!("  Output: {:?}", output);
				}
				Event::EntryCreated {
					library_id,
					entry_id,
				} => {
					eprintln!("  Library: {}", library_id);
					eprintln!("  Entry: {}", entry_id);
				}
				Event::EntryModified {
					library_id,
					entry_id,
				} => {
					eprintln!("  Library: {}", library_id);
					eprintln!("  Entry: {}", entry_id);
				}
				_ => {
					eprintln!("  {:?}", event);
				}
			}
		}
		eprintln!("\n=== End Events ===\n");
	}

	/// Get events filtered by type
	pub async fn get_events_by_type(&self, event_type: &str) -> Vec<Event> {
		let events = self.events.lock().await;
		events
			.iter()
			.filter(|e| e.variant_name() == event_type)
			.cloned()
			.collect()
	}

	/// Get ResourceChangedBatch events for a specific resource type
	pub async fn get_resource_batch_events(&self, resource_type: &str) -> Vec<Event> {
		let events = self.events.lock().await;
		events
			.iter()
			.filter(
				|e| matches!(e, Event::ResourceChangedBatch { resource_type: rt, .. } if rt == resource_type),
			)
			.cloned()
			.collect()
	}
}

/// Statistics about collected events
#[derive(Debug, Default)]
pub struct EventStats {
	pub resource_changed: HashMap<String, usize>,
	pub resource_changed_batch: HashMap<String, usize>,
	pub indexing_started: usize,
	pub indexing_completed: usize,
	pub jobs_started: HashMap<String, usize>,
	pub jobs_completed: HashMap<String, usize>,
	pub entries_created: usize,
	pub entries_modified: usize,
	pub entries_deleted: usize,
	pub entries_moved: usize,
}

impl EventStats {
	/// Print formatted statistics to stderr
	pub fn print(&self) {
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

		eprintln!("\nEntry events:");
		eprintln!("  Created: {}", self.entries_created);
		eprintln!("  Modified: {}", self.entries_modified);
		eprintln!("  Deleted: {}", self.entries_deleted);
		eprintln!("  Moved: {}", self.entries_moved);

		eprintln!("\nJob events:");
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
