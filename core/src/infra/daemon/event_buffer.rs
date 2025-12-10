use std::collections::VecDeque;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

use crate::infra::daemon::types::EventFilter;
use crate::infra::event::Event;

/// A buffered event with timestamp for time-based eviction
#[derive(Debug, Clone)]
struct BufferedEvent {
	event: Arc<Event>,
	timestamp: Instant,
}

/// Thread-safe event buffer with time-based eviction
///
/// Buffers recent events to handle subscription race conditions where events
/// are emitted before subscriptions are created. When a new subscription is
/// created, buffered events matching the subscription filter are replayed.
pub struct EventBuffer {
	events: Arc<RwLock<VecDeque<BufferedEvent>>>,
	retention_duration: Duration,
	max_size: usize,
}

impl EventBuffer {
	/// Create a new event buffer with default settings
	///
	/// - Retention: 5 seconds
	/// - Max size: 100 events
	pub fn new() -> Self {
		Self {
			events: Arc::new(RwLock::new(VecDeque::with_capacity(100))),
			retention_duration: Duration::from_secs(5),
			max_size: 100,
		}
	}

	/// Add an event to the buffer
	///
	/// Events are wrapped in Arc to avoid expensive clones when replaying
	/// to multiple subscriptions. If the buffer exceeds max_size, the oldest
	/// events are evicted (FIFO).
	pub async fn add_event(&self, event: Event) {
		let mut events = self.events.write().await;

		events.push_back(BufferedEvent {
			event: Arc::new(event),
			timestamp: Instant::now(),
		});

		// Enforce max size (evict oldest)
		while events.len() > self.max_size {
			events.pop_front();
		}
	}

	/// Get buffered events that match the subscription filter
	///
	/// Returns Arc<Event> to avoid cloning large event payloads.
	/// Events are returned in chronological order (oldest first).
	pub async fn get_matching_events(
		&self,
		event_types: &[String],
		filter: &Option<EventFilter>,
	) -> Vec<Arc<Event>> {
		let events = self.events.read().await;

		events
			.iter()
			.filter_map(|buffered| {
				if Self::matches_filter(&buffered.event, event_types, filter) {
					Some(Arc::clone(&buffered.event))
				} else {
					None
				}
			})
			.collect()
	}

	/// Remove events older than retention_duration
	///
	/// Should be called periodically (e.g., every 1 second) to prevent
	/// unbounded memory growth.
	pub async fn cleanup_expired(&self) {
		let mut events = self.events.write().await;
		let now = Instant::now();

		events.retain(|buffered| now.duration_since(buffered.timestamp) < self.retention_duration);
	}

	/// Check if an event matches the subscription filter
	///
	/// This is a copy of RpcServer::should_forward_event to avoid module coupling.
	/// The logic must stay in sync with the main filtering implementation.
	fn matches_filter(event: &Event, event_types: &[String], filter: &Option<EventFilter>) -> bool {
		// If event_types is empty, forward all events
		// Otherwise, treat event_types as an INCLUSION list (only forward these)
		if !event_types.is_empty() {
			let event_type = event.variant_name();

			if !event_types.contains(&event_type.to_string()) {
				return false;
			}
		}

		// Apply additional filters if specified
		if let Some(filter) = filter {
			// Filter by resource type
			if let Some(filter_resource_type) = &filter.resource_type {
				if let Some(event_resource_type) = event.resource_type() {
					if event_resource_type != filter_resource_type {
						return false;
					}
				} else {
					// Event is not a resource event, but filter expects one
					return false;
				}
			}

			// Filter by path scope (for resource events)
			if let Some(path_scope) = &filter.path_scope {
				let include_descendants = filter.include_descendants.unwrap_or(false);
				let affects = event.affects_path(path_scope, include_descendants);

				if !affects {
					return false;
				}
			}

			// Filter by job_id
			match event {
				Event::JobProgress { job_id, .. }
				| Event::JobStarted { job_id, .. }
				| Event::JobCompleted { job_id, .. }
				| Event::JobFailed { job_id, .. }
				| Event::JobCancelled { job_id, .. } => {
					if let Some(filter_job_id) = &filter.job_id {
						return job_id == filter_job_id;
					}
				}
				Event::LibraryCreated { id, .. }
				| Event::LibraryOpened { id, .. }
				| Event::LibraryClosed { id, .. } => {
					if let Some(filter_library_id) = &filter.library_id {
						return id == filter_library_id;
					}
				}
				_ => {}
			}
		}

		true
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::infra::event::Event;

	#[tokio::test]
	async fn test_buffer_size_limit() {
		let buffer = EventBuffer::new();

		// Add 150 events (exceeds max_size of 100)
		for i in 0..150 {
			buffer.add_event(Event::CoreStarted).await;
		}

		// Verify only last 100 events are kept
		let events = buffer.events.read().await;
		assert_eq!(events.len(), 100);
	}

	#[tokio::test]
	async fn test_time_based_cleanup() {
		let buffer = EventBuffer {
			events: Arc::new(RwLock::new(VecDeque::new())),
			retention_duration: Duration::from_millis(100),
			max_size: 100,
		};

		// Add event
		buffer.add_event(Event::CoreStarted).await;

		// Verify event exists
		{
			let events = buffer.events.read().await;
			assert_eq!(events.len(), 1);
		}

		// Wait for expiration
		tokio::time::sleep(Duration::from_millis(150)).await;

		// Cleanup
		buffer.cleanup_expired().await;

		// Verify event was removed
		let events = buffer.events.read().await;
		assert_eq!(events.len(), 0);
	}

	#[tokio::test]
	async fn test_empty_filter_matches_all() {
		let buffer = EventBuffer::new();

		buffer.add_event(Event::CoreStarted).await;
		buffer.add_event(Event::CoreShutdown).await;

		// Empty filter should match all events
		let matching = buffer.get_matching_events(&[], &None).await;
		assert_eq!(matching.len(), 2);
	}

	#[tokio::test]
	async fn test_event_type_filtering() {
		let buffer = EventBuffer::new();

		buffer.add_event(Event::CoreStarted).await;
		buffer.add_event(Event::CoreShutdown).await;

		// Filter for only CoreStarted
		let matching = buffer
			.get_matching_events(&["CoreStarted".to_string()], &None)
			.await;

		assert_eq!(matching.len(), 1);
		assert!(matches!(&*matching[0], Event::CoreStarted));
	}

	#[tokio::test]
	async fn test_arc_cloning_is_cheap() {
		let buffer = EventBuffer::new();

		buffer.add_event(Event::CoreStarted).await;

		// Get matching events multiple times (simulating multiple subscriptions)
		let match1 = buffer.get_matching_events(&[], &None).await;
		let match2 = buffer.get_matching_events(&[], &None).await;

		// Both should point to same underlying event (Arc cloning)
		assert_eq!(Arc::strong_count(&match1[0]), 3); // buffer + match1 + match2
	}
}
