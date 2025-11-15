//! Dedicated event bus for sync operations
//!
//! This module provides a separate event bus specifically for sync coordination,
//! isolated from the general EventBus to prevent sync starvation from high-volume
//! events like filesystem changes or job progress updates.

use crate::infra::sync::SharedChangeEntry;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;
use tracing::{debug, error};
use uuid::Uuid;

/// Event bus specifically for sync coordination
///
/// Separate from general EventBus to prevent starvation of critical sync events
/// by high-volume events (filesystem watcher, job progress, etc).
///
/// Key differences from general EventBus:
/// - Larger capacity (10k vs 1k) - sync is critical
/// - Only sync events (no filtering needed)
/// - Typed events (no generic Event enum overhead)
/// - Lag warnings are CRITICAL alerts
#[derive(Debug, Clone)]
pub struct SyncEventBus {
	sender: broadcast::Sender<SyncEvent>,
}

/// Sync-specific events (not mixed with UI/job/volume events)
///
/// These events coordinate sync operations between TransactionManager and PeerSync.
/// They represent changes that need to be broadcast to remote peers.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SyncEvent {
	/// Device-owned state change (locations, entries, volumes, audit logs)
	///
	/// Simple state broadcast without log-based conflict resolution.
	/// Each device owns its data and broadcasts updates to peers.
	StateChange {
		library_id: Uuid,
		model_type: String,
		record_uuid: Uuid,
		device_id: Uuid,
		data: serde_json::Value,
		timestamp: DateTime<Utc>,
	},

	/// Shared resource change (tags, albums, user_metadata)
	///
	/// Uses HLC-ordered log for conflict resolution.
	/// All devices can modify shared resources, HLC determines order.
	SharedChange {
		library_id: Uuid,
		entry: SharedChangeEntry,
	},

	/// Sync metrics update (for observability, non-critical)
	///
	/// Periodic metrics snapshots for monitoring sync health.
	/// Can be dropped if the bus is under load.
	MetricsUpdated {
		library_id: Uuid,
		metrics: serde_json::Value,
	},
}

impl SyncEvent {
	/// Get the library ID for this event
	pub fn library_id(&self) -> Uuid {
		match self {
			SyncEvent::StateChange { library_id, .. } => *library_id,
			SyncEvent::SharedChange { library_id, .. } => *library_id,
			SyncEvent::MetricsUpdated { library_id, .. } => *library_id,
		}
	}

	/// Get a human-readable event type name
	pub fn event_type(&self) -> &str {
		match self {
			SyncEvent::StateChange { .. } => "StateChange",
			SyncEvent::SharedChange { .. } => "SharedChange",
			SyncEvent::MetricsUpdated { .. } => "MetricsUpdated",
		}
	}

	/// Check if this is a critical event (should never be dropped)
	pub fn is_critical(&self) -> bool {
		match self {
			SyncEvent::StateChange { .. } | SyncEvent::SharedChange { .. } => true,
			SyncEvent::MetricsUpdated { .. } => false,
		}
	}
}

impl SyncEventBus {
	/// Create a new sync event bus with large capacity
	///
	/// Capacity is 10,000 events - sync is critical and should rarely lag.
	/// If lag occurs with this capacity, it indicates extreme system load or a bug.
	pub fn new() -> Self {
		let (sender, _) = broadcast::channel(10_000);
		debug!("Created sync event bus with capacity 10,000");
		Self { sender }
	}

	/// Create a sync event bus with custom capacity (for testing)
	#[cfg(test)]
	pub fn new_with_capacity(capacity: usize) -> Self {
		let (sender, _) = broadcast::channel(capacity);
		debug!("Created sync event bus with capacity {}", capacity);
		Self { sender }
	}

	/// Emit a sync event to all subscribers
	///
	/// Returns the number of active subscribers that received the event.
	pub fn emit(&self, event: SyncEvent) -> usize {
		// Extract metadata before moving event
		let event_type = event.event_type().to_string();
		let library_id = event.library_id();
		let is_critical = event.is_critical();

		match self.sender.send(event) {
			Ok(count) => {
				debug!(
					event_type = %event_type,
					library_id = %library_id,
					subscribers = count,
					critical = is_critical,
					"Sync event emitted"
				);
				count
			}
			Err(_) => {
				// No subscribers - this is unusual but not necessarily an error
				// (could happen during shutdown or before PeerSync is initialized)
				debug!(
					event_type = %event_type,
					library_id = %library_id,
					"Sync event emitted but no subscribers"
				);
				0
			}
		}
	}

	/// Subscribe to sync events
	///
	/// Returns a receiver that will get all future sync events.
	/// Multiple subscribers can exist (e.g., multiple PeerSync instances for different libraries).
	pub fn subscribe(&self) -> broadcast::Receiver<SyncEvent> {
		self.sender.subscribe()
	}

	/// Get the number of active subscribers
	pub fn subscriber_count(&self) -> usize {
		self.sender.receiver_count()
	}
}

impl Default for SyncEventBus {
	fn default() -> Self {
		Self::new()
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::infra::sync::{ChangeType, HLC};

	#[test]
	fn test_sync_event_bus_creation() {
		let bus = SyncEventBus::new();
		assert_eq!(bus.subscriber_count(), 0);
	}

	#[test]
	fn test_emit_with_no_subscribers() {
		let bus = SyncEventBus::new();

		let event = SyncEvent::StateChange {
			library_id: Uuid::new_v4(),
			model_type: "entry".to_string(),
			record_uuid: Uuid::new_v4(),
			device_id: Uuid::new_v4(),
			data: serde_json::json!({}),
			timestamp: Utc::now(),
		};

		let count = bus.emit(event);
		assert_eq!(count, 0);
	}

	#[tokio::test]
	async fn test_emit_with_subscribers() {
		let bus = SyncEventBus::new();
		let mut sub1 = bus.subscribe();
		let mut sub2 = bus.subscribe();

		let library_id = Uuid::new_v4();
		let event = SyncEvent::StateChange {
			library_id,
			model_type: "entry".to_string(),
			record_uuid: Uuid::new_v4(),
			device_id: Uuid::new_v4(),
			data: serde_json::json!({"name": "test.txt"}),
			timestamp: Utc::now(),
		};

		let count = bus.emit(event.clone());
		assert_eq!(count, 2);

		// Both subscribers should receive the event
		let received1 = sub1.recv().await.unwrap();
		let received2 = sub2.recv().await.unwrap();

		assert_eq!(received1.library_id(), library_id);
		assert_eq!(received2.library_id(), library_id);
	}

	#[tokio::test]
	async fn test_shared_change_event() {
		let bus = SyncEventBus::new();
		let mut subscriber = bus.subscribe();

		let library_id = Uuid::new_v4();
		let entry = SharedChangeEntry {
			hlc: HLC::new(1, 0, Uuid::new_v4()),
			model_type: "tag".to_string(),
			record_uuid: Uuid::new_v4(),
			change_type: ChangeType::Upsert,
			data: serde_json::json!({"name": "important"}),
		};

		let event = SyncEvent::SharedChange {
			library_id,
			entry: entry.clone(),
		};

		bus.emit(event);

		let received = subscriber.recv().await.unwrap();
		match received {
			SyncEvent::SharedChange {
				library_id: recv_lib_id,
				entry: recv_entry,
			} => {
				assert_eq!(recv_lib_id, library_id);
				assert_eq!(recv_entry.hlc, entry.hlc);
				assert_eq!(recv_entry.model_type, "tag");
			}
			_ => panic!("Expected SharedChange event"),
		}
	}

	#[test]
	fn test_event_criticality() {
		let state_event = SyncEvent::StateChange {
			library_id: Uuid::new_v4(),
			model_type: "entry".to_string(),
			record_uuid: Uuid::new_v4(),
			device_id: Uuid::new_v4(),
			data: serde_json::json!({}),
			timestamp: Utc::now(),
		};
		assert!(state_event.is_critical());

		let shared_event = SyncEvent::SharedChange {
			library_id: Uuid::new_v4(),
			entry: SharedChangeEntry {
				hlc: HLC::new(1, 0, Uuid::new_v4()),
				model_type: "tag".to_string(),
				record_uuid: Uuid::new_v4(),
				change_type: ChangeType::Upsert,
				data: serde_json::json!({}),
			},
		};
		assert!(shared_event.is_critical());

		let metrics_event = SyncEvent::MetricsUpdated {
			library_id: Uuid::new_v4(),
			metrics: serde_json::json!({}),
		};
		assert!(!metrics_event.is_critical());
	}

	#[tokio::test]
	async fn test_no_lag_with_large_capacity() {
		let bus = SyncEventBus::new();
		let mut subscriber = bus.subscribe();

		// Emit 9000 events (below 10k capacity)
		for i in 0..9000 {
			let event = SyncEvent::StateChange {
				library_id: Uuid::new_v4(),
				model_type: "entry".to_string(),
				record_uuid: Uuid::new_v4(),
				device_id: Uuid::new_v4(),
				data: serde_json::json!({"index": i}),
				timestamp: Utc::now(),
			};
			bus.emit(event);
		}

		// Should receive first event without lag
		let result = subscriber.recv().await;
		assert!(result.is_ok());
	}

	#[tokio::test]
	async fn test_lag_with_small_capacity() {
		// Create bus with tiny capacity to test lag behavior
		let bus = SyncEventBus::new_with_capacity(10);
		let mut subscriber = bus.subscribe();

		// Emit way more events than capacity
		for i in 0..100 {
			let event = SyncEvent::StateChange {
				library_id: Uuid::new_v4(),
				model_type: "entry".to_string(),
				record_uuid: Uuid::new_v4(),
				device_id: Uuid::new_v4(),
				data: serde_json::json!({"index": i}),
				timestamp: Utc::now(),
			};
			bus.emit(event);
		}

		// Should get lag error
		let result = subscriber.recv().await;
		match result {
			Err(broadcast::error::RecvError::Lagged(skipped)) => {
				assert!(skipped > 0);
			}
			_ => panic!("Expected lag error, got {:?}", result),
		}
	}
}
