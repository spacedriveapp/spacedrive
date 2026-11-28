//! # Agent MCP limits
//!
//! Per-grant and per-endpoint limits.
//!
//! The defaults are the table in `docs/core/design/agent-access-threat-model.md`.
//! An agent loops, retries and fans out, so every budget here is checked
//! before work starts and answered with 429 and `Retry-After` when spent.

use std::time::{Duration, Instant};

/// Request limits the endpoint enforces.
///
/// The daemon runs with [`Limits::default`] except for
/// `block_bytes_per_hour`, which comes from `agent_mcp.block_bytes_per_hour`.
/// Tests lower single fields to reach a limit quickly.
#[derive(Debug, Clone)]
pub struct Limits {
	pub sessions_per_grant: usize,
	pub calls_per_grant: usize,
	pub calls_total: usize,
	pub rate_per_second: f64,
	pub rate_burst: f64,
	pub max_body_bytes: usize,
	pub max_header_bytes: usize,
	pub call_timeout: Duration,
	pub bytes_per_hour: u64,
	/// Bytes the space endpoint's block reads may return per grant per hour,
	/// kept apart from `bytes_per_hour` so a mount can page files in without
	/// spending the tools' budget.
	pub block_bytes_per_hour: u64,
	pub auth_failures_per_minute: u32,
	pub auth_lockout: Duration,
	/// How long an idle MCP session lives. Event streams have no idle limit.
	pub session_idle: Duration,
	pub revocation_poll: Duration,
	/// How often an event stream sends a heartbeat comment.
	pub stream_heartbeat: Duration,
	/// How long one event stream write may wait on the client before the
	/// stream is closed as stalled.
	pub stream_write_timeout: Duration,
}

impl Default for Limits {
	fn default() -> Self {
		Self {
			sessions_per_grant: 4,
			calls_per_grant: 4,
			calls_total: 32,
			rate_per_second: 10.0,
			rate_burst: 30.0,
			max_body_bytes: 64 * 1024,
			max_header_bytes: 16 * 1024,
			call_timeout: Duration::from_secs(30),
			bytes_per_hour: 256 * 1024 * 1024,
			block_bytes_per_hour: super::DEFAULT_BLOCK_BYTES_PER_HOUR,
			auth_failures_per_minute: 20,
			auth_lockout: Duration::from_secs(5 * 60),
			session_idle: Duration::from_secs(30 * 60),
			revocation_poll: Duration::from_millis(500),
			stream_heartbeat: Duration::from_secs(15),
			stream_write_timeout: Duration::from_secs(30),
		}
	}
}

/// `read` returns this much per call unless the agent asks for less or more.
pub const READ_DEFAULT_BYTES: u64 = 64 * 1024;
/// The most one `read` may return.
pub const READ_MAX_BYTES: u64 = 1024 * 1024;
/// Lines a line-range `read` returns unless the agent asks for another count.
pub const READ_DEFAULT_LINES: u64 = 2000;
/// Entries per `list_dir` and `glob` page.
pub const PAGE_MAX: u32 = 1000;
/// Entries per page when the agent does not say.
pub const PAGE_DEFAULT: u32 = 200;
/// Entries one `glob` call may visit before it stops and says so.
pub const GLOB_VISIT_MAX: u64 = 100_000;
/// Longest `grep` and `glob` pattern.
pub const PATTERN_MAX_BYTES: usize = 1024;
/// Largest compiled `grep` regex.
pub const REGEX_MAX_BYTES: usize = 1024 * 1024;
/// Matches per `grep` page.
pub const GREP_PAGE_MAX: u32 = 1000;
/// Results per `search` page.
pub const SEARCH_PAGE_MAX: u32 = 100;
/// Results per `search` page when the agent does not say.
pub const SEARCH_PAGE_DEFAULT: u32 = 20;
/// Results per `search_content` page, the most `search.content` returns.
pub const CONTENT_PAGE_MAX: u32 = 200;
/// Results per `search_content` page when the agent does not say.
pub const CONTENT_PAGE_DEFAULT: u32 = 20;
/// Results per `search_semantic` page, the most `search.semantic` returns.
pub const SEMANTIC_PAGE_MAX: u32 = crate::ops::search::semantic::MAX_RESULTS_LIMIT;
/// The longest `search_semantic` query, the most `search.semantic` embeds.
pub const SEMANTIC_QUERY_MAX_BYTES: usize = crate::ops::search::semantic::MAX_QUERY_BYTES;

/// A token bucket: `rate` tokens a second, holding at most `burst`.
#[derive(Debug)]
pub struct Bucket {
	tokens: f64,
	last: Instant,
}

impl Bucket {
	pub fn full(burst: f64) -> Self {
		Self {
			tokens: burst,
			last: Instant::now(),
		}
	}

	/// Take one token, or say how long until one is available.
	pub fn take(&mut self, rate: f64, burst: f64) -> Result<(), Duration> {
		let now = Instant::now();
		let elapsed = now.duration_since(self.last).as_secs_f64();
		self.tokens = (self.tokens + elapsed * rate).min(burst);
		self.last = now;
		if self.tokens >= 1.0 {
			self.tokens -= 1.0;
			return Ok(());
		}
		Err(Duration::from_secs_f64((1.0 - self.tokens) / rate))
	}
}

/// Bytes returned in the current hour.
#[derive(Debug)]
pub struct ByteWindow {
	started: Instant,
	bytes: u64,
}

impl ByteWindow {
	pub fn new() -> Self {
		Self {
			started: Instant::now(),
			bytes: 0,
		}
	}

	fn roll(&mut self) {
		if self.started.elapsed() >= Duration::from_secs(3600) {
			self.started = Instant::now();
			self.bytes = 0;
		}
	}

	/// How long until the window resets, when `budget` is spent.
	pub fn spent(&mut self, budget: u64) -> Option<Duration> {
		self.roll();
		(self.bytes >= budget)
			.then(|| Duration::from_secs(3600).saturating_sub(self.started.elapsed()))
	}

	pub fn add(&mut self, bytes: u64) {
		self.roll();
		self.bytes = self.bytes.saturating_add(bytes);
	}
}

/// Failed authentications from one client address.
#[derive(Debug)]
pub struct Failures {
	window: Instant,
	count: u32,
	locked_until: Option<Instant>,
}

impl Failures {
	pub fn new() -> Self {
		Self {
			window: Instant::now(),
			count: 0,
			locked_until: None,
		}
	}

	/// How long the address stays locked out, if it is.
	pub fn locked(&self) -> Option<Duration> {
		self.locked_until
			.and_then(|until| until.checked_duration_since(Instant::now()))
	}

	/// Count one failure and lock the address once it passes `per_minute`.
	pub fn fail(&mut self, per_minute: u32, lockout: Duration) {
		if self.window.elapsed() >= Duration::from_secs(60) {
			self.window = Instant::now();
			self.count = 0;
		}
		self.count += 1;
		if self.count > per_minute {
			self.locked_until = Some(Instant::now() + lockout);
			self.count = 0;
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn a_bucket_allows_its_burst_then_refuses() {
		let mut bucket = Bucket::full(3.0);
		for _ in 0..3 {
			assert!(bucket.take(1.0, 3.0).is_ok());
		}
		let wait = bucket.take(1.0, 3.0).unwrap_err();
		assert!(wait <= Duration::from_secs(1));
	}

	#[test]
	fn failures_lock_after_the_limit() {
		let mut failures = Failures::new();
		for _ in 0..3 {
			failures.fail(3, Duration::from_secs(60));
			assert!(failures.locked().is_none());
		}
		failures.fail(3, Duration::from_secs(60));
		assert!(failures.locked().is_some());
	}

	#[test]
	fn a_spent_window_reports_its_reset() {
		let mut window = ByteWindow::new();
		assert!(window.spent(10).is_none());
		window.add(10);
		assert!(window.spent(10).is_some());
	}
}
