//! Hybrid Logical Clock (HLC) implementation for distributed sync
//!
//! HLC provides a globally consistent ordering of events across devices without
//! requiring clock synchronization. It combines physical time with a logical counter
//! to ensure causality is preserved.

use crate::infra::sync::time_source::TimeSource;
use serde::{Deserialize, Serialize};
use std::str::FromStr;
use std::sync::{Arc, Mutex};
use uuid::Uuid;

/// Hybrid Logical Clock
///
/// Provides total ordering of events across distributed devices by combining:
/// - Physical time (milliseconds since epoch)
/// - Logical counter (for events in same millisecond)
/// - Device ID (for deterministic tie-breaking)
#[derive(Debug, Clone, Copy, Serialize, Deserialize, Eq, PartialEq)]
pub struct HLC {
	/// Physical time component (milliseconds since Unix epoch)
	pub timestamp: u64,

	/// Logical counter for events within the same millisecond
	pub counter: u64,

	/// Device that generated this HLC (for deterministic ordering)
	pub device_id: Uuid,
}

impl HLC {
	/// Create a new HLC with current time and zero counter
	pub fn now(device_id: Uuid, time_source: &dyn TimeSource) -> Self {
		Self {
			timestamp: time_source.current_time_ms(),
			counter: 0,
			device_id,
		}
	}

	/// Generate next HLC based on previous HLC
	///
	/// If the timestamp is the same millisecond, increments the counter.
	/// Otherwise, resets counter to 0 with new timestamp.
	pub fn generate(last: Option<HLC>, device_id: Uuid, time_source: &dyn TimeSource) -> Self {
		let now = time_source.current_time_ms();

		match last {
			Some(last) if last.timestamp == now => {
				// Same millisecond, increment counter
				Self {
					timestamp: now,
					counter: last.counter + 1,
					device_id,
				}
			}
			_ => {
				// New millisecond or no previous HLC
				Self {
					timestamp: now,
					counter: 0,
					device_id,
				}
			}
		}
	}

	/// Update this HLC based on received HLC (causality tracking)
	///
	/// Implements the HLC update rule:
	/// - Take max of local and received timestamp
	/// - If same timestamp, take max counter + 1
	/// - Otherwise reset counter based on which timestamp is used
	pub fn update(&mut self, received: HLC, time_source: &dyn TimeSource) {
		let now = time_source.current_time_ms();

		// Take max of all three: local, received, and physical time
		let max_timestamp = self.timestamp.max(received.timestamp).max(now);

		if max_timestamp == self.timestamp && max_timestamp == received.timestamp {
			// Both had same timestamp, increment past both
			self.counter = self.counter.max(received.counter) + 1;
		} else if max_timestamp == received.timestamp {
			// Received is newer, adopt their counter + 1
			self.timestamp = received.timestamp;
			self.counter = received.counter + 1;
		} else if max_timestamp == now && now > self.timestamp.max(received.timestamp) {
			// Physical time jumped ahead, reset counter
			self.timestamp = now;
			self.counter = 0;
		}
		// else: local timestamp is still the max, keep it
	}

	/// Convert HLC to sortable string representation
	///
	/// Format: "{timestamp:016x}-{counter:016x}-{device_id}"
	/// This format is lexicographically sortable and can be used as a database key.
	pub fn as_display(&self) -> String {
		format!(
			"{:016x}-{:016x}-{}",
			self.timestamp, self.counter, self.device_id
		)
	}

	/// Parse HLC from string representation
	pub fn from_string(s: &str) -> Result<Self, HLCError> {
		// Split only on first two hyphens (UUID contains hyphens)
		let parts: Vec<&str> = s.splitn(3, '-').collect();
		if parts.len() != 3 {
			return Err(HLCError::ParseError(format!(
				"Invalid HLC format: expected 3 parts, got {}. Input: '{}'",
				parts.len(),
				s
			)));
		}

		let timestamp = u64::from_str_radix(parts[0], 16)
			.map_err(|e| HLCError::ParseError(format!("Invalid timestamp: {}", e)))?;

		let counter = u64::from_str_radix(parts[1], 16)
			.map_err(|e| HLCError::ParseError(format!("Invalid counter: {}", e)))?;

		let device_id = Uuid::parse_str(parts[2])
			.map_err(|e| HLCError::ParseError(format!("Invalid device_id: {}", e)))?;

		Ok(Self {
			timestamp,
			counter,
			device_id,
		})
	}
}

/// Ordering is based on: timestamp, then counter, then device_id
impl Ord for HLC {
	fn cmp(&self, other: &Self) -> std::cmp::Ordering {
		self.timestamp
			.cmp(&other.timestamp)
			.then(self.counter.cmp(&other.counter))
			.then(self.device_id.cmp(&other.device_id))
	}
}

impl PartialOrd for HLC {
	fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
		Some(self.cmp(other))
	}
}

impl std::fmt::Display for HLC {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		write!(
			f,
			"{:016x}-{:016x}-{}",
			self.timestamp, self.counter, self.device_id
		)
	}
}

/// Implement FromStr trait for HLC parsing from strings
///
/// This enables parsing HLC from watermark strings stored in database.
/// Required for TODO #3: HLC incremental sync.
impl FromStr for HLC {
	type Err = HLCError;

	fn from_str(s: &str) -> Result<Self, Self::Err> {
		Self::from_string(s)
	}
}

/// HLC Generator for a device
///
/// Thread-safe HLC generator that maintains causality by tracking
/// the last generated HLC and updating based on received HLCs.
pub struct HLCGenerator {
	device_id: Uuid,
	last_hlc: Mutex<Option<HLC>>,
	time_source: Arc<dyn TimeSource>,
}

impl HLCGenerator {
	/// Create a new HLC generator for this device with specified time source
	pub fn new(device_id: Uuid, time_source: Arc<dyn TimeSource>) -> Self {
		Self {
			device_id,
			last_hlc: Mutex::new(None),
			time_source,
		}
	}

	/// Generate the next HLC
	///
	/// This is the primary method for creating HLCs for local events.
	pub fn next(&self) -> HLC {
		let mut last = self.last_hlc.lock().unwrap();
		let new_hlc = HLC::generate(*last, self.device_id, self.time_source.as_ref());
		*last = Some(new_hlc);
		new_hlc
	}

	/// Update based on received HLC (causality tracking)
	///
	/// Call this when receiving an HLC from another device to ensure
	/// causality is preserved in subsequently generated HLCs.
	pub fn update(&self, received: HLC) {
		let mut last = self.last_hlc.lock().unwrap();

		match *last {
			Some(mut local) => {
				local.update(received, self.time_source.as_ref());
				*last = Some(local);
			}
			None => {
				// First HLC received, initialize with it
				*last = Some(received);
			}
		}
	}

	/// Get the last generated or received HLC
	pub fn last(&self) -> Option<HLC> {
		*self.last_hlc.lock().unwrap()
	}
}

/// HLC-related errors
#[derive(Debug, thiserror::Error)]
pub enum HLCError {
	#[error("Failed to parse HLC: {0}")]
	ParseError(String),
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::infra::sync::time_source::FakeTimeSource;

	#[test]
	fn test_hlc_generation() {
		let device_id = Uuid::new_v4();
		let time = FakeTimeSource::new(1000);

		let hlc1 = HLC::now(device_id, &time);
		assert_eq!(hlc1.counter, 0);
		assert_eq!(hlc1.timestamp, 1000);
		assert_eq!(hlc1.device_id, device_id);

		// Generate next in same millisecond (time hasn't advanced)
		let hlc2 = HLC::generate(Some(hlc1), device_id, &time);
		assert_eq!(hlc2.timestamp, hlc1.timestamp);
		assert_eq!(hlc2.counter, hlc1.counter + 1);

		// Advance time and generate again
		time.advance(100);
		let hlc3 = HLC::generate(Some(hlc2), device_id, &time);
		assert_eq!(hlc3.timestamp, 1100);
		assert_eq!(hlc3.counter, 0);
	}

	#[test]
	fn test_hlc_ordering() {
		let device_a = Uuid::new_v4();
		let device_b = Uuid::new_v4();

		let hlc1 = HLC {
			timestamp: 1000,
			counter: 0,
			device_id: device_a,
		};

		let hlc2 = HLC {
			timestamp: 1000,
			counter: 1,
			device_id: device_b,
		};

		let hlc3 = HLC {
			timestamp: 1001,
			counter: 0,
			device_id: device_a,
		};

		// Timestamp ordering
		assert!(hlc1 < hlc2);
		assert!(hlc2 < hlc3);
		assert!(hlc1 < hlc3);

		// Total ordering is guaranteed
		assert!(hlc1.cmp(&hlc2) != std::cmp::Ordering::Equal);
	}

	#[test]
	fn test_hlc_update_causality() {
		let device_a = Uuid::new_v4();
		let device_b = Uuid::new_v4();
		let time = FakeTimeSource::new(1000);

		let mut local = HLC {
			timestamp: 1000,
			counter: 0,
			device_id: device_a,
		};

		let received = HLC {
			timestamp: 1005,
			counter: 3,
			device_id: device_b,
		};

		local.update(received, &time);

		// Since physical time (1000) < received (1005), should adopt received + 1
		assert_eq!(local.timestamp, 1005);
		assert_eq!(local.counter, 4);

		// Now advance time ahead of both
		time.set(2000);
		let received2 = HLC {
			timestamp: 1500,
			counter: 0,
			device_id: device_b,
		};

		local.update(received2, &time);

		// Physical time (2000) is max, counter resets
		assert_eq!(local.timestamp, 2000);
		assert_eq!(local.counter, 0);
	}

	#[test]
	fn test_hlc_string_roundtrip() {
		let device_id = Uuid::new_v4();
		let hlc = HLC {
			timestamp: 1234567890,
			counter: 42,
			device_id,
		};

		let s = hlc.to_string();
		let parsed = HLC::from_string(&s).unwrap();

		assert_eq!(hlc, parsed);
	}

	#[test]
	fn test_hlc_generator() {
		let device_id = Uuid::new_v4();
		let time = FakeTimeSource::new(1000);
		let gen = HLCGenerator::new(device_id, Arc::new(time.clone()));

		let hlc1 = gen.next();
		assert_eq!(hlc1.device_id, device_id);
		assert_eq!(hlc1.timestamp, 1000);
		assert_eq!(hlc1.counter, 0);

		// Same millisecond
		let hlc2 = gen.next();
		assert_eq!(hlc2.timestamp, 1000);
		assert_eq!(hlc2.counter, 1);

		// Advance time
		time.advance(500);
		let hlc3 = gen.next();
		assert_eq!(hlc3.timestamp, 1500);
		assert_eq!(hlc3.counter, 0);
	}

	#[test]
	fn test_generator_causality_tracking() {
		let device_a = Uuid::new_v4();
		let device_b = Uuid::new_v4();

		let time_a = FakeTimeSource::new(1000);
		let time_b = FakeTimeSource::new(1000);

		let gen_a = HLCGenerator::new(device_a, Arc::new(time_a.clone()));
		let gen_b = HLCGenerator::new(device_b, Arc::new(time_b.clone()));

		// Device A generates event at t=1000
		let hlc_a = gen_a.next();
		assert_eq!(hlc_a.timestamp, 1000);

		// Device B receives it (still at t=1000) and updates
		gen_b.update(hlc_a);

		// Device B generates next event (still t=1000)
		let hlc_b = gen_b.next();

		// B's event must be after A's (causality preserved)
		// Same timestamp, but counter incremented
		assert!(hlc_b > hlc_a);
		assert_eq!(hlc_b.timestamp, 1000);
		assert_eq!(hlc_b.counter, 1);
	}
}
