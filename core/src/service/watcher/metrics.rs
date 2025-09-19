//! Metrics and observability for the location watcher

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tracing::{debug, info};

/// Metrics for a single location worker
#[derive(Debug, Default)]
pub struct LocationWorkerMetrics {
	/// Number of events processed
	pub events_processed: AtomicU64,
	/// Number of events coalesced (suppressed)
	pub events_coalesced: AtomicU64,
	/// Number of batches processed
	pub batches_processed: AtomicU64,
	/// Current queue depth
	pub queue_depth: AtomicU64,
	/// Maximum queue depth reached
	pub max_queue_depth: AtomicU64,
	/// Number of rename chains collapsed
	pub rename_chains_collapsed: AtomicU64,
	/// Number of create+remove neutralizations
	pub neutralized_events: AtomicU64,
	/// Average batch size
	pub total_batch_size: AtomicU64,
	/// Last batch processing time
	pub last_batch_duration: AtomicU64,
	/// Maximum batch processing time
	pub max_batch_duration: AtomicU64,
}

impl LocationWorkerMetrics {
	/// Create new metrics
	pub fn new() -> Self {
		Self::default()
	}

	/// Record an event processed
	pub fn record_event_processed(&self) {
		self.events_processed.fetch_add(1, Ordering::Relaxed);
	}

	/// Record an event coalesced
	pub fn record_event_coalesced(&self) {
		self.events_coalesced.fetch_add(1, Ordering::Relaxed);
	}

	/// Record a batch processed
	pub fn record_batch_processed(&self, batch_size: usize, duration: Duration) {
		self.batches_processed.fetch_add(1, Ordering::Relaxed);
		self.total_batch_size
			.fetch_add(batch_size as u64, Ordering::Relaxed);

		let duration_ms = duration.as_millis() as u64;
		self.last_batch_duration
			.store(duration_ms, Ordering::Relaxed);

		// Update max duration
		let mut current_max = self.max_batch_duration.load(Ordering::Relaxed);
		while duration_ms > current_max {
			match self.max_batch_duration.compare_exchange_weak(
				current_max,
				duration_ms,
				Ordering::Relaxed,
				Ordering::Relaxed,
			) {
				Ok(_) => break,
				Err(val) => current_max = val,
			}
		}
	}

	/// Update queue depth
	pub fn update_queue_depth(&self, depth: usize) {
		let depth_u64 = depth as u64;
		self.queue_depth.store(depth_u64, Ordering::Relaxed);

		// Update max depth
		let mut current_max = self.max_queue_depth.load(Ordering::Relaxed);
		while depth_u64 > current_max {
			match self.max_queue_depth.compare_exchange_weak(
				current_max,
				depth_u64,
				Ordering::Relaxed,
				Ordering::Relaxed,
			) {
				Ok(_) => break,
				Err(val) => current_max = val,
			}
		}
	}

	/// Record a rename chain collapsed
	pub fn record_rename_chain_collapsed(&self) {
		self.rename_chains_collapsed.fetch_add(1, Ordering::Relaxed);
	}

	/// Record a neutralized event (create+remove)
	pub fn record_neutralized_event(&self) {
		self.neutralized_events.fetch_add(1, Ordering::Relaxed);
	}

	/// Get average batch size
	pub fn get_average_batch_size(&self) -> f64 {
		let total_batches = self.batches_processed.load(Ordering::Relaxed);
		if total_batches == 0 {
			0.0
		} else {
			let total_size = self.total_batch_size.load(Ordering::Relaxed);
			total_size as f64 / total_batches as f64
		}
	}

	/// Get coalescing rate (percentage of events that were coalesced)
	pub fn get_coalescing_rate(&self) -> f64 {
		let processed = self.events_processed.load(Ordering::Relaxed);
		let coalesced = self.events_coalesced.load(Ordering::Relaxed);

		if processed == 0 {
			0.0
		} else {
			(coalesced as f64 / processed as f64) * 100.0
		}
	}

	/// Log current metrics
	pub fn log_metrics(&self, location_id: uuid::Uuid) {
		info!(
			"Location {} metrics: processed={}, coalesced={}, batches={}, avg_batch_size={:.2}, coalescing_rate={:.2}%, max_queue_depth={}, max_batch_duration={}ms",
			location_id,
			self.events_processed.load(Ordering::Relaxed),
			self.events_coalesced.load(Ordering::Relaxed),
			self.batches_processed.load(Ordering::Relaxed),
			self.get_average_batch_size(),
			self.get_coalescing_rate(),
			self.max_queue_depth.load(Ordering::Relaxed),
			self.max_batch_duration.load(Ordering::Relaxed)
		);
	}
}

/// Global watcher metrics
#[derive(Debug, Default)]
pub struct WatcherMetrics {
	/// Total locations being watched
	pub total_locations: AtomicU64,
	/// Total events received from filesystem
	pub total_events_received: AtomicU64,
	/// Total workers created
	pub total_workers_created: AtomicU64,
	/// Total workers destroyed
	pub total_workers_destroyed: AtomicU64,
}

impl WatcherMetrics {
	/// Create new metrics
	pub fn new() -> Self {
		Self::default()
	}

	/// Record an event received
	pub fn record_event_received(&self) {
		self.total_events_received.fetch_add(1, Ordering::Relaxed);
	}

	/// Record a worker created
	pub fn record_worker_created(&self) {
		self.total_workers_created.fetch_add(1, Ordering::Relaxed);
	}

	/// Record a worker destroyed
	pub fn record_worker_destroyed(&self) {
		self.total_workers_destroyed.fetch_add(1, Ordering::Relaxed);
	}

	/// Update total locations count
	pub fn update_total_locations(&self, count: usize) {
		self.total_locations.store(count as u64, Ordering::Relaxed);
	}

	/// Get event processing rate (events per second)
	pub fn get_processing_rate(&self) -> f64 {
		let received = self.total_events_received.load(Ordering::Relaxed);
		// This would need to be calculated with timestamps in a real implementation
		// For now, return 0 as a placeholder
		0.0
	}

	/// Log current metrics
	pub fn log_metrics(&self) {
		info!(
			"Watcher metrics: locations={}, events_received={}, workers_created={}, workers_destroyed={}",
			self.total_locations.load(Ordering::Relaxed),
			self.total_events_received.load(Ordering::Relaxed),
			self.total_workers_created.load(Ordering::Relaxed),
			self.total_workers_destroyed.load(Ordering::Relaxed)
		);
	}
}

/// Metrics collector that periodically logs metrics
#[derive(Clone)]
pub struct MetricsCollector {
	watcher_metrics: Arc<WatcherMetrics>,
	worker_metrics: Arc<Mutex<std::collections::HashMap<uuid::Uuid, Arc<LocationWorkerMetrics>>>>,
	log_interval: Duration,
}

impl MetricsCollector {
	/// Create a new metrics collector
	pub fn new(watcher_metrics: Arc<WatcherMetrics>, log_interval: Duration) -> Self {
		Self {
			watcher_metrics,
			worker_metrics: Arc::new(Mutex::new(std::collections::HashMap::new())),
			log_interval,
		}
	}

	/// Add a worker's metrics
	pub fn add_worker_metrics(&self, location_id: uuid::Uuid, metrics: Arc<LocationWorkerMetrics>) {
		if let Ok(mut worker_metrics) = self.worker_metrics.lock() {
			worker_metrics.insert(location_id, metrics);
		}
	}

	/// Remove a worker's metrics
	pub fn remove_worker_metrics(&self, location_id: &uuid::Uuid) {
		if let Ok(mut worker_metrics) = self.worker_metrics.lock() {
			worker_metrics.remove(location_id);
		}
	}

	/// Start the metrics collection loop
	pub async fn start_collection(&self) {
		let mut interval = tokio::time::interval(self.log_interval);

		loop {
			interval.tick().await;

			// Log watcher metrics
			self.watcher_metrics.log_metrics();

			// Log worker metrics
			if let Ok(worker_metrics) = self.worker_metrics.lock() {
				for (location_id, metrics) in worker_metrics.iter() {
					metrics.log_metrics(*location_id);
				}
			}
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use std::time::Duration;

	#[test]
	fn test_location_worker_metrics() {
		let metrics = LocationWorkerMetrics::new();

		metrics.record_event_processed();
		metrics.record_event_coalesced();
		metrics.record_batch_processed(10, Duration::from_millis(50));

		assert_eq!(metrics.events_processed.load(Ordering::Relaxed), 1);
		assert_eq!(metrics.events_coalesced.load(Ordering::Relaxed), 1);
		assert_eq!(metrics.batches_processed.load(Ordering::Relaxed), 1);
		assert_eq!(metrics.get_average_batch_size(), 10.0);
	}

	#[test]
	fn test_coalescing_rate() {
		let metrics = LocationWorkerMetrics::new();

		// Process 10 events, coalesce 3
		for _ in 0..10 {
			metrics.record_event_processed();
		}
		for _ in 0..3 {
			metrics.record_event_coalesced();
		}

		assert_eq!(metrics.get_coalescing_rate(), 30.0);
	}
}
