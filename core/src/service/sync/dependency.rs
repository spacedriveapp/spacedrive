//! Dependency tracking for sync operations
//!
//! Tracks which sync updates are waiting for specific dependencies (parent_id, content_id, etc.)
//! and provides event-driven retry when those dependencies are resolved.
//!
//! This replaces the O(n²) "retry entire buffer" approach with O(n) targeted retries.

use super::state::BufferedUpdate;
use std::collections::HashMap;
use tokio::sync::RwLock;
use tracing::{debug, info};
use uuid::Uuid;

/// Tracks sync updates waiting for missing dependencies
///
/// Generic design: maps any UUID to updates waiting for it, regardless of type.
/// Works for:
/// - parent_id (entry → entry)
/// - content_id (entry → content_identity)
/// - location_id (entry → location)
/// - metadata_id (entry → user_metadata)
/// - Any other FK constraint
pub struct DependencyTracker {
	/// Maps dependency UUID → updates waiting for it
	waiting_for: RwLock<HashMap<Uuid, Vec<BufferedUpdate>>>,
}

impl DependencyTracker {
	/// Create a new dependency tracker
	pub fn new() -> Self {
		Self {
			waiting_for: RwLock::new(HashMap::new()),
		}
	}

	/// Add an update that's waiting for a specific dependency
	///
	/// When the update fails due to a missing FK reference, track it here.
	/// It will be retried when the dependency UUID is resolved.
	pub async fn add_dependency(&self, missing_uuid: Uuid, update: BufferedUpdate) {
		let mut waiting = self.waiting_for.write().await;
		
		waiting
			.entry(missing_uuid)
			.or_insert_with(Vec::new)
			.push(update);

		debug!(
			dependency_uuid = %missing_uuid,
			waiting_count = waiting.get(&missing_uuid).map(|v| v.len()).unwrap_or(0),
			"Added dependency tracking"
		);
	}

	/// Resolve a dependency and get all updates that were waiting for it
	///
	/// Call this when a record with the given UUID is successfully applied.
	/// Returns all updates that can now be retried.
	pub async fn resolve(&self, resolved_uuid: Uuid) -> Vec<BufferedUpdate> {
		let mut waiting = self.waiting_for.write().await;
		
		if let Some(updates) = waiting.remove(&resolved_uuid) {
			info!(
				resolved_uuid = %resolved_uuid,
				waiting_count = updates.len(),
				"Resolving dependencies - found waiting updates"
			);
			updates
		} else {
			Vec::new()
		}
	}

	/// Get statistics about pending dependencies
	pub async fn stats(&self) -> DependencyStats {
		let waiting = self.waiting_for.read().await;
		
		let total_dependencies = waiting.len();
		let total_waiting_updates: usize = waiting.values().map(|v| v.len()).sum();
		
		// Count by update type
		let mut state_changes = 0;
		let mut shared_changes = 0;
		
		for updates in waiting.values() {
			for update in updates {
				match update {
					BufferedUpdate::StateChange(_) => state_changes += 1,
					BufferedUpdate::SharedChange(_) => shared_changes += 1,
				}
			}
		}

		DependencyStats {
			total_dependencies,
			total_waiting_updates,
			waiting_state_changes: state_changes,
			waiting_shared_changes: shared_changes,
		}
	}

	/// Check if any updates are waiting
	pub async fn is_empty(&self) -> bool {
		self.waiting_for.read().await.is_empty()
	}

	/// Get count of unique dependencies being waited on
	pub async fn dependency_count(&self) -> usize {
		self.waiting_for.read().await.len()
	}

	/// Get all pending dependency UUIDs (for requesting from peers)
	///
	/// Returns a list of UUIDs that are currently blocking updates.
	/// These can be requested from peers to resolve stuck dependencies.
	pub async fn get_pending_dependency_uuids(&self) -> Vec<Uuid> {
		let waiting = self.waiting_for.read().await;
		waiting.keys().copied().collect()
	}

	/// Clear all pending dependencies (timeout/force sync fallback)
	///
	/// Call this as a last resort when dependencies cannot be resolved.
	/// Returns the number of dependencies cleared.
	pub async fn clear_all(&self) -> usize {
		let mut waiting = self.waiting_for.write().await;
		let count = waiting.len();
		waiting.clear();
		count
	}
}

impl Default for DependencyTracker {
	fn default() -> Self {
		Self::new()
	}
}

/// Statistics about pending dependencies
#[derive(Debug, Clone)]
pub struct DependencyStats {
	/// Number of unique UUIDs being waited on
	pub total_dependencies: usize,
	/// Total number of updates waiting
	pub total_waiting_updates: usize,
	/// StateChange updates waiting
	pub waiting_state_changes: usize,
	/// SharedChange updates waiting
	pub waiting_shared_changes: usize,
}

impl DependencyStats {
	pub fn is_empty(&self) -> bool {
		self.total_waiting_updates == 0
	}
}

/// Extract the missing UUID from a sync dependency error message
///
/// Parses errors like:
/// - "FK mapping failed: Sync dependency missing: parent_id -> entries (uuid=XXX)"
/// - "FK mapping failed: Sync dependency missing: content_id -> content_identities (uuid=XXX)"
pub fn extract_missing_dependency_uuid(error: &str) -> Option<Uuid> {
	// Look for pattern: "uuid=XXXXXXXX-XXXX-XXXX-XXXX-XXXXXXXXXXXX"
	if let Some(start) = error.find("uuid=") {
		let uuid_str = &error[start + 5..];
		// Take until closing paren or end of string
		let end = uuid_str.find(')').unwrap_or(uuid_str.len());
		let uuid_str = &uuid_str[..end];
		
		Uuid::parse_str(uuid_str).ok()
	} else {
		None
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::service::sync::state::StateChangeMessage;
	use chrono::Utc;

	#[test]
	fn test_extract_uuid_from_parent_id_error() {
		let error = "FK mapping failed: Sync dependency missing: parent_id -> entries (uuid=082653bb-d55b-45ea-ac0a-18205197981b): Entry with uuid=082653bb-d55b-45ea-ac0a-18205197981b not found";
		
		let uuid = extract_missing_dependency_uuid(error);
		assert!(uuid.is_some());
		assert_eq!(
			uuid.unwrap().to_string(),
			"082653bb-d55b-45ea-ac0a-18205197981b"
		);
	}

	#[test]
	fn test_extract_uuid_from_content_id_error() {
		let error = "FK mapping failed: Sync dependency missing: content_id -> content_identities (uuid=d9d3a478-ecf6-4cb4-8711-ddf776037dbb)";
		
		let uuid = extract_missing_dependency_uuid(error);
		assert!(uuid.is_some());
		assert_eq!(
			uuid.unwrap().to_string(),
			"d9d3a478-ecf6-4cb4-8711-ddf776037dbb"
		);
	}

	#[test]
	fn test_extract_uuid_from_location_id_error() {
		let error = "FK mapping failed: Sync dependency missing: location_id -> locations (uuid=a1b2c3d4-e5f6-7890-abcd-ef1234567890)";
		
		let uuid = extract_missing_dependency_uuid(error);
		assert!(uuid.is_some());
		assert_eq!(
			uuid.unwrap().to_string(),
			"a1b2c3d4-e5f6-7890-abcd-ef1234567890"
		);
	}

	#[test]
	fn test_extract_uuid_no_match() {
		let error = "Some other error";
		assert!(extract_missing_dependency_uuid(error).is_none());
	}

	#[tokio::test]
	async fn test_dependency_tracker_basic() {
		let tracker = DependencyTracker::new();
		
		let parent_uuid = Uuid::new_v4();
		let child_change = StateChangeMessage {
			model_type: "entry".to_string(),
			record_uuid: Uuid::new_v4(),
			device_id: Uuid::new_v4(),
			data: serde_json::json!({}),
			timestamp: Utc::now(),
		};

		// Add dependency
		tracker
			.add_dependency(parent_uuid, BufferedUpdate::StateChange(child_change.clone()))
			.await;

		let stats = tracker.stats().await;
		assert_eq!(stats.total_dependencies, 1);
		assert_eq!(stats.total_waiting_updates, 1);
		assert_eq!(stats.waiting_state_changes, 1);

		// Resolve dependency
		let resolved = tracker.resolve(parent_uuid).await;
		assert_eq!(resolved.len(), 1);

		// Should be empty now
		assert!(tracker.is_empty().await);
	}

	#[tokio::test]
	async fn test_multiple_children_same_parent() {
		let tracker = DependencyTracker::new();
		
		let parent_uuid = Uuid::new_v4();
		
		// Three children waiting for same parent
		for _ in 0..3 {
			let child = StateChangeMessage {
				model_type: "entry".to_string(),
				record_uuid: Uuid::new_v4(),
				device_id: Uuid::new_v4(),
				data: serde_json::json!({}),
				timestamp: Utc::now(),
			};
			tracker
				.add_dependency(parent_uuid, BufferedUpdate::StateChange(child))
				.await;
		}

		let stats = tracker.stats().await;
		assert_eq!(stats.total_dependencies, 1); // One unique parent
		assert_eq!(stats.total_waiting_updates, 3); // Three children waiting

		// Resolve - should get all 3 children
		let resolved = tracker.resolve(parent_uuid).await;
		assert_eq!(resolved.len(), 3);
	}

	#[tokio::test]
	async fn test_different_parents() {
		let tracker = DependencyTracker::new();
		
		let parent1 = Uuid::new_v4();
		let parent2 = Uuid::new_v4();
		
		// Child waiting for parent1
		tracker
			.add_dependency(
				parent1,
				BufferedUpdate::StateChange(StateChangeMessage {
					model_type: "entry".to_string(),
					record_uuid: Uuid::new_v4(),
					device_id: Uuid::new_v4(),
					data: serde_json::json!({}),
					timestamp: Utc::now(),
				}),
			)
			.await;

		// Child waiting for parent2
		tracker
			.add_dependency(
				parent2,
				BufferedUpdate::StateChange(StateChangeMessage {
					model_type: "entry".to_string(),
					record_uuid: Uuid::new_v4(),
					device_id: Uuid::new_v4(),
					data: serde_json::json!({}),
					timestamp: Utc::now(),
				}),
			)
			.await;

		assert_eq!(tracker.dependency_count().await, 2);

		// Resolve parent1 - should only get its child
		let resolved = tracker.resolve(parent1).await;
		assert_eq!(resolved.len(), 1);

		// Parent2 still has a child waiting
		assert_eq!(tracker.dependency_count().await, 1);
	}
}

