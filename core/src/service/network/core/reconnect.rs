//! # Reconnection policy
//!
//! Decides which side of a paired pair dials after a connection drops and how
//! the dial attempts are spaced. The device with the lower EndpointId is the
//! initiator and dials at once, so the two sides do not race each other into
//! duplicate connections. The other side defers, but only for a grace period:
//! if the initiator lost its retry state (it restarted too, or its dial cannot
//! reach us) the deferrer dials anyway rather than waiting forever.
//!
//! Retries back off exponentially up to a cap and stop after a bounded number
//! of attempts; the periodic sweep in the networking service re-arms a fresh
//! schedule for any device that is still disconnected.

use std::time::Duration;

use iroh::EndpointId;

/// How long the deferring side waits for the initiator before dialing itself.
pub const DEFER_GRACE: Duration = Duration::from_secs(15);

/// Upper bound on a single dial, so a hung discovery cannot stall the schedule.
pub const CONNECT_TIMEOUT: Duration = Duration::from_secs(30);

const BASE_DELAY: Duration = Duration::from_secs(2);
const MAX_DELAY: Duration = Duration::from_secs(30);
const MAX_ATTEMPTS: u32 = 12;

/// Which side of a pair is expected to dial first.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DialRole {
	Initiator,
	Deferrer,
}

/// The lower EndpointId initiates; exactly one side of any pair gets this role.
pub fn dial_role(me: &EndpointId, remote: &EndpointId) -> DialRole {
	if me < remote {
		DialRole::Initiator
	} else {
		DialRole::Deferrer
	}
}

/// Retry schedule for one device's reconnection loop.
#[derive(Debug, Clone)]
pub struct ReconnectSchedule {
	role: DialRole,
	attempts: u32,
}

impl ReconnectSchedule {
	pub fn new(role: DialRole) -> Self {
		Self { role, attempts: 0 }
	}

	pub fn role(&self) -> DialRole {
		self.role
	}

	/// Delay before the first dial: none for the initiator, the grace period
	/// for the deferrer.
	pub fn initial_delay(&self) -> Duration {
		match self.role {
			DialRole::Initiator => Duration::ZERO,
			DialRole::Deferrer => DEFER_GRACE,
		}
	}

	/// Records a failed dial and returns how long to wait before the next one,
	/// or `None` once the schedule is exhausted.
	pub fn next_delay(&mut self) -> Option<Duration> {
		self.attempts += 1;
		if self.attempts >= MAX_ATTEMPTS {
			return None;
		}
		let exponent = (self.attempts - 1).min(16);
		let delay = BASE_DELAY.saturating_mul(1u32 << exponent);
		Some(delay.min(MAX_DELAY))
	}

	pub fn attempts(&self) -> u32 {
		self.attempts
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use iroh::SecretKey;

	fn ids() -> (EndpointId, EndpointId) {
		let a = SecretKey::from_bytes(&[1u8; 32]).public();
		let b = SecretKey::from_bytes(&[2u8; 32]).public();
		if a < b {
			(a, b)
		} else {
			(b, a)
		}
	}

	#[test]
	fn exactly_one_side_initiates() {
		let (low, high) = ids();
		assert_eq!(dial_role(&low, &high), DialRole::Initiator);
		assert_eq!(dial_role(&high, &low), DialRole::Deferrer);
	}

	#[test]
	fn initiator_dials_at_once_and_deferrer_waits_for_grace() {
		assert_eq!(
			ReconnectSchedule::new(DialRole::Initiator).initial_delay(),
			Duration::ZERO
		);
		assert_eq!(
			ReconnectSchedule::new(DialRole::Deferrer).initial_delay(),
			DEFER_GRACE
		);
	}

	#[test]
	fn backoff_grows_then_caps_then_exhausts() {
		let mut schedule = ReconnectSchedule::new(DialRole::Initiator);
		let mut delays = Vec::new();
		while let Some(delay) = schedule.next_delay() {
			delays.push(delay);
		}
		assert_eq!(delays.len() as u32, MAX_ATTEMPTS - 1);
		assert_eq!(delays[0], Duration::from_secs(2));
		assert_eq!(delays[1], Duration::from_secs(4));
		assert_eq!(delays[2], Duration::from_secs(8));
		assert_eq!(delays[3], Duration::from_secs(16));
		assert!(delays[4..].iter().all(|d| *d == MAX_DELAY));
		assert_eq!(schedule.attempts(), MAX_ATTEMPTS);

		let total: Duration = delays.iter().sum();
		assert!(total <= Duration::from_secs(5 * 60));
	}

	#[test]
	fn deferrer_eventually_dials_within_bounded_time() {
		let mut schedule = ReconnectSchedule::new(DialRole::Deferrer);
		let first_dial_at = schedule.initial_delay();
		assert!(first_dial_at <= Duration::from_secs(30));
		assert!(schedule.next_delay().is_some());
	}
}
