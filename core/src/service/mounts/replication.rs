//! Controls over replica transfers: the bandwidth cap both ends of a fetch
//! share, the pause switch, and the progress of every fetch in flight. A
//! replica fetch is the one transfer Spacedrive starts on its own, as soon
//! as a paired device connects, and an initial copy runs to gigabytes, so
//! on a metered or shared link it needs a ceiling the person sets once and
//! the daemon keeps, a way to defer it entirely that survives a restart,
//! and a place that says how far it has got.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use specta::Type;
use tokio::time::Instant;
use uuid::Uuid;

use crate::config::ReplicationConfig;
use crate::infra::event::{Event, EventBus};
use crate::service::network::protocol::RemoteSourceInfo;

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

/// The daemon's one cap, shared by every artifact and sidecar page served
/// or received, so the setting bounds what replication costs the link in
/// total. Tile fetches for a replica being browsed are not counted; they
/// are a person's own reads, not replication.
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

	/// A transfer is listed from its first byte to its last and no longer:
	/// the reporter owns the row, counts from where it resumed, and clears
	/// it however the fetch ends.
	#[tokio::test]
	async fn a_reporter_owns_its_row_for_the_life_of_the_transfer() {
		let info = RemoteSourceInfo {
			id: Uuid::now_v7(),
			root: std::path::PathBuf::from("/mnt/pool/kept"),
			volume_uuid: None,
			attached: true,
			entry_count: None,
			total_bytes: None,
			generation: 1,
			dirty: false,
			nested: false,
			sidecar_store: Uuid::nil(),
			sidecar_cursor: 0,
		};
		let reporter = TransferReporter::start(None, Uuid::now_v7(), "owner", &info, 400, 1_000);
		let listed = transfer(info.id).expect("listed while in flight");
		assert_eq!(
			(listed.bytes, listed.total, listed.resumed_from),
			(400, 1_000, 400)
		);
		reporter.advance(100);
		assert_eq!(transfer(info.id).expect("still listed").bytes, 500);
		drop(reporter);
		assert!(
			transfer(info.id).is_none(),
			"the row goes with the reporter"
		);
	}

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

/// One replica fetch as it stands. `bytes` counts what is on disk,
/// including what an earlier attempt left in the partial file, so a resumed
/// transfer reports from where it continues rather than from zero.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
pub struct ReplicaTransferProgress {
	pub device_id: Uuid,
	pub device_label: String,
	pub source_id: Uuid,
	/// The source's root on its owner, for naming the row.
	pub root: std::path::PathBuf,
	pub bytes: u64,
	pub total: u64,
	/// Recent transfer rate, averaged over the last few seconds.
	pub bytes_per_sec: u64,
	/// Where this attempt picked up; zero for a fresh transfer.
	pub resumed_from: u64,
	/// The transfer has left the active set: complete when `bytes` reaches
	/// `total`, stopped otherwise with its partial file kept.
	pub finished: bool,
}

/// The replication picture `core.status` and the CLI summarize.
///
/// Defaults to no transfers so a newer client can still read the status
/// payload of a daemon that predates the field.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, Type)]
pub struct ReplicationStatus {
	pub paused: bool,
	pub max_bytes_per_sec: u64,
	pub transfers: Vec<ReplicaTransferProgress>,
}

impl ReplicationStatus {
	pub fn bytes(&self) -> u64 {
		self.transfers.iter().map(|t| t.bytes).sum()
	}

	pub fn total(&self) -> u64 {
		self.transfers.iter().map(|t| t.total).sum()
	}

	pub fn bytes_per_sec(&self) -> u64 {
		self.transfers.iter().map(|t| t.bytes_per_sec).sum()
	}
}

struct ActiveTransfer {
	progress: ReplicaTransferProgress,
	/// The sample the rate is measured from, replaced every window.
	window: (Instant, u64),
	/// The rate seen in the previous window, so a fresh one does not swing.
	last_rate: u64,
	announced: Instant,
}

const RATE_WINDOW: Duration = Duration::from_secs(3);
const ANNOUNCE_EVERY: Duration = Duration::from_secs(1);

static ACTIVE: OnceLock<Mutex<HashMap<Uuid, ActiveTransfer>>> = OnceLock::new();

fn active() -> &'static Mutex<HashMap<Uuid, ActiveTransfer>> {
	ACTIVE.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Every fetch in flight right now.
pub fn transfers() -> Vec<ReplicaTransferProgress> {
	let mut transfers: Vec<_> = active()
		.lock()
		.unwrap()
		.values()
		.map(|t| t.progress.clone())
		.collect();
	transfers.sort_by(|a, b| a.root.cmp(&b.root));
	transfers
}

/// The fetch in flight for one source, if any.
pub fn transfer(source_id: Uuid) -> Option<ReplicaTransferProgress> {
	active()
		.lock()
		.unwrap()
		.get(&source_id)
		.map(|t| t.progress.clone())
}

pub fn status() -> ReplicationStatus {
	ReplicationStatus {
		paused: paused(),
		max_bytes_per_sec: throttle().bytes_per_sec(),
		transfers: transfers(),
	}
}

/// One fetch's reporter: it owns the source's entry in the active set for
/// as long as it lives, publishes a progress event about once a second,
/// and on drop announces the transfer finished, complete or not. Dropping
/// is what every exit path does, so a cancelled task reports too.
pub struct TransferReporter {
	source_id: Uuid,
	events: Option<Arc<EventBus>>,
}

impl TransferReporter {
	pub fn start(
		events: Option<Arc<EventBus>>,
		device_id: Uuid,
		device_label: &str,
		info: &RemoteSourceInfo,
		resumed_from: u64,
		total: u64,
	) -> Self {
		let progress = ReplicaTransferProgress {
			device_id,
			device_label: device_label.to_string(),
			source_id: info.id,
			root: info.root.clone(),
			bytes: resumed_from,
			total,
			bytes_per_sec: 0,
			resumed_from,
			finished: false,
		};
		let now = Instant::now();
		active().lock().unwrap().insert(
			info.id,
			ActiveTransfer {
				progress: progress.clone(),
				window: (now, resumed_from),
				last_rate: 0,
				announced: now,
			},
		);
		if let Some(events) = &events {
			events.emit(Event::ReplicationProgress { transfer: progress });
		}
		Self {
			source_id: info.id,
			events,
		}
	}

	/// Record that `bytes` more landed on disk.
	pub fn advance(&self, bytes: u64) {
		let announce = {
			let mut active = active().lock().unwrap();
			let Some(entry) = active.get_mut(&self.source_id) else {
				return;
			};
			let now = Instant::now();
			entry.progress.bytes += bytes;
			let since = now.duration_since(entry.window.0);
			if since >= RATE_WINDOW {
				entry.last_rate =
					((entry.progress.bytes - entry.window.1) as f64 / since.as_secs_f64()) as u64;
				entry.window = (now, entry.progress.bytes);
			}
			entry.progress.bytes_per_sec = if entry.last_rate == 0 && since > Duration::ZERO {
				((entry.progress.bytes - entry.window.1) as f64 / since.as_secs_f64()) as u64
			} else {
				entry.last_rate
			};
			if now.duration_since(entry.announced) >= ANNOUNCE_EVERY {
				entry.announced = now;
				Some(entry.progress.clone())
			} else {
				None
			}
		};
		if let (Some(events), Some(progress)) = (&self.events, announce) {
			events.emit(Event::ReplicationProgress { transfer: progress });
		}
	}
}

impl Drop for TransferReporter {
	fn drop(&mut self) {
		let Some(entry) = active().lock().unwrap().remove(&self.source_id) else {
			return;
		};
		if let Some(events) = &self.events {
			let mut progress = entry.progress;
			progress.finished = true;
			progress.bytes_per_sec = 0;
			events.emit(Event::ReplicationProgress { transfer: progress });
		}
	}
}
