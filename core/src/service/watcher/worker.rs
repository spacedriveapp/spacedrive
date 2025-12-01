//! Per-location worker for processing file system events with batching and coalescing

use crate::context::CoreContext;
use crate::infra::event::{Event, EventBus, FsRawEventKind};
use crate::ops::indexing::responder;
use crate::service::watcher::{LocationWatcherConfig, LocationWorkerMetrics, WatcherEvent};
use anyhow::Result;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::mpsc;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

/// Per-location worker that processes events with batching and coalescing
pub struct LocationWorker {
	/// Location ID this worker handles
	location_id: Uuid,
	/// Library ID this worker belongs to
	library_id: Uuid,
	/// Receiver for incoming events
	receiver: mpsc::Receiver<WatcherEvent>,
	/// Core context for DB access
	context: Arc<CoreContext>,
	/// Event bus for emitting final events
	events: Arc<EventBus>,
	/// Worker configuration
	config: LocationWatcherConfig,
	/// Metrics for this worker
	metrics: Arc<LocationWorkerMetrics>,
	/// Indexing rule toggles for filtering events
	rule_toggles: crate::ops::indexing::rules::RuleToggles,
	/// Location root path for rule evaluation
	location_root: PathBuf,
	/// Volume backend for this location (cached)
	volume_backend: Arc<tokio::sync::RwLock<Option<Arc<dyn crate::volume::VolumeBackend>>>>,
}

impl LocationWorker {
	/// Create a new location worker
	pub fn new(
		location_id: Uuid,
		library_id: Uuid,
		receiver: mpsc::Receiver<WatcherEvent>,
		context: Arc<CoreContext>,
		events: Arc<EventBus>,
		config: LocationWatcherConfig,
		metrics: Arc<LocationWorkerMetrics>,
		rule_toggles: crate::ops::indexing::rules::RuleToggles,
		location_root: PathBuf,
	) -> Self {
		Self {
			location_id,
			library_id,
			receiver,
			context,
			events,
			config,
			metrics,
			rule_toggles,
			location_root,
			volume_backend: Arc::new(tokio::sync::RwLock::new(None)),
		}
	}

	/// Get or initialize the volume backend for this location
	async fn get_volume_backend(&self) -> Option<Arc<dyn crate::volume::VolumeBackend>> {
		// Check cache first
		{
			let backend_lock = self.volume_backend.read().await;
			if let Some(backend) = backend_lock.as_ref() {
				return Some(backend.clone());
			}
		}

		// Backend not cached, resolve it
		let backend = if let Some(mut volume) = self
			.context
			.volume_manager
			.volume_for_path(&self.location_root)
			.await
		{
			debug!(
				"Resolved volume backend for location {}: {}",
				self.location_id, volume.name
			);
			Some(self.context.volume_manager.backend_for_volume(&mut volume))
		} else {
			debug!(
				"No volume found for location {} at path {}, using local filesystem fallback",
				self.location_id,
				self.location_root.display()
			);
			None
		};

		// Cache it
		if let Some(ref backend) = backend {
			*self.volume_backend.write().await = Some(backend.clone());
		}

		backend
	}

	/// Run the worker event processing loop
	pub async fn run(mut self) -> Result<()> {
		info!("Starting location worker for location {}", self.location_id);

		while let Some(first_event) = self.receiver.recv().await {
			// Record event received
			self.metrics.record_event_processed();

			// Start batching window
			let mut batch = vec![first_event];
			let deadline = Instant::now() + Duration::from_millis(self.config.debounce_window_ms);

			// Collect events within the debounce window
			while let Ok(event) = self.receiver.try_recv() {
				batch.push(event);
				self.metrics.record_event_processed();
				if Instant::now() >= deadline || batch.len() >= self.config.max_batch_size {
					break;
				}
			}

			// Check for queue overflow and trigger focused re-index if needed
			let queue_depth = self.receiver.len();
			self.metrics.update_queue_depth(queue_depth);

			if self.config.enable_focused_reindex
				&& queue_depth > self.config.max_queue_depth_before_reindex
			{
				warn!(
					"Queue depth {} exceeds threshold {} for location {}, triggering focused re-index",
					queue_depth, self.config.max_queue_depth_before_reindex, self.location_id
				);

				// Trigger focused re-index for this location
				if let Err(e) = self.trigger_focused_reindex().await {
					error!(
						"Failed to trigger focused re-index for location {}: {}",
						self.location_id, e
					);
				}

				// Clear the current batch and continue with normal processing
				batch.clear();
				continue;
			}

			debug!(
				"Processing batch of {} events for location {}",
				batch.len(),
				self.location_id
			);

			let batch_start = Instant::now();

			// Coalesce and deduplicate events
			let coalesced = self.coalesce_events(batch)?;

			// Apply parent-first ordering
			let ordered = self.parent_first_ordering(coalesced);
			let batch_size = ordered.len();

			// Process the batch
			if let Err(e) = self.process_batch(ordered).await {
				error!(
					"Failed to process batch for location {}: {}",
					self.location_id, e
				);
			}

			// Record batch metrics
			let batch_duration = batch_start.elapsed();
			self.metrics
				.record_batch_processed(batch_size, batch_duration);
		}

		info!("Location worker for location {} stopped", self.location_id);
		Ok(())
	}

	/// Coalesce and deduplicate events within a batch
	fn coalesce_events(&self, mut events: Vec<WatcherEvent>) -> Result<Vec<WatcherEvent>> {
		// Separate Rename events (they have no primary path)
		let mut rename_events = Vec::new();
		let mut other_events = Vec::new();

		for event in events {
			if !event.should_process() {
				continue;
			}

			if matches!(
				&event.kind,
				crate::service::watcher::event_handler::WatcherEventKind::Rename { .. }
			) {
				// Rename events go directly to output (don't coalesce with other events)
				rename_events.push(event);
			} else if let Some(_path) = event.primary_path() {
				other_events.push(event);
			}
		}

		// Group non-rename events by path for deduplication
		let mut path_events: HashMap<PathBuf, Vec<WatcherEvent>> = HashMap::new();

		for event in other_events {
			if let Some(path) = event.primary_path() {
				path_events.entry(path.clone()).or_default().push(event);
			}
		}

		let mut coalesced = Vec::new();

		// Add rename events first (they should be processed before creates/modifies)
		coalesced.extend(rename_events);

		for (path, mut path_events) in path_events {
			if path_events.is_empty() {
				continue;
			}

			// Sort by timestamp to ensure proper ordering
			path_events.sort_by_key(|e| e.timestamp);

			// Apply coalescing rules
			let final_event = self.coalesce_path_events(path_events)?;
			if let Some(event) = final_event {
				coalesced.push(event);
			}
		}

		Ok(coalesced)
	}

	/// Coalesce events for a single path
	fn coalesce_path_events(&self, mut events: Vec<WatcherEvent>) -> Result<Option<WatcherEvent>> {
		if events.is_empty() {
			return Ok(None);
		}

		// If only one event, return it
		if events.len() == 1 {
			return Ok(Some(events.into_iter().next().unwrap()));
		}

		// Apply coalescing rules
		let mut creates = 0;
		let mut modifies = 0;
		let mut removes = 0;
		let mut renames = Vec::new();

		for event in &events {
			match event.kind {
				crate::service::watcher::event_handler::WatcherEventKind::Create => {
					creates += 1;
				}
				crate::service::watcher::event_handler::WatcherEventKind::Modify => {
					modifies += 1;
				}
				crate::service::watcher::event_handler::WatcherEventKind::Remove => {
					removes += 1;
				}
				crate::service::watcher::event_handler::WatcherEventKind::Rename {
					ref from,
					ref to,
				} => {
					renames.push((from.clone(), to.clone()));
				}
				_ => {} // Ignore other event types
			}
		}

		// Apply coalescing rules
		if creates > 0 && removes > 0 {
			// Create + Remove = neutralized (temp files)
			self.metrics.record_neutralized_event();
			return Ok(None);
		}

		if removes > 0 && modifies > 0 {
			// Modify after Remove = ignore modify
			modifies = 0;
		}

		// Collapse rename chains A→B, B→C → A→C
		let renames_count = renames.len();
		let final_rename = self.collapse_rename_chain(renames)?;
		if renames_count > 1 && final_rename.is_some() {
			self.metrics.record_rename_chain_collapsed();
		}

		// Determine final event
		if let Some((from, to)) = final_rename {
			Ok(Some(WatcherEvent {
				kind: crate::service::watcher::event_handler::WatcherEventKind::Rename { from, to },
				paths: vec![],
				timestamp: events.last().unwrap().timestamp,
				attrs: vec![],
			}))
		} else if removes > 0 {
			Ok(Some(WatcherEvent {
				kind: crate::service::watcher::event_handler::WatcherEventKind::Remove,
				paths: events.first().unwrap().paths.clone(),
				timestamp: events.last().unwrap().timestamp,
				attrs: vec![],
			}))
		} else if creates > 0 {
			Ok(Some(WatcherEvent {
				kind: crate::service::watcher::event_handler::WatcherEventKind::Create,
				paths: events.first().unwrap().paths.clone(),
				timestamp: events.last().unwrap().timestamp,
				attrs: vec![],
			}))
		} else if modifies > 0 {
			Ok(Some(WatcherEvent {
				kind: crate::service::watcher::event_handler::WatcherEventKind::Modify,
				paths: events.first().unwrap().paths.clone(),
				timestamp: events.last().unwrap().timestamp,
				attrs: vec![],
			}))
		} else {
			Ok(None)
		}
	}

	/// Collapse rename chains A→B, B→C → A→C
	fn collapse_rename_chain(
		&self,
		renames: Vec<(PathBuf, PathBuf)>,
	) -> Result<Option<(PathBuf, PathBuf)>> {
		if renames.is_empty() {
			return Ok(None);
		}

		if renames.len() == 1 {
			return Ok(Some(renames.into_iter().next().unwrap()));
		}

		// Build a chain by connecting renames
		let mut chain: HashMap<PathBuf, PathBuf> = renames.into_iter().collect();
		let mut final_chain: HashMap<PathBuf, PathBuf> = HashMap::new();

		for (from, to) in chain.iter() {
			let mut current_from = from.clone();
			let mut current_to = to.clone();

			// Follow the chain to find the final destination
			while let Some(next_to) = chain.get(&current_to) {
				current_to = next_to.clone();
			}

			// Only add if this is the start of a chain (not a middle link)
			if !chain.values().any(|v| v == from) {
				final_chain.insert(current_from, current_to);
			}
		}

		// Return the first (and should be only) chain
		Ok(final_chain.into_iter().next())
	}

	/// Apply parent-first ordering to events
	fn parent_first_ordering(&self, mut events: Vec<WatcherEvent>) -> Vec<WatcherEvent> {
		// Separate directory and file events
		let mut dir_events = Vec::new();
		let mut file_events = Vec::new();

		for event in events {
			if let Some(path) = event.primary_path() {
				if path.is_dir() {
					dir_events.push(event);
				} else {
					file_events.push(event);
				}
			} else {
				// Rename events might not have a primary path, check both from and to
				match &event.kind {
					crate::service::watcher::event_handler::WatcherEventKind::Rename {
						from,
						to,
					} => {
						if from.is_dir() || to.is_dir() {
							dir_events.push(event);
						} else {
							file_events.push(event);
						}
					}
					_ => file_events.push(event),
				}
			}
		}

		// Sort directory events by path depth (shallowest first)
		dir_events.sort_by_key(|e| {
			e.primary_path()
				.map(|p| p.components().count())
				.unwrap_or(0)
		});

		// Combine: directories first, then files
		let mut result = dir_events;
		result.extend(file_events);
		result
	}

	/// Process a batch of coalesced and ordered events
	async fn process_batch(&self, events: Vec<WatcherEvent>) -> Result<()> {
		if events.is_empty() {
			return Ok(());
		}

		// Convert to FsRawEventKind for the responder
		let mut raw_events = Vec::new();

		for event in events {
			let raw_kind = match event.kind {
				crate::service::watcher::event_handler::WatcherEventKind::Create => {
					if let Some(path) = event.primary_path() {
						Some(FsRawEventKind::Create { path: path.clone() })
					} else {
						None
					}
				}
				crate::service::watcher::event_handler::WatcherEventKind::Modify => {
					if let Some(path) = event.primary_path() {
						Some(FsRawEventKind::Modify { path: path.clone() })
					} else {
						None
					}
				}
				crate::service::watcher::event_handler::WatcherEventKind::Remove => {
					if let Some(path) = event.primary_path() {
						Some(FsRawEventKind::Remove { path: path.clone() })
					} else {
						None
					}
				}
				crate::service::watcher::event_handler::WatcherEventKind::Rename { from, to } => {
					debug!(
						"Worker converting Rename event: {} -> {}",
						from.display(),
						to.display()
					);
					Some(FsRawEventKind::Rename { from, to })
				}
				_ => None,
			};

			if let Some(kind) = raw_kind {
				raw_events.push(kind);
			}
		}

		// Process the batch through the responder
		debug!(
			"Worker sending {} raw events to responder for location {}",
			raw_events.len(),
			self.location_id
		);
		for event in &raw_events {
			match event {
				FsRawEventKind::Create { path } => debug!("  → Create: {}", path.display()),
				FsRawEventKind::Modify { path } => debug!("  → Modify: {}", path.display()),
				FsRawEventKind::Remove { path } => debug!("  → Remove: {}", path.display()),
				FsRawEventKind::Rename { from, to } => {
					debug!("  → Rename: {} -> {}", from.display(), to.display())
				}
			}
		}

		// Get volume backend for this location
		let volume_backend = self.get_volume_backend().await;

		if let Err(e) = responder::apply_batch(
			&self.context,
			self.library_id,
			self.location_id,
			raw_events,
			self.rule_toggles,
			&self.location_root,
			volume_backend.as_ref(),
		)
		.await
		{
			error!(
				"Failed to apply batch for location {}: {}",
				self.location_id, e
			);
		}

		Ok(())
	}

	/// Trigger a focused re-index for this location when queue overflow is detected
	async fn trigger_focused_reindex(&mut self) -> Result<()> {
		info!(
			"Triggering focused re-index for location {}",
			self.location_id
		);

		// Emit a custom event to trigger focused re-indexing
		// This would typically be handled by a job system or indexing service
		let reindex_event = Event::Custom {
			event_type: "focused_reindex".to_string(),
			data: serde_json::json!({
				"location_id": self.location_id,
				"library_id": self.library_id,
				"reason": "queue_overflow",
				"timestamp": std::time::SystemTime::now()
					.duration_since(std::time::UNIX_EPOCH)
					.unwrap_or_default()
					.as_secs()
			}),
		};

		self.events.emit(reindex_event);

		// Clear any remaining events in the queue to prevent further overflow
		let mut cleared_count = 0;
		while self.receiver.try_recv().is_ok() {
			cleared_count += 1;
		}

		if cleared_count > 0 {
			info!(
				"Cleared {} events from queue for location {} during focused re-index",
				cleared_count, self.location_id
			);
		}

		Ok(())
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use std::path::PathBuf;
	use std::time::SystemTime;

	#[test]
	fn test_coalesce_create_remove() {
		let config = LocationWatcherConfig::default();
		let worker = LocationWorker {
			location_id: Uuid::new_v4(),
			library_id: Uuid::new_v4(),
			receiver: mpsc::channel(10).1,
			context: Arc::new(create_mock_context()),
			events: Arc::new(EventBus::default()),
			config,
			metrics: Arc::new(LocationWorkerMetrics::new()),
			rule_toggles: Default::default(),
			location_root: PathBuf::from("/test"),
		};

		let events = vec![
			WatcherEvent {
				kind: crate::service::watcher::event_handler::WatcherEventKind::Create,
				paths: vec![PathBuf::from("/test/file.tmp")],
				timestamp: SystemTime::now(),
				attrs: vec![],
			},
			WatcherEvent {
				kind: crate::service::watcher::event_handler::WatcherEventKind::Remove,
				paths: vec![PathBuf::from("/test/file.tmp")],
				timestamp: SystemTime::now(),
				attrs: vec![],
			},
		];

		let result = worker.coalesce_events(events).unwrap();
		assert!(result.is_empty()); // Should be neutralized
	}

	#[test]
	fn test_coalesce_rename_chain() {
		let config = LocationWatcherConfig::default();
		let worker = LocationWorker {
			location_id: Uuid::new_v4(),
			library_id: Uuid::new_v4(),
			receiver: mpsc::channel(10).1,
			context: Arc::new(create_mock_context()),
			events: Arc::new(EventBus::default()),
			metrics: Arc::new(LocationWorkerMetrics::new()),
			config,
			rule_toggles: Default::default(),
			location_root: PathBuf::from("/test"),
		};

		let renames = vec![
			(PathBuf::from("/test/A"), PathBuf::from("/test/B")),
			(PathBuf::from("/test/B"), PathBuf::from("/test/C")),
		];

		let result = worker.collapse_rename_chain(renames).unwrap();
		assert_eq!(
			result,
			Some((PathBuf::from("/test/A"), PathBuf::from("/test/C")))
		);
	}

	fn create_mock_context() -> CoreContext {
		// This is a placeholder implementation for tests
		// In a real implementation, this would create a proper CoreContext with mocked dependencies
		unimplemented!("Mock CoreContext not yet implemented for tests")
	}
}
