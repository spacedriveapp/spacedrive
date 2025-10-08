//! Leader sync handler
//!
//! Handles leader-side sync: listening for commits and pushing notifications to followers.

use crate::infra::event::{Event, EventBus, EventSubscriber};
use crate::infra::sync::{SyncLogDb, TransactionManager};
use crate::library::Library;
use anyhow::Result;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
use tokio::time;
use tracing::{debug, info, warn};
use uuid::Uuid;

/// Batch notification state
struct NotificationBatch {
	from_sequence: u64,
	to_sequence: u64,
	entry_count: usize,
	last_update: tokio::time::Instant,
}

/// Leader sync handler
///
/// Subscribes to commit events and pushes NewEntries notifications to followers.
pub struct LeaderSync {
	library_id: Uuid,
	sync_log_db: Arc<SyncLogDb>,
	event_subscriber: Mutex<EventSubscriber>,
	pending_batches: Arc<Mutex<HashMap<Uuid, NotificationBatch>>>,
}

impl LeaderSync {
	/// Create a new leader sync handler
	pub async fn new_with_deps(
		library_id: Uuid,
		sync_log_db: Arc<SyncLogDb>,
		event_bus: Arc<EventBus>,
		_db: Arc<crate::infra::db::Database>,
	) -> Result<Self> {
		info!(library_id = %library_id, "Creating leader sync handler");

		// Subscribe to events
		let event_subscriber = event_bus.subscribe();

		Ok(Self {
			library_id,
			sync_log_db,
			event_subscriber: Mutex::new(event_subscriber),
			pending_batches: Arc::new(Mutex::new(HashMap::new())),
		})
	}

	/// Run the leader sync loop
	///
	/// Listens for commit events and pushes notifications to followers.
	pub async fn run(&self) {
		info!(library_id = %self.library_id, "Starting leader sync loop");

		// Spawn batch notifier task (debounces rapid commits)
		let pending_batches = self.pending_batches.clone();
		let library_id = self.library_id;
		tokio::spawn(async move {
			Self::batch_notifier_loop(library_id, pending_batches).await;
		});

		// Main event loop
		let mut event_subscriber = self.event_subscriber.lock().await;
		loop {
			match event_subscriber.recv().await {
				Ok(event) => {
					self.handle_event(event).await;
				}
				Err(e) => {
					warn!(
						library_id = %self.library_id,
						error = %e,
						"Error receiving event, continuing..."
					);
					tokio::time::sleep(Duration::from_millis(100)).await;
				}
			}
		}
	}

	/// Handle an event (check if it's a sync commit)
	async fn handle_event(&self, event: Event) {
		// Check for Custom events that indicate sync commits
		if let Event::Custom { event_type, data } = event {
			// TransactionManager emits events like "location_insert", "tag_update", etc.
			if event_type.ends_with("_insert")
				|| event_type.ends_with("_update")
				|| event_type.ends_with("_delete")
			{
				// Extract sequence from event data
				if let Some(sequence) = data.get("sequence").and_then(|v| v.as_u64()) {
					self.queue_notification(sequence).await;
				}
			}
		}
	}

	/// Queue a notification for batching
	async fn queue_notification(&self, sequence: u64) {
		let mut batches = self.pending_batches.lock().await;
		let batch = batches.entry(self.library_id).or_insert(NotificationBatch {
			from_sequence: sequence,
			to_sequence: sequence,
			entry_count: 1,
			last_update: tokio::time::Instant::now(),
		});

		// Extend batch
		if sequence < batch.from_sequence {
			batch.from_sequence = sequence;
		}
		if sequence > batch.to_sequence {
			batch.to_sequence = sequence;
		}
		batch.entry_count += 1;
		batch.last_update = tokio::time::Instant::now();

		debug!(
			library_id = %self.library_id,
			sequence = sequence,
			batch_size = batch.entry_count,
			"Queued notification for batching"
		);
	}

	/// Batch notifier loop (runs every 100ms)
	///
	/// Debounces rapid commits into single notifications.
	async fn batch_notifier_loop(
		library_id: Uuid,
		pending_batches: Arc<Mutex<HashMap<Uuid, NotificationBatch>>>,
	) {
		let mut interval = time::interval(Duration::from_millis(100));

		loop {
			interval.tick().await;

			let mut batches = pending_batches.lock().await;
			if let Some(batch) = batches.remove(&library_id) {
				// Only send if batch has been stable for 100ms
				if batch.last_update.elapsed() >= Duration::from_millis(100) {
					info!(
						library_id = %library_id,
						from_seq = batch.from_sequence,
						to_seq = batch.to_sequence,
						count = batch.entry_count,
						"Sending batched notification to followers"
					);

					// TODO: Send via SyncProtocolHandler when networking integration is complete
					// protocol_handler.notify_followers(batch.from_sequence, batch.to_sequence).await;
				} else {
					// Put it back if not ready
					batches.insert(library_id, batch);
				}
			}
		}
	}
}
