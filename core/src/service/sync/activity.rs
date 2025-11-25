//! Sync Activity Aggregator
//!
//! Monitors sync metrics and emits activity events for real-time UI updates.
//! Calculates deltas between metric snapshots to generate meaningful activity events.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use chrono::Utc;
use tokio::sync::RwLock;
use tracing::{debug, warn};
use uuid::Uuid;

use crate::infra::event::{Event, EventBus, SyncActivityType};
use crate::service::sync::metrics::{snapshot::SyncMetricsSnapshot, SyncMetricsCollector};
use crate::service::sync::state::DeviceSyncState;

/// Aggregates sync metrics into activity events for the frontend
pub struct SyncActivityAggregator {
	library_id: Uuid,
	metrics: Arc<SyncMetricsCollector>,
	event_bus: Arc<EventBus>,
	last_snapshot: RwLock<Option<SyncMetricsSnapshot>>,
	aggregation_interval: Duration,
}

impl SyncActivityAggregator {
	pub fn new(
		library_id: Uuid,
		metrics: Arc<SyncMetricsCollector>,
		event_bus: Arc<EventBus>,
	) -> Self {
		Self {
			library_id,
			metrics,
			event_bus,
			last_snapshot: RwLock::new(None),
			aggregation_interval: Duration::from_secs(1),
		}
	}

	/// Run the aggregator loop (spawned as background task)
	pub async fn run(self: Arc<Self>) {
		let mut interval = tokio::time::interval(self.aggregation_interval);

		loop {
			interval.tick().await;

			let current = SyncMetricsSnapshot::from_metrics(self.metrics.metrics()).await;
			let previous = self.last_snapshot.read().await.clone();

			if let Some(prev) = previous {
				self.emit_activity_events(&current, &prev).await;
			}

			*self.last_snapshot.write().await = Some(current);
		}
	}

	async fn emit_activity_events(&self, current: &SyncMetricsSnapshot, previous: &SyncMetricsSnapshot) {
		// State changes
		if current.state.current_state != previous.state.current_state {
			self.event_bus.emit(Event::SyncStateChanged {
				library_id: self.library_id,
				previous_state: format!("{:?}", previous.state.current_state),
				new_state: format!("{:?}", current.state.current_state),
				timestamp: Utc::now().to_rfc3339(),
			});
		}

		// Per-peer activity deltas
		self.emit_peer_activity(current, previous).await;

		// Connection changes
		self.emit_connection_changes(current, previous).await;

		// Recent errors
		self.emit_recent_errors(current, previous).await;
	}

	async fn emit_peer_activity(&self, current: &SyncMetricsSnapshot, previous: &SyncMetricsSnapshot) {
		for (peer_id, peer_metrics) in &current.data_volume.entries_by_device {
			let prev_peer = previous.data_volume.entries_by_device.get(peer_id);

			// Calculate deltas
			let delta_received = peer_metrics.entries_received
				- prev_peer.map(|p| p.entries_received).unwrap_or(0);

			if delta_received > 0 {
				self.event_bus.emit(Event::SyncActivity {
					library_id: self.library_id,
					peer_device_id: *peer_id,
					activity_type: SyncActivityType::ChangesReceived {
						changes: delta_received,
					},
					model_type: None,
					count: delta_received,
					timestamp: Utc::now().to_rfc3339(),
				});
			}
		}

		// Broadcast activity (aggregate across all peers)
		let delta_broadcasts = current.operations.broadcasts_sent
			- previous.operations.broadcasts_sent;

		if delta_broadcasts > 0 {
			// Use first online peer or just emit without specific peer
			let first_online_peer = current
				.data_volume
				.entries_by_device
				.iter()
				.find(|(_, p)| p.is_online)
				.map(|(id, _)| *id);

			if let Some(peer_id) = first_online_peer {
				self.event_bus.emit(Event::SyncActivity {
					library_id: self.library_id,
					peer_device_id: peer_id,
					activity_type: SyncActivityType::BroadcastSent {
						changes: delta_broadcasts,
					},
					model_type: None,
					count: delta_broadcasts,
					timestamp: Utc::now().to_rfc3339(),
				});
			}
		}

		// Applied changes
		let delta_applied = current.operations.changes_applied - previous.operations.changes_applied;

		if delta_applied > 0 {
			let first_online_peer = current
				.data_volume
				.entries_by_device
				.iter()
				.find(|(_, p)| p.is_online)
				.map(|(id, _)| *id);

			if let Some(peer_id) = first_online_peer {
				self.event_bus.emit(Event::SyncActivity {
					library_id: self.library_id,
					peer_device_id: peer_id,
					activity_type: SyncActivityType::ChangesApplied {
						changes: delta_applied,
					},
					model_type: None,
					count: delta_applied,
					timestamp: Utc::now().to_rfc3339(),
				});
			}
		}

		// Backfill events (state-based detection)
		match (
			&previous.state.current_state,
			&current.state.current_state,
		) {
			(DeviceSyncState::Ready, DeviceSyncState::Backfilling { .. }) => {
				if let Some(peer_id) = current
					.data_volume
					.entries_by_device
					.iter()
					.find(|(_, p)| p.is_online)
					.map(|(id, _)| *id)
				{
					self.event_bus.emit(Event::SyncActivity {
						library_id: self.library_id,
						peer_device_id: peer_id,
						activity_type: SyncActivityType::BackfillStarted,
						model_type: None,
						count: 0,
						timestamp: Utc::now().to_rfc3339(),
					});
				}
			}
			(DeviceSyncState::Backfilling { .. }, DeviceSyncState::Ready)
			| (DeviceSyncState::Backfilling { .. }, DeviceSyncState::CatchingUp { .. }) => {
				// Backfill completed
				let total_entries: u64 = current
					.data_volume
					.entries_by_device
					.values()
					.map(|p| p.entries_received)
					.sum();

				if let Some(peer_id) = current
					.data_volume
					.entries_by_device
					.iter()
					.find(|(_, p)| p.is_online)
					.map(|(id, _)| *id)
				{
					self.event_bus.emit(Event::SyncActivity {
						library_id: self.library_id,
						peer_device_id: peer_id,
						activity_type: SyncActivityType::BackfillCompleted {
							records: total_entries,
						},
						model_type: None,
						count: total_entries,
						timestamp: Utc::now().to_rfc3339(),
					});
				}
			}
			(DeviceSyncState::Ready, DeviceSyncState::CatchingUp { .. }) => {
				if let Some(peer_id) = current
					.data_volume
					.entries_by_device
					.iter()
					.find(|(_, p)| p.is_online)
					.map(|(id, _)| *id)
				{
					self.event_bus.emit(Event::SyncActivity {
						library_id: self.library_id,
						peer_device_id: peer_id,
						activity_type: SyncActivityType::CatchUpStarted,
						model_type: None,
						count: 0,
						timestamp: Utc::now().to_rfc3339(),
					});
				}
			}
			(DeviceSyncState::CatchingUp { .. }, DeviceSyncState::Ready) => {
				if let Some(peer_id) = current
					.data_volume
					.entries_by_device
					.iter()
					.find(|(_, p)| p.is_online)
					.map(|(id, _)| *id)
				{
					self.event_bus.emit(Event::SyncActivity {
						library_id: self.library_id,
						peer_device_id: peer_id,
						activity_type: SyncActivityType::CatchUpCompleted,
						model_type: None,
						count: 0,
						timestamp: Utc::now().to_rfc3339(),
					});
				}
			}
			_ => {}
		}
	}

	async fn emit_connection_changes(&self, current: &SyncMetricsSnapshot, previous: &SyncMetricsSnapshot) {
		for (peer_id, peer_metrics) in &current.data_volume.entries_by_device {
			let prev_online = previous
				.data_volume
				.entries_by_device
				.get(peer_id)
				.map(|p| p.is_online)
				.unwrap_or(false);

			if peer_metrics.is_online != prev_online {
				self.event_bus.emit(Event::SyncConnectionChanged {
					library_id: self.library_id,
					peer_device_id: *peer_id,
					peer_name: peer_metrics.device_name.clone(),
					connected: peer_metrics.is_online,
					timestamp: Utc::now().to_rfc3339(),
				});
			}
		}
	}

	async fn emit_recent_errors(&self, current: &SyncMetricsSnapshot, previous: &SyncMetricsSnapshot) {
		// Only emit errors that are new since last snapshot
		let new_errors = current
			.errors
			.recent_errors
			.iter()
			.filter(|e| {
				// Check if this error occurred after the previous snapshot's state timestamp
				e.timestamp > previous.state.state_entered_at
			})
			.take(5); // Limit to 5 errors per interval to avoid spam

		for error in new_errors {
			self.event_bus.emit(Event::SyncError {
				library_id: self.library_id,
				peer_device_id: error.device_id,
				error_type: error.error_type.clone(),
				message: error.message.clone(),
				timestamp: error.timestamp.to_rfc3339(),
			});
		}
	}
}
