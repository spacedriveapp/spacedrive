//! # Failed login limiter
//!
//! `sd-server` answers HTTP Basic on every request, so a password can be
//! guessed as fast as the network allows. The limiter counts failed
//! authentications per client address and locks the address out once it
//! reaches the limit within the window. A locked address is refused before its
//! credentials are checked, so a correct guess during the lockout teaches the
//! attacker nothing.
//!
//! The address is the TCP peer. Behind a reverse proxy every client shares the
//! proxy's address, which is the price of not trusting `X-Forwarded-For`, a
//! header any client can forge. IPv6 clients are grouped by /64, the smallest
//! prefix a single host is usually given, so rotating addresses inside it
//! does not reset the count.
//!
//! Successful logins never clear a count. Failures age out with the window, so
//! a legitimate user behind the same address cannot launder an attacker's
//! guesses.
//!
//! ## Example
//! ```rust,ignore
//! let limiter = LoginLimiter::default();
//! let ip = "203.0.113.7".parse().unwrap();
//! if limiter.check(ip).is_ok() {
//!     limiter.record_failure(ip);
//! }
//! ```

use std::{
	collections::HashMap,
	net::{IpAddr, Ipv6Addr},
	sync::Mutex,
	time::{Duration, Instant},
};

use tracing::warn;

/// Failed authentications allowed per client address in one window.
pub const MAX_FAILURES: u32 = 20;
pub const WINDOW: Duration = Duration::from_secs(60);
pub const LOCKOUT: Duration = Duration::from_secs(5 * 60);
/// Addresses tracked at once, so a flood of sources cannot grow memory without limit.
pub const MAX_TRACKED: usize = 10_000;

#[derive(Debug, Clone, Copy)]
struct ClientState {
	window_start: Instant,
	failures: u32,
	locked_until: Option<Instant>,
}

impl ClientState {
	fn is_stale(&self, now: Instant, window: Duration) -> bool {
		match self.locked_until {
			Some(until) => until <= now,
			None => now.duration_since(self.window_start) >= window,
		}
	}
}

#[derive(Debug)]
pub struct LoginLimiter {
	max_failures: u32,
	window: Duration,
	lockout: Duration,
	max_tracked: usize,
	clients: Mutex<HashMap<IpAddr, ClientState>>,
}

impl Default for LoginLimiter {
	fn default() -> Self {
		Self::new(MAX_FAILURES, WINDOW, LOCKOUT, MAX_TRACKED)
	}
}

impl LoginLimiter {
	pub fn new(max_failures: u32, window: Duration, lockout: Duration, max_tracked: usize) -> Self {
		Self {
			max_failures,
			window,
			lockout,
			max_tracked: max_tracked.max(1),
			clients: Mutex::new(HashMap::new()),
		}
	}

	/// Returns the time left on the address's lockout, if it is locked out.
	pub fn check(&self, ip: IpAddr) -> Result<(), Duration> {
		self.check_at(ip, Instant::now())
	}

	/// Counts a failed authentication and returns the lockout it started, if any.
	pub fn record_failure(&self, ip: IpAddr) -> Option<Duration> {
		self.record_failure_at(ip, Instant::now())
	}

	fn check_at(&self, ip: IpAddr, now: Instant) -> Result<(), Duration> {
		let key = client_key(ip);
		let mut clients = self.clients.lock().unwrap_or_else(|e| e.into_inner());
		let Some(state) = clients.get(&key) else {
			return Ok(());
		};
		match state.locked_until {
			Some(until) if until > now => Err(until - now),
			Some(_) => {
				clients.remove(&key);
				Ok(())
			}
			None => Ok(()),
		}
	}

	fn record_failure_at(&self, ip: IpAddr, now: Instant) -> Option<Duration> {
		let key = client_key(ip);
		let mut clients = self.clients.lock().unwrap_or_else(|e| e.into_inner());
		if !clients.contains_key(&key) && clients.len() >= self.max_tracked {
			self.make_room(&mut clients, now);
		}
		let state = clients.entry(key).or_insert(ClientState {
			window_start: now,
			failures: 0,
			locked_until: None,
		});
		if let Some(until) = state.locked_until {
			if until > now {
				return None;
			}
		}
		if state.is_stale(now, self.window) {
			*state = ClientState {
				window_start: now,
				failures: 0,
				locked_until: None,
			};
		}
		state.failures += 1;
		if state.failures < self.max_failures {
			return None;
		}
		state.locked_until = Some(now + self.lockout);
		warn!(
			client = %key,
			failures = state.failures,
			lockout_secs = self.lockout.as_secs(),
			"Too many failed logins; locking out client address"
		);
		Some(self.lockout)
	}

	/// Drops stale entries, then the least valuable live one, to admit a new address.
	///
	/// Unlocked counters go before lockouts, and among lockouts the one closest
	/// to ending goes first, so a flood of fresh addresses cannot free an
	/// attacker that is already locked out until every other slot is spent.
	fn make_room(&self, clients: &mut HashMap<IpAddr, ClientState>, now: Instant) {
		clients.retain(|_, state| !state.is_stale(now, self.window));
		if clients.len() < self.max_tracked {
			return;
		}
		let victim = clients
			.iter()
			.min_by_key(|(_, state)| match state.locked_until {
				None => (0, state.window_start),
				Some(until) => (1, until),
			})
			.map(|(ip, _)| *ip);
		if let Some(ip) = victim {
			clients.remove(&ip);
		}
	}

	#[cfg(test)]
	fn tracked(&self) -> usize {
		self.clients.lock().unwrap().len()
	}
}

/// The bucket a client address counts against.
fn client_key(ip: IpAddr) -> IpAddr {
	match ip.to_canonical() {
		IpAddr::V6(v6) => {
			let segments = v6.segments();
			IpAddr::V6(Ipv6Addr::new(
				segments[0],
				segments[1],
				segments[2],
				segments[3],
				0,
				0,
				0,
				0,
			))
		}
		v4 => v4,
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	fn ip(s: &str) -> IpAddr {
		s.parse().unwrap()
	}

	#[test]
	fn locks_out_after_max_failures() {
		let limiter = LoginLimiter::default();
		let now = Instant::now();
		let client = ip("203.0.113.7");
		for _ in 0..MAX_FAILURES - 1 {
			assert_eq!(limiter.record_failure_at(client, now), None);
			assert!(limiter.check_at(client, now).is_ok());
		}
		assert_eq!(limiter.record_failure_at(client, now), Some(LOCKOUT));
		assert_eq!(limiter.check_at(client, now), Err(LOCKOUT));
	}

	#[test]
	fn lockout_expires() {
		let limiter = LoginLimiter::default();
		let now = Instant::now();
		let client = ip("203.0.113.7");
		for _ in 0..MAX_FAILURES {
			limiter.record_failure_at(client, now);
		}
		let almost = now + LOCKOUT - Duration::from_secs(1);
		assert_eq!(
			limiter.check_at(client, almost),
			Err(Duration::from_secs(1))
		);
		assert!(limiter.check_at(client, now + LOCKOUT).is_ok());
		assert_eq!(limiter.record_failure_at(client, now + LOCKOUT), None);
	}

	#[test]
	fn failures_age_out_with_the_window() {
		let limiter = LoginLimiter::default();
		let now = Instant::now();
		let client = ip("203.0.113.7");
		for _ in 0..MAX_FAILURES - 1 {
			limiter.record_failure_at(client, now);
		}
		let later = now + WINDOW;
		assert_eq!(limiter.record_failure_at(client, later), None);
		assert!(limiter.check_at(client, later).is_ok());
	}

	#[test]
	fn other_addresses_are_unaffected() {
		let limiter = LoginLimiter::default();
		let now = Instant::now();
		for _ in 0..MAX_FAILURES {
			limiter.record_failure_at(ip("203.0.113.7"), now);
		}
		assert!(limiter.check_at(ip("203.0.113.8"), now).is_ok());
		assert!(limiter.check_at(ip("2001:db8:1::1"), now).is_ok());
	}

	#[test]
	fn ipv6_counts_per_slash_64_and_mapped_ipv4_per_address() {
		let limiter = LoginLimiter::default();
		let now = Instant::now();
		for i in 0..MAX_FAILURES {
			limiter.record_failure_at(ip(&format!("2001:db8::{:x}", i + 1)), now);
		}
		assert!(limiter.check_at(ip("2001:db8::ffff"), now).is_err());
		assert!(limiter.check_at(ip("2001:db8:0:1::1"), now).is_ok());

		for _ in 0..MAX_FAILURES {
			limiter.record_failure_at(ip("::ffff:198.51.100.1"), now);
		}
		assert!(limiter.check_at(ip("198.51.100.1"), now).is_err());
		assert!(limiter.check_at(ip("::ffff:198.51.100.2"), now).is_ok());
	}

	#[test]
	fn tracking_is_bounded_and_keeps_lockouts() {
		let limiter = LoginLimiter::new(2, WINDOW, LOCKOUT, 4);
		let now = Instant::now();
		let locked = ip("192.0.2.1");
		limiter.record_failure_at(locked, now);
		limiter.record_failure_at(locked, now);
		for i in 0..100u8 {
			limiter.record_failure_at(ip(&format!("198.51.100.{i}")), now);
			assert!(limiter.tracked() <= 4);
		}
		assert!(limiter.check_at(locked, now).is_err());
	}
}
