//! Read-pattern recorder for mount frontends.
//!
//! The open question the mounts design turns on is whether a loopback
//! server can serve streaming reads as well as a native filesystem module.
//! That argument rests on what read pattern the host's own client produces
//! — how large its reads are, whether it prefetches, how it behaves on a
//! seek — and none of that is knowable by reasoning about it.
//!
//! So every frontend records what it was asked for. This says nothing about
//! how well the request was served; `mounts.cache_status` answers that. The
//! two together are experiments 2 and 5 in `docs/core/design/mounts.md`.
//!
//! Recording is off until switched on and costs one relaxed atomic load
//! when it is off.

use std::collections::{HashMap, VecDeque};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::Instant;

/// Ring capacity. A minute of heavy scrubbing is a few thousand reads, so
/// this holds a whole session without bounding the experiment.
const CAPACITY: usize = 200_000;

static ENABLED: AtomicBool = AtomicBool::new(false);
static EVENTS: OnceLock<Mutex<VecDeque<ReadEvent>>> = OnceLock::new();
static ORIGIN: OnceLock<Instant> = OnceLock::new();

#[derive(Debug, Clone, Copy)]
pub struct ReadEvent {
	/// Milliseconds since the recorder's origin, so a session reads as a
	/// timeline rather than wall-clock noise.
	pub at_ms: u64,
	/// Which frontend was asked.
	pub frontend: Frontend,
	/// Stable per-path id, for telling sequential from seeking without
	/// keeping paths in the ring.
	pub file: u64,
	pub offset: u64,
	pub len: u64,
	/// How long the frontend waited for the bytes.
	pub micros: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Frontend {
	Smb,
	WebDav,
}

impl Frontend {
	fn as_str(self) -> &'static str {
		match self {
			Self::Smb => "smb",
			Self::WebDav => "webdav",
		}
	}
}

fn events() -> &'static Mutex<VecDeque<ReadEvent>> {
	EVENTS.get_or_init(|| Mutex::new(VecDeque::with_capacity(1024)))
}

pub fn is_enabled() -> bool {
	ENABLED.load(Ordering::Relaxed)
}

/// Turn recording on or off. Enabling starts a fresh timeline.
pub fn set_enabled(on: bool) {
	if on {
		let _ = ORIGIN.set(Instant::now());
		clear();
	}
	ENABLED.store(on, Ordering::Relaxed);
}

pub fn clear() {
	events().lock().unwrap().clear();
}

/// Stable id for a path without holding the string.
pub fn file_id(path: &Path) -> u64 {
	use std::hash::{Hash, Hasher};
	let mut hasher = std::collections::hash_map::DefaultHasher::new();
	path.hash(&mut hasher);
	hasher.finish()
}

/// Record one served read. `started` is when the frontend began waiting.
pub fn record(frontend: Frontend, file: u64, offset: u64, len: u64, started: Instant) {
	if !is_enabled() {
		return;
	}
	let origin = ORIGIN.get_or_init(Instant::now);
	let event = ReadEvent {
		at_ms: started.saturating_duration_since(*origin).as_millis() as u64,
		frontend,
		file,
		offset,
		len,
		micros: started.elapsed().as_micros() as u64,
	};
	let mut events = events().lock().unwrap();
	if events.len() >= CAPACITY {
		events.pop_front();
	}
	events.push_back(event);
}

// ---------------------------------------------------------------- summary

/// What the trace says, reduced to the numbers the decision needs.
#[derive(Debug, Clone, Default)]
pub struct ReadSummary {
	pub enabled: bool,
	pub reads: u64,
	pub bytes: u64,
	pub files: u64,
	pub span_ms: u64,
	/// The read size a client asks for most often — the single most telling
	/// number, because it is what a native module would get to choose.
	pub common_read_bytes: u64,
	pub min_read_bytes: u64,
	pub max_read_bytes: u64,
	pub mean_read_bytes: u64,
	/// Reads that begin exactly where the previous read of that file ended.
	pub sequential: u64,
	/// Reads that jump backwards or forwards, i.e. a seek.
	pub seeks: u64,
	/// Reads that overlap the previous one — a client re-reading bytes it
	/// already had, which is pure waste a native module could avoid.
	pub rereads: u64,
	pub p50_micros: u64,
	pub p95_micros: u64,
	pub max_micros: u64,
	/// Read-size distribution, coarse buckets, largest first non-empty.
	pub size_buckets: Vec<(String, u64)>,
	pub by_frontend: Vec<(String, u64)>,
}

const BUCKETS: [(u64, &str); 7] = [
	(4 * 1024, "<=4 KiB"),
	(64 * 1024, "<=64 KiB"),
	(128 * 1024, "<=128 KiB"),
	(256 * 1024, "<=256 KiB"),
	(1024 * 1024, "<=1 MiB"),
	(4 * 1024 * 1024, "<=4 MiB"),
	(u64::MAX, "> 4 MiB"),
];

pub fn summary() -> ReadSummary {
	let events = events().lock().unwrap();
	let mut out = ReadSummary {
		enabled: is_enabled(),
		reads: events.len() as u64,
		..Default::default()
	};
	if events.is_empty() {
		return out;
	}

	let mut sizes: HashMap<u64, u64> = HashMap::new();
	let mut buckets: HashMap<&'static str, u64> = HashMap::new();
	let mut frontends: HashMap<&'static str, u64> = HashMap::new();
	let mut last_end: HashMap<u64, u64> = HashMap::new();
	let mut latencies: Vec<u64> = Vec::with_capacity(events.len());
	let mut files: HashMap<u64, ()> = HashMap::new();

	out.min_read_bytes = u64::MAX;
	let first_at = events.front().map(|e| e.at_ms).unwrap_or(0);
	let last_at = events.back().map(|e| e.at_ms).unwrap_or(0);
	out.span_ms = last_at.saturating_sub(first_at);

	for event in events.iter() {
		out.bytes += event.len;
		out.min_read_bytes = out.min_read_bytes.min(event.len);
		out.max_read_bytes = out.max_read_bytes.max(event.len);
		*sizes.entry(event.len).or_default() += 1;
		*frontends.entry(event.frontend.as_str()).or_default() += 1;
		files.insert(event.file, ());
		latencies.push(event.micros);

		let bucket = BUCKETS
			.iter()
			.find(|(limit, _)| event.len <= *limit)
			.map(|(_, name)| *name)
			.unwrap_or("> 4 MiB");
		*buckets.entry(bucket).or_default() += 1;

		match last_end.get(&event.file) {
			Some(&end) if event.offset == end => out.sequential += 1,
			Some(&end) if event.offset < end => out.rereads += 1,
			Some(_) => out.seeks += 1,
			// The first read of a file is neither.
			None => {}
		}
		last_end.insert(event.file, event.offset + event.len);
	}

	out.files = files.len() as u64;
	out.mean_read_bytes = out.bytes / out.reads.max(1);
	out.common_read_bytes = sizes
		.into_iter()
		.max_by_key(|(size, count)| (*count, *size))
		.map(|(size, _)| size)
		.unwrap_or(0);

	latencies.sort_unstable();
	let pick = |q: f64| -> u64 {
		let idx = ((latencies.len() as f64 - 1.0) * q).round() as usize;
		latencies.get(idx).copied().unwrap_or(0)
	};
	out.p50_micros = pick(0.50);
	out.p95_micros = pick(0.95);
	out.max_micros = latencies.last().copied().unwrap_or(0);

	out.size_buckets = BUCKETS
		.iter()
		.filter_map(|(_, name)| buckets.get(name).map(|count| ((*name).to_string(), *count)))
		.collect();
	out.by_frontend = frontends
		.into_iter()
		.map(|(name, count)| (name.to_string(), count))
		.collect();
	out
}

#[cfg(test)]
mod tests {
	use super::*;
	use std::path::PathBuf;

	/// The recorder is process-global by design, so tests that switch it on
	/// and off take turns. Without this one test clears the buffer another
	/// is filling, which fails rarely enough to be mistaken for a real bug.
	static SERIAL: Mutex<()> = Mutex::new(());

	fn at(offset: u64, len: u64) -> ReadEvent {
		ReadEvent {
			at_ms: 0,
			frontend: Frontend::Smb,
			file: 1,
			offset,
			len,
			micros: 10,
		}
	}

	#[test]
	fn classifies_sequential_seek_and_reread() {
		let _turn = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
		set_enabled(true);
		{
			let mut events = events().lock().unwrap();
			events.clear();
			events.push_back(at(0, 1000)); // first: uncounted
			events.push_back(at(1000, 1000)); // sequential
			events.push_back(at(2000, 1000)); // sequential
			events.push_back(at(9000, 1000)); // seek forward
			events.push_back(at(500, 1000)); // backwards: a re-read
		}
		let s = summary();
		assert_eq!(s.reads, 5);
		assert_eq!(s.sequential, 2);
		assert_eq!(s.seeks, 1);
		assert_eq!(s.rereads, 1);
		assert_eq!(s.common_read_bytes, 1000);
		assert_eq!(s.files, 1);
		set_enabled(false);
	}

	#[test]
	fn disabled_records_nothing() {
		let _turn = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
		set_enabled(false);
		clear();
		record(Frontend::Smb, 1, 0, 4096, Instant::now());
		assert_eq!(summary().reads, 0);
	}

	#[test]
	fn file_ids_separate_paths() {
		assert_ne!(
			file_id(&PathBuf::from("/a/one.mov")),
			file_id(&PathBuf::from("/a/two.mov"))
		);
	}
}
