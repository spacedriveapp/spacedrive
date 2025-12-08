//! Batch aggregator for sync events
//!
//! Aggregates batch ingestion events to reduce database writes.
//! Flushes based on time (30s), size (10k records), or explicit trigger.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use chrono::{DateTime, Utc};
use tokio::sync::RwLock;
use tracing::{debug, warn};
use uuid::Uuid;

use super::logger::SyncEventLogger;
use super::types::{EventCategory, EventSeverity, SyncEventLog, SyncEventType};

/// Key for batch aggregation
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct BatchKey {
	peer_id: Option<Uuid>,
}

/// Pending batch accumulator
#[derive(Debug, Clone)]
struct PendingBatch {
	model_counts: HashMap<String, u64>,
	total_count: u64,
	started_at: DateTime<Utc>,
	peer_id: Option<Uuid>,
}

impl PendingBatch {
	fn new(peer_id: Option<Uuid>) -> Self {
		Self {
			model_counts: HashMap::new(),
			total_count: 0,
			started_at: Utc::now(),
			peer_id,
		}
	}

	fn add(&mut self, model_type: String, count: u64) {
		*self.model_counts.entry(model_type).or_insert(0) += count;
		self.total_count += count;
	}

	fn duration_ms(&self) -> u64 {
		let duration = Utc::now().signed_duration_since(self.started_at);
		duration.num_milliseconds().max(0) as u64
	}
}

/// Batch aggregator configuration
#[derive(Debug, Clone)]
pub struct BatchAggregatorConfig {
	/// Flush after this duration
	pub flush_interval: Duration,

	/// Flush when batch reaches this size
	pub max_batch_size: usize,
}

impl Default for BatchAggregatorConfig {
	fn default() -> Self {
		Self {
			flush_interval: Duration::from_secs(30),
			max_batch_size: 10_000,
		}
	}
}

/// Batch aggregator for reducing event log writes
pub struct BatchAggregator {
	device_id: Uuid,
	pending_batches: Arc<RwLock<HashMap<BatchKey, PendingBatch>>>,
	logger: Arc<SyncEventLogger>,
	config: BatchAggregatorConfig,
}

impl BatchAggregator {
	/// Create a new batch aggregator
	pub fn new(
		device_id: Uuid,
		logger: Arc<SyncEventLogger>,
		config: BatchAggregatorConfig,
	) -> Self {
		Self {
			device_id,
			pending_batches: Arc::new(RwLock::new(HashMap::new())),
			logger,
			config,
		}
	}

	/// Add records to the batch
	pub async fn add_records(&self, model_type: String, count: u64, peer_id: Option<Uuid>) {
		let key = BatchKey { peer_id };

		let mut batches = self.pending_batches.write().await;
		let batch = batches
			.entry(key.clone())
			.or_insert_with(|| PendingBatch::new(peer_id));

		batch.add(model_type, count);

		// Check if we should flush immediately due to size
		if batch.total_count >= self.config.max_batch_size as u64 {
			let batch_to_flush = batch.clone();
			drop(batches); // Release lock before async operation

			debug!(
				total_count = batch_to_flush.total_count,
				"Flushing batch due to size limit"
			);
			self.flush_batch_internal(batch_to_flush).await;

			// Remove the flushed batch
			self.pending_batches.write().await.remove(&key);
		}
	}

	/// Explicit flush (called on state transitions or shutdown)
	pub async fn flush_all(&self) {
		let mut batches = self.pending_batches.write().await;
		let batches_to_flush: Vec<PendingBatch> = batches.drain().map(|(_, v)| v).collect();
		drop(batches);

		for batch in batches_to_flush {
			self.flush_batch_internal(batch).await;
		}
	}

	/// Internal flush implementation
	async fn flush_batch_internal(&self, batch: PendingBatch) {
		if batch.total_count == 0 {
			return;
		}

		let model_types: Vec<String> = batch.model_counts.keys().cloned().collect();
		let duration_ms = batch.duration_ms();

		// Create summary string
		let model_breakdown: Vec<String> = batch
			.model_counts
			.iter()
			.map(|(model, count)| format!("{} {}", count, model))
			.collect();

		let summary = if model_breakdown.len() <= 3 {
			format!(
				"Ingested batch of {} records ({})",
				batch.total_count,
				model_breakdown.join(", ")
			)
		} else {
			format!(
				"Ingested batch of {} records across {} model types",
				batch.total_count,
				model_breakdown.len()
			)
		};

		let event = SyncEventLog::new(self.device_id, SyncEventType::BatchIngestion, summary)
			.with_category(EventCategory::DataFlow)
			.with_severity(EventSeverity::Debug)
			.with_model_types(model_types)
			.with_record_count(batch.total_count)
			.with_duration_ms(duration_ms);

		let event = if let Some(peer_id) = batch.peer_id {
			event.with_peer(peer_id)
		} else {
			event
		};

		if let Err(e) = self.logger.log(event).await {
			warn!("Failed to flush batch event: {}", e);
		}
	}

	/// Run periodic flush task (spawned as background task)
	pub async fn run_periodic_flush(self: Arc<Self>) {
		let mut interval = tokio::time::interval(self.config.flush_interval);

		loop {
			interval.tick().await;

			// Flush batches that are old enough
			let now = Utc::now();
			let mut batches = self.pending_batches.write().await;
			let keys_to_flush: Vec<BatchKey> = batches
				.iter()
				.filter(|(_, batch)| {
					now.signed_duration_since(batch.started_at)
						>= chrono::Duration::from_std(self.config.flush_interval).unwrap()
				})
				.map(|(k, _)| k.clone())
				.collect();

			let batches_to_flush: Vec<PendingBatch> = keys_to_flush
				.iter()
				.filter_map(|k| batches.remove(k))
				.collect();

			drop(batches);

			for batch in batches_to_flush {
				self.flush_batch_internal(batch).await;
			}
		}
	}
}
