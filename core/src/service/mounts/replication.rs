//! Controls over replica transfers: the bandwidth cap both ends of a fetch
//! share, and the pause switch. A replica fetch is the one transfer
//! Spacedrive starts on its own, as soon as a paired device connects, and
//! an initial copy runs to gigabytes, so on a metered or shared link it
//! needs a ceiling the person sets once and the daemon keeps, and a way to
//! defer it entirely that survives a restart.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

use tokio::time::Instant;

use crate::config::ReplicationConfig;

/// A token bucket over bytes. The rate is the sustained ceiling and also
/// the burst: one second of credit accumulates while nothing moves, so a
/// transfer that starts after a pause does not stall for its first chunk.
/// A chunk larger than the burst passes against a full bucket and leaves it
/// in debt, which the following chunks pay off, so a low cap still moves
/// large chunks at the asked-for average.
pub struct Throttle {
	bytes_per_sec: AtomicU64,
	bucket: Mutex<Bucket>,
}

struct Bucket {
	tokens: f64,
	refilled_at: Instant,
}

impl Throttle {
	pub fn new(bytes_per_sec: u64) -> Self {
		Self {
			bytes_per_sec: AtomicU64::new(bytes_per_sec),
			bucket: Mutex::new(Bucket {
				tokens: bytes_per_sec as f64,
				refilled_at: Instant::now(),
			}),
		}
	}

	/// The cap in bytes per second; zero is unlimited.
	pub fn bytes_per_sec(&self) -> u64 {
		self.bytes_per_sec.load(Ordering::Relaxed)
	}

	/// Change the cap. Takes effect at the next chunk of every transfer in
	/// flight; credit already in the bucket is clamped to the new burst.
	pub fn set_bytes_per_sec(&self, bytes_per_sec: u64) {
		self.bytes_per_sec.store(bytes_per_sec, Ordering::Relaxed);
		let mut bucket = self.bucket.lock().unwrap();
		bucket.tokens = bucket.tokens.min(bytes_per_sec as f64);
	}

	/// Wait until `bytes` may move. Returns at once while unlimited.
	pub async fn acquire(&self, bytes: u64) {
		loop {
			let rate = self.bytes_per_sec() as f64;
			if rate == 0.0 {
				return;
			}
			let wait = {
				let mut bucket = self.bucket.lock().unwrap();
				let now = Instant::now();
				let refill = now.duration_since(bucket.refilled_at).as_secs_f64() * rate;
				bucket.tokens = (bucket.tokens + refill).min(rate);
				bucket.refilled_at = now;
				let want = bytes as f64;
				if bucket.tokens >= want || bucket.tokens >= rate {
					bucket.tokens -= want;
					return;
				}
				Duration::from_secs_f64((want.min(rate) - bucket.tokens) / rate)
			};
			tokio::time::sleep(wait).await;
		}
	}
}

static THROTTLE: OnceLock<Throttle> = OnceLock::new();

/// The daemon's one cap, shared by every artifact served and every artifact
/// received, so the setting bounds what replication costs the link in total.
pub fn throttle() -> &'static Throttle {
	THROTTLE.get_or_init(|| Throttle::new(0))
}

static PAUSED: AtomicBool = AtomicBool::new(false);

/// Whether replica fetches are paused. A paused daemon still lists and
/// refreshes what owners publish, serves what it already holds, and answers
/// peers' fetches; it starts no transfer of its own, and one in flight stops
/// at its next chunk with its partial file kept.
pub fn paused() -> bool {
	PAUSED.load(Ordering::Relaxed)
}

/// Flip the pause switch in memory. The persisted value lives in the
/// daemon config, which the action that calls this writes.
pub fn set_paused(paused: bool) {
	PAUSED.store(paused, Ordering::Relaxed);
}

/// Apply the persisted settings at startup and after a config update.
pub fn configure(config: &ReplicationConfig) {
	throttle().set_bytes_per_sec(config.max_bytes_per_sec);
	set_paused(config.paused);
}

#[cfg(test)]
mod tests {
	use super::*;

	/// A full bucket passes one second of bytes at once; past that, bytes
	/// move at the cap. Measured against tokio's paused clock, so the test
	/// asserts the limiter's arithmetic rather than the scheduler's timing.
	#[tokio::test(start_paused = true)]
	async fn bytes_past_the_burst_move_at_the_cap() {
		let throttle = Throttle::new(1_000);
		let started = Instant::now();
		throttle.acquire(1_000).await;
		assert_eq!(started.elapsed(), Duration::ZERO, "the burst is free");

		throttle.acquire(500).await;
		assert_eq!(started.elapsed(), Duration::from_millis(500));

		throttle.acquire(2_000).await;
		assert_eq!(
			started.elapsed(),
			Duration::from_millis(1_500),
			"a chunk past the burst goes once a full second of credit is in"
		);
		throttle.acquire(1_000).await;
		assert_eq!(
			started.elapsed(),
			Duration::from_millis(3_500),
			"4,500 bytes after a 1,000-byte burst take 3.5 s at 1,000 B/s"
		);
	}

	/// A chunk bigger than one second of the cap still passes, against a
	/// full bucket, and the debt it leaves delays what follows.
	#[tokio::test(start_paused = true)]
	async fn an_oversized_chunk_passes_and_leaves_debt() {
		let throttle = Throttle::new(100);
		let started = Instant::now();
		throttle.acquire(1_000).await;
		assert_eq!(started.elapsed(), Duration::ZERO);
		throttle.acquire(100).await;
		assert_eq!(
			started.elapsed(),
			Duration::from_secs(10),
			"the next full bucket arrives after the debt is paid"
		);
	}

	/// Zero is unlimited, and the cap can change under a transfer.
	#[tokio::test(start_paused = true)]
	async fn zero_is_unlimited_and_the_cap_changes_live() {
		let throttle = Throttle::new(0);
		let started = Instant::now();
		throttle.acquire(u64::MAX).await;
		assert_eq!(started.elapsed(), Duration::ZERO);

		throttle.set_bytes_per_sec(1_000);
		throttle.acquire(1_000).await;
		throttle.acquire(1_000).await;
		assert_eq!(started.elapsed(), Duration::from_secs(2));

		throttle.set_bytes_per_sec(0);
		throttle.acquire(1 << 40).await;
		assert_eq!(started.elapsed(), Duration::from_secs(2));
	}
}
