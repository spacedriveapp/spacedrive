use std::time::Duration;

/// Supervision cadence and thresholds. The defaults are the production
/// values; tests inject shorter ones.
#[derive(Debug, Clone)]
pub struct Timing {
	/// Interval between health passes over every service.
	pub health_interval: Duration,
	/// Consecutive failed probes before an adopted service is declared dead
	/// and respawned as an owned child.
	pub adopted_death_threshold: u32,
	/// An owned child that exits faster than this counts toward the failure cap.
	pub fast_exit: Duration,
	/// Fast exits tolerated before the supervisor gives up on a service.
	pub max_fast_exits: u32,
	/// First restart delay; doubles per consecutive fast exit.
	pub backoff_base: Duration,
	/// Ceiling on the restart delay.
	pub backoff_cap: Duration,
	/// Time between SIGTERM and SIGKILL when stopping an owned child.
	pub stop_grace: Duration,
}

impl Default for Timing {
	fn default() -> Self {
		Self {
			health_interval: Duration::from_secs(30),
			adopted_death_threshold: 3,
			fast_exit: Duration::from_secs(10),
			max_fast_exits: 5,
			backoff_base: Duration::from_secs(1),
			backoff_cap: Duration::from_secs(60),
			stop_grace: Duration::from_secs(10),
		}
	}
}

impl Timing {
	/// Restart delay after a crash: exponential in the consecutive fast-exit
	/// count, capped.
	pub fn backoff(&self, fast_exits: u32) -> Duration {
		let factor = 2u32.saturating_pow(fast_exits);
		self.backoff_base
			.saturating_mul(factor)
			.min(self.backoff_cap)
	}
}
