//! Time-series storage for sync metrics history

use crate::service::sync::metrics::snapshot::SyncMetricsSnapshot;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Ring buffer for storing historical metrics snapshots
#[derive(Debug)]
pub struct MetricsHistory {
	/// Ring buffer of snapshots
	snapshots: Arc<RwLock<VecDeque<SyncMetricsSnapshot>>>,

	/// Maximum number of snapshots to keep
	max_size: usize,
}

impl MetricsHistory {
	/// Create a new metrics history with specified capacity
	pub fn new(max_size: usize) -> Self {
		Self {
			snapshots: Arc::new(RwLock::new(VecDeque::with_capacity(max_size))),
			max_size,
		}
	}

	/// Add a new snapshot to the history
	pub async fn add_snapshot(&self, snapshot: SyncMetricsSnapshot) {
		let mut snapshots = self.snapshots.write().await;

		// Add new snapshot
		snapshots.push_back(snapshot);

		// Trim to max size
		while snapshots.len() > self.max_size {
			snapshots.pop_front();
		}
	}

	/// Get all snapshots
	pub async fn get_all_snapshots(&self) -> Vec<SyncMetricsSnapshot> {
		let snapshots = self.snapshots.read().await;
		snapshots.clone().into()
	}

	/// Get snapshots since a specific time
	pub async fn get_snapshots_since(&self, since: DateTime<Utc>) -> Vec<SyncMetricsSnapshot> {
		let snapshots = self.snapshots.read().await;
		snapshots
			.iter()
			.filter(|snapshot| snapshot.timestamp >= since)
			.cloned()
			.collect()
	}

	/// Get snapshots in a time range
	pub async fn get_snapshots_range(
		&self,
		start: DateTime<Utc>,
		end: DateTime<Utc>,
	) -> Vec<SyncMetricsSnapshot> {
		let snapshots = self.snapshots.read().await;
		snapshots
			.iter()
			.filter(|snapshot| snapshot.timestamp >= start && snapshot.timestamp <= end)
			.cloned()
			.collect()
	}

	/// Get the latest snapshot
	pub async fn get_latest_snapshot(&self) -> Option<SyncMetricsSnapshot> {
		let snapshots = self.snapshots.read().await;
		snapshots.back().cloned()
	}

	/// Get the oldest snapshot
	pub async fn get_oldest_snapshot(&self) -> Option<SyncMetricsSnapshot> {
		let snapshots = self.snapshots.read().await;
		snapshots.front().cloned()
	}

	/// Clear all snapshots
	pub async fn clear(&self) {
		let mut snapshots = self.snapshots.write().await;
		snapshots.clear();
	}

	/// Get the number of stored snapshots
	pub async fn len(&self) -> usize {
		let snapshots = self.snapshots.read().await;
		snapshots.len()
	}

	/// Check if history is empty
	pub async fn is_empty(&self) -> bool {
		let snapshots = self.snapshots.read().await;
		snapshots.is_empty()
	}

	/// Get capacity
	pub fn capacity(&self) -> usize {
		self.max_size
	}
}

impl Default for MetricsHistory {
	fn default() -> Self {
		Self::new(1000) // Default to 1000 snapshots
	}
}

/// Configuration for metrics history
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricsHistoryConfig {
	/// Maximum number of snapshots to keep in memory
	pub max_snapshots: usize,

	/// How often to take snapshots (in seconds)
	pub snapshot_interval_secs: u64,

	/// Whether to persist snapshots to database
	pub persist_to_db: bool,

	/// How often to persist to database (in seconds)
	pub persist_interval_secs: u64,
}

impl Default for MetricsHistoryConfig {
	fn default() -> Self {
		Self {
			max_snapshots: 1000,
			snapshot_interval_secs: 60, // Every minute
			persist_to_db: false,
			persist_interval_secs: 300, // Every 5 minutes
		}
	}
}
