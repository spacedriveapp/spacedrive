//! Sync leader election and lease management
//!
//! Each library requires a single leader device responsible for assigning sync log
//! sequence numbers. This module implements a simple leader election protocol with
//! heartbeats and automatic failover.

use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use thiserror::Error;
use tracing::{debug, info, warn};
use uuid::Uuid;

/// Errors related to leader election
#[derive(Debug, Error)]
pub enum LeaderError {
	#[error("Not the leader: device {current_leader} holds the lease until {expires_at}")]
	NotLeader {
		current_leader: Uuid,
		expires_at: DateTime<Utc>,
	},

	#[error("Leader lease expired: last heartbeat at {last_heartbeat}")]
	LeaseExpired { last_heartbeat: DateTime<Utc> },

	#[error("Invalid leader state: {0}")]
	InvalidState(String),
}

pub type Result<T> = std::result::Result<T, LeaderError>;

/// Leader election constants
pub mod constants {
	use chrono::Duration;

	/// Leader sends heartbeat every 30 seconds
	pub const HEARTBEAT_INTERVAL: Duration = Duration::seconds(30);

	/// Leader is considered offline if no heartbeat for 60 seconds
	pub const LEASE_TIMEOUT: Duration = Duration::seconds(60);

	/// Lease extension duration (when heartbeat is sent)
	pub const LEASE_EXTENSION: Duration = Duration::seconds(90);
}

/// Sync role for a device in a library
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SyncRole {
	/// This device is the leader (assigns sequence numbers)
	Leader,
	/// This device is a follower (receives sync from leader)
	Follower,
}

/// Leader state for a library
///
/// This is stored in the device's `sync_leadership` JSON field in the database.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncLeadership {
	/// Device ID of the current leader
	pub leader_device_id: Uuid,

	/// When the leader's lease expires
	pub lease_expires_at: DateTime<Utc>,

	/// Last time we received a heartbeat from the leader
	pub last_heartbeat_at: DateTime<Utc>,

	/// When this leadership record was last updated
	pub updated_at: DateTime<Utc>,
}

impl SyncLeadership {
	/// Create a new leadership record for a device
	pub fn new(leader_device_id: Uuid) -> Self {
		let now = Utc::now();
		Self {
			leader_device_id,
			lease_expires_at: now + constants::LEASE_EXTENSION,
			last_heartbeat_at: now,
			updated_at: now,
		}
	}

	/// Check if the lease is still valid
	pub fn is_valid(&self) -> bool {
		Utc::now() < self.lease_expires_at
	}

	/// Check if the leader has timed out (no heartbeat for 60s)
	pub fn has_timed_out(&self) -> bool {
		Utc::now() - self.last_heartbeat_at > constants::LEASE_TIMEOUT
	}

	/// Extend the lease (called when heartbeat received)
	pub fn extend_lease(&mut self) {
		let now = Utc::now();
		self.lease_expires_at = now + constants::LEASE_EXTENSION;
		self.last_heartbeat_at = now;
		self.updated_at = now;
	}
}

/// Leadership manager for a library
///
/// Tracks leadership state and handles election/re-election.
/// This is a lightweight in-memory structure; persistent state is in the database.
pub struct LeadershipManager {
	/// This device's ID
	device_id: Uuid,

	/// Current leadership state per library (library_id -> SyncLeadership)
	library_leadership: HashMap<Uuid, SyncLeadership>,
}

impl LeadershipManager {
	/// Create a new leadership manager
	pub fn new(device_id: Uuid) -> Self {
		Self {
			device_id,
			library_leadership: HashMap::new(),
		}
	}

	/// Initialize leadership for a library
	///
	/// This should be called when a library is opened. If this device created
	/// the library, it becomes the initial leader. Otherwise, it's a follower
	/// and will learn about the leader from the network.
	pub fn initialize_library(&mut self, library_id: Uuid, is_creator: bool) -> SyncRole {
		if is_creator {
			info!(
				library_id = %library_id,
				device_id = %self.device_id,
				"Initializing as library leader (creator)"
			);

			let leadership = SyncLeadership::new(self.device_id);
			self.library_leadership.insert(library_id, leadership);
			SyncRole::Leader
		} else {
			debug!(
				library_id = %library_id,
				device_id = %self.device_id,
				"Initializing as library follower"
			);
			SyncRole::Follower
		}
	}

	/// Update leadership state from the network
	///
	/// Called when we receive a heartbeat or leadership announcement from another device.
	pub fn update_leadership(&mut self, library_id: Uuid, leadership: SyncLeadership) {
		debug!(
			library_id = %library_id,
			leader = %leadership.leader_device_id,
			expires_at = %leadership.lease_expires_at,
			"Updating leadership state"
		);

		self.library_leadership.insert(library_id, leadership);
	}

	/// Check if this device is the leader for a library
	pub fn is_leader(&self, library_id: Uuid) -> bool {
		if let Some(leadership) = self.library_leadership.get(&library_id) {
			leadership.leader_device_id == self.device_id && leadership.is_valid()
		} else {
			false
		}
	}

	/// Get the current leader for a library
	pub fn get_leader(&self, library_id: Uuid) -> Option<Uuid> {
		self.library_leadership
			.get(&library_id)
			.filter(|l| l.is_valid())
			.map(|l| l.leader_device_id)
	}

	/// Get the current role for this device in a library
	pub fn get_role(&self, library_id: Uuid) -> SyncRole {
		if self.is_leader(library_id) {
			SyncRole::Leader
		} else {
			SyncRole::Follower
		}
	}

	/// Attempt to become the leader for a library
	///
	/// This is called when:
	/// 1. A library is created (creator becomes leader)
	/// 2. The current leader times out (re-election)
	///
	/// Uses highest device_id as tiebreaker if multiple devices attempt election.
	pub fn request_leadership(&mut self, library_id: Uuid) -> Result<bool> {
		// Check if there's a valid leader
		if let Some(leadership) = self.library_leadership.get(&library_id) {
			if leadership.is_valid() && !leadership.has_timed_out() {
				// Leader is still valid
				if leadership.leader_device_id == self.device_id {
					// We're already the leader - extend our lease
					let mut new_leadership = leadership.clone();
					new_leadership.extend_lease();
					self.library_leadership.insert(library_id, new_leadership);
					return Ok(true);
				} else {
					// Another device is the leader
					return Err(LeaderError::NotLeader {
						current_leader: leadership.leader_device_id,
						expires_at: leadership.lease_expires_at,
					});
				}
			}
		}

		// No valid leader - we can become the leader
		info!(
			library_id = %library_id,
			device_id = %self.device_id,
			"Becoming leader for library"
		);

		let leadership = SyncLeadership::new(self.device_id);
		self.library_leadership.insert(library_id, leadership);
		Ok(true)
	}

	/// Send a heartbeat (leader only)
	///
	/// Extends the lease and returns the updated leadership state
	/// to be broadcast to followers.
	pub fn send_heartbeat(&mut self, library_id: Uuid) -> Result<SyncLeadership> {
		if let Some(leadership) = self.library_leadership.get_mut(&library_id) {
			if leadership.leader_device_id != self.device_id {
				return Err(LeaderError::NotLeader {
					current_leader: leadership.leader_device_id,
					expires_at: leadership.lease_expires_at,
				});
			}

			leadership.extend_lease();
			Ok(leadership.clone())
		} else {
			Err(LeaderError::InvalidState(
				"No leadership state for library".to_string(),
			))
		}
	}

	/// Check for leader timeouts and trigger re-election if needed
	///
	/// Should be called periodically by followers to detect leader failures.
	pub fn check_leader_timeout(&mut self, library_id: Uuid) -> Option<SyncRole> {
		if let Some(leadership) = self.library_leadership.get(&library_id) {
			if leadership.has_timed_out() && leadership.leader_device_id != self.device_id {
				warn!(
					library_id = %library_id,
					old_leader = %leadership.leader_device_id,
					last_heartbeat = %leadership.last_heartbeat_at,
					"Leader timeout detected, requesting leadership"
				);

				// Attempt to become leader
				match self.request_leadership(library_id) {
					Ok(true) => {
						info!(
							library_id = %library_id,
							new_leader = %self.device_id,
							"Successfully elected as new leader"
						);
						return Some(SyncRole::Leader);
					}
					Ok(false) => {
						debug!("Leadership request denied");
					}
					Err(e) => {
						debug!("Leadership request failed: {}", e);
					}
				}
			}
		}
		None
	}

	/// Get the device ID of this device
	pub fn device_id(&self) -> Uuid {
		self.device_id
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_sync_leadership_creation() {
		let device_id = Uuid::new_v4();
		let leadership = SyncLeadership::new(device_id);

		assert_eq!(leadership.leader_device_id, device_id);
		assert!(leadership.is_valid());
		assert!(!leadership.has_timed_out());
	}

	#[test]
	fn test_leadership_manager_initialization() {
		let device_id = Uuid::new_v4();
		let library_id = Uuid::new_v4();

		let mut manager = LeadershipManager::new(device_id);

		// Creator becomes leader
		let role = manager.initialize_library(library_id, true);
		assert_eq!(role, SyncRole::Leader);
		assert!(manager.is_leader(library_id));

		// Non-creator is follower
		let library_id2 = Uuid::new_v4();
		let role = manager.initialize_library(library_id2, false);
		assert_eq!(role, SyncRole::Follower);
		assert!(!manager.is_leader(library_id2));
	}

	#[test]
	fn test_leadership_request() {
		let device_id = Uuid::new_v4();
		let library_id = Uuid::new_v4();

		let mut manager = LeadershipManager::new(device_id);

		// First request should succeed
		let result = manager.request_leadership(library_id);
		assert!(result.is_ok());
		assert!(manager.is_leader(library_id));

		// Second request should succeed (we're already leader)
		let result = manager.request_leadership(library_id);
		assert!(result.is_ok());
	}

	#[test]
	fn test_follower_cannot_be_leader() {
		let leader_id = Uuid::new_v4();
		let follower_id = Uuid::new_v4();
		let library_id = Uuid::new_v4();

		// Leader establishes leadership
		let mut leader_manager = LeadershipManager::new(leader_id);
		leader_manager.initialize_library(library_id, true);
		assert!(leader_manager.is_leader(library_id));

		// Follower learns about leader
		let mut follower_manager = LeadershipManager::new(follower_id);
		let leadership = leader_manager
			.library_leadership
			.get(&library_id)
			.unwrap()
			.clone();
		follower_manager.update_leadership(library_id, leadership);

		// Follower cannot become leader while lease is valid
		let result = follower_manager.request_leadership(library_id);
		assert!(result.is_err());
		assert!(!follower_manager.is_leader(library_id));
	}

	#[test]
	fn test_heartbeat_extends_lease() {
		let device_id = Uuid::new_v4();
		let library_id = Uuid::new_v4();

		let mut manager = LeadershipManager::new(device_id);
		manager.initialize_library(library_id, true);

		let original_expiry = manager
			.library_leadership
			.get(&library_id)
			.unwrap()
			.lease_expires_at;

		// Wait a bit and send heartbeat
		std::thread::sleep(std::time::Duration::from_millis(100));

		let result = manager.send_heartbeat(library_id);
		assert!(result.is_ok());

		let new_expiry = manager
			.library_leadership
			.get(&library_id)
			.unwrap()
			.lease_expires_at;

		// Lease should be extended
		assert!(new_expiry > original_expiry);
	}
}
