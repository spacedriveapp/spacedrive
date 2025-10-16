//! Retry queue for failed sync messages
//!
//! Handles automatic retry of failed message sends with exponential backoff.

use crate::service::network::protocol::sync::messages::SyncMessage;
use chrono::{DateTime, Duration, Utc};
use std::collections::VecDeque;
use tokio::sync::RwLock;
use uuid::Uuid;

/// Maximum number of retry attempts before giving up
///
/// After this many failed attempts, messages are dropped to prevent
/// unbounded queue growth for persistently unreachable devices.
const MAX_RETRIES: u32 = 5;

/// Initial retry delay in seconds
///
/// Uses exponential backoff: 5s, 10s, 20s, 40s, 80s
const INITIAL_DELAY_SECS: i64 = 5;

/// Entry in the retry queue
#[derive(Debug, Clone)]
struct RetryEntry {
	/// Target device to send to
	target_device: Uuid,

	/// Message to send
	message: SyncMessage,

	/// Number of attempts made
	attempts: u32,

	/// Next retry time
	next_retry: DateTime<Utc>,
}

/// Retry queue for failed message sends
pub struct RetryQueue {
	queue: RwLock<VecDeque<RetryEntry>>,
}

impl RetryQueue {
	/// Create a new empty retry queue
	pub fn new() -> Self {
		Self {
			queue: RwLock::new(VecDeque::new()),
		}
	}

	/// Enqueue a message for retry
	pub async fn enqueue(&self, target_device: Uuid, message: SyncMessage) {
		let entry = RetryEntry {
			target_device,
			message,
			attempts: 0,
			next_retry: Utc::now() + Duration::seconds(INITIAL_DELAY_SECS),
		};

		self.queue.write().await.push_back(entry);
	}

	/// Get messages that are ready for retry
	///
	/// Processes entries in-place to avoid race conditions from popping/re-queuing.
	/// Atomically removes ready entries and updates remaining entries in one write lock.
	pub async fn get_ready(&self) -> Vec<(Uuid, SyncMessage)> {
		let now = Utc::now();
		let mut queue = self.queue.write().await;
		let mut ready = Vec::new();
		let mut i = 0;

		// Process entries in-place without popping the entire queue
		while i < queue.len() {
			let entry = &queue[i];

			if entry.next_retry <= now {
				// This entry is ready - remove it from queue
				let mut entry = queue.remove(i).unwrap();
				entry.attempts += 1;

				if entry.attempts >= MAX_RETRIES {
					// Max retries reached, drop it (don't re-queue)
					tracing::warn!(
						target_device = %entry.target_device,
						attempts = entry.attempts,
						"Max retries reached, dropping message"
					);
					// Don't increment i - we removed an element
					continue;
				}

				// Add to ready list
				ready.push((entry.target_device, entry.message.clone()));

				// Calculate exponential backoff delay
				let delay_secs = INITIAL_DELAY_SECS * (2_i64.pow(entry.attempts));
				entry.next_retry = now + Duration::seconds(delay_secs);

				// Re-insert at end of queue
				queue.push_back(entry);

				// Don't increment i - we removed an element
			} else {
				// Not ready yet, move to next entry
				i += 1;
			}
		}

		ready
	}

	/// Get current queue size
	pub async fn len(&self) -> usize {
		self.queue.read().await.len()
	}

	/// Check if queue is empty
	pub async fn is_empty(&self) -> bool {
		self.queue.read().await.is_empty()
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[tokio::test]
	async fn test_retry_queue() {
		let queue = RetryQueue::new();
		let device_id = Uuid::new_v4();
		let message = SyncMessage::Error {
			library_id: Uuid::new_v4(),
			message: "test".to_string(),
		};

		// Enqueue a message
		queue.enqueue(device_id, message.clone()).await;
		assert_eq!(queue.len().await, 1);

		// Should not be ready immediately
		let ready = queue.get_ready().await;
		assert_eq!(ready.len(), 0);

		// Still in queue
		assert_eq!(queue.len().await, 1);
	}
}
