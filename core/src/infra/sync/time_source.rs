//! Time source abstraction for HLC testing
//!
//! Provides a trait-based abstraction over time sources, enabling
//! deterministic testing of HLC behavior without relying on system time.
//! Production code uses SystemTimeSource for actual wall-clock time, while
//! tests use FakeTimeSource for controlled, reproducible timing.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

/// Abstracts time source for HLC generation
///
/// Allows HLC to work with real system time in production and fake
/// controllable time in tests. All implementations must be thread-safe
/// (Send + Sync) since HLCGenerator is shared across async tasks.
pub trait TimeSource: Send + Sync {
	/// Returns current time in milliseconds since Unix epoch
	fn current_time_ms(&self) -> u64;
}

/// Production time source using system clock
///
/// Uses chrono::Utc for millisecond-precision timestamps. This is the
/// standard time source for all production HLC operations.
#[derive(Debug, Clone, Copy, Default)]
pub struct SystemTimeSource;

impl TimeSource for SystemTimeSource {
	fn current_time_ms(&self) -> u64 {
		chrono::Utc::now().timestamp_millis() as u64
	}
}

/// Test time source with manual control
///
/// Provides deterministic time for tests. Uses AtomicU64 for thread-safety,
/// allowing tests to share a single FakeTimeSource across multiple HLCGenerators
/// while controlling time progression. Supports setting time backwards for
/// clock skew testing scenarios.
///
/// ## Example
/// ```rust,no_run
/// use sd_core::infra::sync::time_source::FakeTimeSource;
///
/// let time = FakeTimeSource::new(1000);
/// assert_eq!(time.current_time_ms(), 1000);
///
/// time.advance(500);
/// assert_eq!(time.current_time_ms(), 1500);
///
/// time.set(2000);
/// assert_eq!(time.current_time_ms(), 2000);
///
/// // Can go backwards for clock skew testing
/// time.set(1000);
/// assert_eq!(time.current_time_ms(), 1000);
/// ```
#[derive(Debug, Clone)]
pub struct FakeTimeSource {
	time: Arc<AtomicU64>,
}

impl Default for FakeTimeSource {
	fn default() -> Self {
		Self::new(1000)
	}
}

impl FakeTimeSource {
	/// Create new fake time source at specified timestamp
	pub fn new(initial_ms: u64) -> Self {
		Self {
			time: Arc::new(AtomicU64::new(initial_ms)),
		}
	}

	/// Advance time by delta milliseconds
	pub fn advance(&self, delta_ms: u64) {
		self.time.fetch_add(delta_ms, Ordering::SeqCst);
	}

	/// Set time to specific value (can go backwards for clock skew testing)
	pub fn set(&self, time_ms: u64) {
		self.time.store(time_ms, Ordering::SeqCst);
	}

	/// Get current fake time
	pub fn get(&self) -> u64 {
		self.time.load(Ordering::SeqCst)
	}
}

impl TimeSource for FakeTimeSource {
	fn current_time_ms(&self) -> u64 {
		self.time.load(Ordering::SeqCst)
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_fake_time_source() {
		let time = FakeTimeSource::new(1000);
		assert_eq!(time.current_time_ms(), 1000);

		time.advance(500);
		assert_eq!(time.current_time_ms(), 1500);

		time.set(2000);
		assert_eq!(time.current_time_ms(), 2000);
	}

	#[test]
	fn test_fake_time_default() {
		let time = FakeTimeSource::default();
		assert_eq!(time.current_time_ms(), 1000);
	}

	#[test]
	fn test_fake_time_backwards() {
		let time = FakeTimeSource::new(5000);
		time.set(1000);
		assert_eq!(time.current_time_ms(), 1000);
	}

	#[test]
	fn test_fake_time_clone() {
		let time = FakeTimeSource::new(1000);
		let time_clone = time.clone();

		time.advance(100);
		assert_eq!(time_clone.get(), 1100);

		time_clone.set(500);
		assert_eq!(time.get(), 500);
	}
}
