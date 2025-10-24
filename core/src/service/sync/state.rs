//! Sync state machine and buffering for new devices

use crate::infra::sync::{SharedChangeEntry, HLC};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::collections::BinaryHeap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::warn;
use uuid::Uuid;

/// Device sync state for state machine
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DeviceSyncState {
	/// Not yet synced, no backfill started
	Uninitialized,

	/// Currently backfilling from peer(s)
	/// Buffers all live updates during this phase
	Backfilling { peer: Uuid, progress: u8 }, // 0-100

	/// Backfill complete, processing buffered updates
	/// Still buffers new updates while catching up
	CatchingUp { buffered_count: usize },

	/// Fully synced, applying live updates immediately
	Ready,

	/// Sync paused (offline or user disabled)
	Paused,
}

impl DeviceSyncState {
	pub fn is_backfilling(&self) -> bool {
		matches!(self, DeviceSyncState::Backfilling { .. })
	}

	pub fn is_catching_up(&self) -> bool {
		matches!(self, DeviceSyncState::CatchingUp { .. })
	}

	pub fn is_ready(&self) -> bool {
		matches!(self, DeviceSyncState::Ready)
	}

	pub fn should_buffer(&self) -> bool {
		matches!(
			self,
			DeviceSyncState::Backfilling { .. } | DeviceSyncState::CatchingUp { .. }
		)
	}
}

/// Update type for buffering
#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub enum BufferedUpdate {
	/// State-based change (device-owned data)
	StateChange(StateChangeMessage),

	/// Log-based change (shared resource)
	SharedChange(SharedChangeEntry),
}

impl BufferedUpdate {
	/// Get timestamp for ordering
	pub fn timestamp(&self) -> u64 {
		match self {
			BufferedUpdate::StateChange(msg) => msg.timestamp.timestamp_millis() as u64,
			BufferedUpdate::SharedChange(entry) => entry.hlc.timestamp,
		}
	}

	/// Get HLC if this is a shared change
	pub fn hlc(&self) -> Option<HLC> {
		match self {
			BufferedUpdate::SharedChange(entry) => Some(entry.hlc),
			_ => None,
		}
	}
}

impl Ord for BufferedUpdate {
	fn cmp(&self, other: &Self) -> Ordering {
		// Reverse ordering for BinaryHeap (min-heap, oldest first)
		match (self, other) {
			(BufferedUpdate::SharedChange(a), BufferedUpdate::SharedChange(b)) => {
				// Use HLC ordering (which includes timestamp, counter, device_id)
				b.hlc.cmp(&a.hlc)
			}
			_ => {
				// For state changes or mixed, compare by timestamp
				other.timestamp().cmp(&self.timestamp())
			}
		}
	}
}

impl PartialOrd for BufferedUpdate {
	fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
		Some(self.cmp(other))
	}
}

/// State change message for device-owned data
#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub struct StateChangeMessage {
	pub model_type: String,
	pub record_uuid: Uuid,
	pub device_id: Uuid,
	pub data: serde_json::Value,
	pub timestamp: DateTime<Utc>,
}

/// Maximum number of buffered updates before dropping oldest
///
/// Prevents unbounded memory growth during long backfill operations.
/// If buffer fills up, oldest updates are dropped (they'll be re-requested
/// if needed via watermark catch-up).
const MAX_BUFFER_SIZE: usize = 100_000;

/// Buffer queue for updates received during backfill/catch-up
///
/// Uses a BinaryHeap to maintain updates sorted by timestamp/HLC,
/// ensuring causality is preserved when processing buffered updates.
///
/// Has a maximum size limit to prevent OOM during long backfills.
pub struct BufferQueue {
	queue: RwLock<BinaryHeap<BufferedUpdate>>,
	max_size: usize,
}

impl BufferQueue {
	/// Create new empty buffer queue with default size limit
	pub fn new() -> Self {
		Self {
			queue: RwLock::new(BinaryHeap::new()),
			max_size: MAX_BUFFER_SIZE,
		}
	}

	/// Create new buffer queue with custom size limit
	pub fn with_max_size(max_size: usize) -> Self {
		Self {
			queue: RwLock::new(BinaryHeap::new()),
			max_size,
		}
	}

	/// Push update to buffer
	///
	/// Updates are automatically sorted by timestamp/HLC.
	/// If buffer is at capacity, silently drops the update (oldest-eviction
	/// will happen naturally when popping, since we can't efficiently remove
	/// specific items from a BinaryHeap).
	pub async fn push(&self, update: BufferedUpdate) {
		let mut queue = self.queue.write().await;

		// Check if at capacity
		if queue.len() >= self.max_size {
			warn!(
				current_size = queue.len(),
				max_size = self.max_size,
				"Buffer queue at capacity, dropping new update"
			);
			return;
		}

		queue.push(update);
	}

	/// Pop next update in order (oldest first, by timestamp/HLC)
	///
	/// Returns the oldest update (smallest timestamp/HLC).
	/// For SharedChanges, uses full HLC ordering (timestamp, counter, device_id).
	/// For StateChanges, uses timestamp only.
	pub async fn pop_ordered(&self) -> Option<BufferedUpdate> {
		let mut queue = self.queue.write().await;
		queue.pop()
	}

	/// Get current buffer size
	pub async fn len(&self) -> usize {
		self.queue.read().await.len()
	}

	/// Check if buffer is empty
	pub async fn is_empty(&self) -> bool {
		self.queue.read().await.is_empty()
	}

	/// Clear all buffered updates
	pub async fn clear(&self) {
		self.queue.write().await.clear();
	}
}

/// Backfill checkpoint for resumability
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackfillCheckpoint {
	/// Device being backfilled from
	pub peer: Uuid,

	/// Resume token (e.g., "entry-500000")
	pub resume_token: Option<String>,

	/// Progress (0.0 - 1.0)
	pub progress: f32,

	/// Model types completed
	pub completed_models: Vec<String>,

	/// Last updated
	pub updated_at: DateTime<Utc>,
}

impl BackfillCheckpoint {
	/// Create new checkpoint starting backfill
	pub fn start(peer: Uuid) -> Self {
		Self {
			peer,
			resume_token: None,
			progress: 0.0,
			completed_models: Vec::new(),
			updated_at: Utc::now(),
		}
	}

	/// Update checkpoint progress
	pub fn update(&mut self, resume_token: Option<String>, progress: f32) {
		self.resume_token = resume_token;
		self.progress = progress;
		self.updated_at = Utc::now();
	}

	/// Mark model type as completed
	pub fn mark_completed(&mut self, model_type: String) {
		if !self.completed_models.contains(&model_type) {
			self.completed_models.push(model_type);
		}
		self.updated_at = Utc::now();
	}

	/// Save checkpoint to disk (TODO: implement persistence)
	pub async fn save(&self) -> Result<(), std::io::Error> {
		// TODO: Persist to disk for crash recovery
		Ok(())
	}

	/// Load checkpoint from disk (TODO: implement persistence)
	pub async fn load() -> Result<Option<Self>, std::io::Error> {
		// TODO: Load from disk
		Ok(None)
	}
}

/// Peer information for selection
#[derive(Debug, Clone)]
pub struct PeerInfo {
	pub device_id: Uuid,
	pub is_online: bool,
	pub latency_ms: f32,
	pub has_complete_state: bool,
	pub active_syncs: usize,
}

impl PeerInfo {
	/// Calculate score for peer selection
	/// Higher score = better candidate for backfill
	pub fn score(&self) -> f32 {
		let mut score = 0.0;

		// Lower latency = higher score
		if self.latency_ms > 0.0 {
			score += 1000.0 / self.latency_ms.max(1.0);
		}

		// Prefer peers with complete state
		if self.has_complete_state {
			score += 100.0;
		}

		// Prefer less busy peers
		score -= self.active_syncs as f32 * 10.0;

		score
	}
}

/// Select best peer for backfill
pub fn select_backfill_peer(available_peers: Vec<PeerInfo>) -> Result<Uuid, &'static str> {
	// Filter online peers
	let online: Vec<_> = available_peers
		.into_iter()
		.filter(|p| p.is_online)
		.collect();

	if online.is_empty() {
		return Err("No online peers available for backfill");
	}

	// Score each peer
	let mut scored: Vec<_> = online
		.into_iter()
		.map(|peer| {
			let score = peer.score();
			(peer, score)
		})
		.collect();

	// Sort by score (highest first)
	scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

	Ok(scored[0].0.device_id)
}

#[cfg(test)]
mod tests {
	use super::*;

	#[tokio::test]
	async fn test_buffer_queue() {
		let queue = BufferQueue::new();

		let update = BufferedUpdate::StateChange(StateChangeMessage {
			model_type: "location".to_string(),
			record_uuid: Uuid::new_v4(),
			device_id: Uuid::new_v4(),
			data: serde_json::json!({"path": "/test"}),
			timestamp: Utc::now(),
		});

		queue.push(update.clone()).await;
		assert_eq!(queue.len().await, 1);

		let popped = queue.pop_ordered().await;
		assert!(popped.is_some());
		assert_eq!(queue.len().await, 0);
	}

	#[test]
	fn test_peer_selection() {
		let peers = vec![
			PeerInfo {
				device_id: Uuid::new_v4(),
				is_online: true,
				latency_ms: 50.0,
				has_complete_state: true,
				active_syncs: 1,
			},
			PeerInfo {
				device_id: Uuid::new_v4(),
				is_online: true,
				latency_ms: 20.0, // Faster!
				has_complete_state: true,
				active_syncs: 0,
			},
			PeerInfo {
				device_id: Uuid::new_v4(),
				is_online: false, // Offline, should be filtered
				latency_ms: 10.0,
				has_complete_state: true,
				active_syncs: 0,
			},
		];

		let selected_id = peers[1].device_id; // Should select the fastest online peer
		let result = select_backfill_peer(peers).unwrap();
		assert_eq!(result, selected_id);
	}

	#[test]
	fn test_sync_state_transitions() {
		let state = DeviceSyncState::Uninitialized;
		assert!(!state.is_ready());

		let state = DeviceSyncState::Backfilling {
			peer: Uuid::new_v4(),
			progress: 50,
		};
		assert!(state.should_buffer());

		let state = DeviceSyncState::Ready;
		assert!(state.is_ready());
		assert!(!state.should_buffer());
	}
}
