//! The pvcache-backed tile source — the grid's half of the cross-process
//! thumbnail contract.
//!
//! The source opens `thumbs.pvcache` with [`PvcacheReader`] (read-only mmap)
//! and never touches the writer handle, even while the demo bake runs in the
//! same process: the daemon owns the writer in the shipped topology, and this
//! side is the code that stays. Grid index → record uuid comes from an entry
//! list delivered once (today the demo folder walk; later a daemon op), and
//! bake completions arrive over a channel (later daemon events) to re-read
//! slots the grid is showing placeholders or stale pixels for.
//!
//! Slot outcomes map to the grid like this: `Fresh` and `Stale` both deliver
//! pixels (stale pixels are valid old content, shown while the rebake runs);
//! `Absent` keeps the placeholder up until the bake for that slot completes.
//! Every wait state settles — a failed bake or the fill ending retires its
//! cells — so [`TileSource::has_pending`] reaches false and the grid idles at
//! true 0% CPU.

use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::mpsc::{channel, Receiver, Sender, TryRecvError};
use std::thread;

use sd_pvcache::{PvcacheReader, TileState};
use uuid::Uuid;

use super::{TileSource, VisibleRange, TILE};

/// Completions absorbed per poll, bounding the per-frame cost of a large
/// background fill finishing while the grid sits still.
const COMPLETIONS_PER_POLL: usize = 4096;

/// One grid cell's identity in the cache: the record uuid and the content
/// version its tile must match to be fresh.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Entry {
	pub uuid: Uuid,
	pub version: u64,
}

/// A bake finishing for a cell, reported by whoever owns the writer. `ok` is
/// false when no producer could turn the source file into pixels.
#[derive(Debug, Clone, Copy)]
pub struct Completion {
	pub index: u32,
	pub ok: bool,
}

struct ReadRequest {
	index: u32,
	uuid: Uuid,
	version: u64,
}

struct Delivery {
	index: u32,
	outcome: ReadOutcome,
}

enum ReadOutcome {
	Fresh(Vec<u8>),
	Stale(Vec<u8>),
	Absent,
}

pub struct PvcacheSource {
	entries: Vec<Entry>,
	/// Consumed once: the walk (or a daemon op) delivers the full entry list,
	/// then the channel is dropped.
	entries_rx: Option<Receiver<Vec<Entry>>>,
	completions_rx: Receiver<Completion>,
	/// The completion channel disconnected: the writer side is done and no
	/// further slots will be published this run.
	filler_done: bool,
	visible: VisibleRange,
	/// Requested cells with no pixels delivered yet — a read is in flight or
	/// a bake is expected.
	waiting: HashSet<u32>,
	/// Cells showing stale pixels while their rebake is in flight.
	stale_wait: HashSet<u32>,
	/// Cells whose bake was reported failed; their slots may still hold
	/// displayable stale pixels, but nothing fresh is coming.
	failed: HashSet<u32>,
	/// Cells that will never fill this run (bake failed with nothing stored);
	/// requests for them are dropped so the grid can go idle.
	dead: HashSet<u32>,
	read_tx: Sender<ReadRequest>,
	delivery_rx: Receiver<Delivery>,
}

impl PvcacheSource {
	/// Open a source over the cache file at `path`. `entries_rx` delivers the
	/// grid's index → `(uuid, version)` mapping once; `completions_rx` streams
	/// bake completions from the writer side for as long as it runs.
	pub fn new(
		path: PathBuf,
		entries_rx: Receiver<Vec<Entry>>,
		completions_rx: Receiver<Completion>,
		visible: VisibleRange,
	) -> Self {
		let (read_tx, read_rx) = channel();
		let (delivery_tx, delivery_rx) = channel();
		thread::Builder::new()
			.name("pvcache-read".into())
			.spawn(move || run_reader(&path, &read_rx, &delivery_tx))
			.expect("failed to spawn pvcache read thread");
		Self {
			entries: Vec::new(),
			entries_rx: Some(entries_rx),
			completions_rx,
			filler_done: false,
			visible,
			waiting: HashSet::new(),
			stale_wait: HashSet::new(),
			failed: HashSet::new(),
			dead: HashSet::new(),
			read_tx,
			delivery_rx,
		}
	}

	fn send_read(&self, index: u32) {
		if let Some(entry) = self.entries.get(index as usize) {
			let _ = self.read_tx.send(ReadRequest {
				index,
				uuid: entry.uuid,
				version: entry.version,
			});
		}
	}
}

impl TileSource for PvcacheSource {
	fn poll(&mut self) {
		if let Some(rx) = &self.entries_rx {
			match rx.try_recv() {
				Ok(entries) => {
					self.entries = entries;
					self.entries_rx = None;
				}
				Err(TryRecvError::Disconnected) => self.entries_rx = None,
				Err(TryRecvError::Empty) => {}
			}
		}
		for _ in 0..COMPLETIONS_PER_POLL {
			match self.completions_rx.try_recv() {
				Ok(Completion { index, ok: true }) => {
					// Only cells the grid is actually showing (placeholder or
					// stale pixels) need a re-read; everything else is read on
					// first request.
					if self.waiting.contains(&index) || self.stale_wait.contains(&index) {
						self.send_read(index);
					}
				}
				Ok(Completion { index, ok: false }) => {
					self.failed.insert(index);
					if self.waiting.remove(&index) {
						self.dead.insert(index);
					}
					self.stale_wait.remove(&index);
				}
				Err(TryRecvError::Empty) => break,
				Err(TryRecvError::Disconnected) => {
					if !self.filler_done {
						self.filler_done = true;
						// Nothing more will be published; settle every open
						// wait with a final read so pending state drains and
						// the grid can go idle.
						let open: Vec<u32> = self
							.waiting
							.iter()
							.chain(self.stale_wait.iter())
							.copied()
							.collect();
						for index in open {
							self.send_read(index);
						}
					}
					break;
				}
			}
		}
	}

	fn len(&self) -> u32 {
		self.entries.len() as u32
	}

	fn tile(&self) -> u32 {
		TILE
	}

	fn request(&mut self, idx: u32) {
		if idx as usize >= self.entries.len()
			|| self.waiting.contains(&idx)
			|| self.stale_wait.contains(&idx)
			|| self.dead.contains(&idx)
		{
			return;
		}
		self.waiting.insert(idx);
		self.send_read(idx);
	}

	fn has_pending(&self) -> bool {
		self.entries_rx.is_some() || !self.waiting.is_empty() || !self.stale_wait.is_empty()
	}

	fn drain(&mut self, max: usize) -> Vec<(u32, Vec<u8>)> {
		let mut out = Vec::new();
		while out.len() < max {
			let Ok(delivery) = self.delivery_rx.try_recv() else {
				break;
			};
			let idx = delivery.index;
			match delivery.outcome {
				ReadOutcome::Fresh(pixels) => {
					self.waiting.remove(&idx);
					self.stale_wait.remove(&idx);
					out.push((idx, pixels));
				}
				ReadOutcome::Stale(pixels) => {
					self.waiting.remove(&idx);
					if self.failed.contains(&idx) || self.filler_done {
						// No rebake is coming; the stale pixels are final for
						// this run.
						self.stale_wait.remove(&idx);
					} else {
						self.stale_wait.insert(idx);
					}
					out.push((idx, pixels));
				}
				ReadOutcome::Absent => {
					// Keep waiting for the bake — unless nothing can fill this
					// slot anymore, in which case the cell retires so the grid
					// can idle on its placeholder.
					if self.failed.contains(&idx) || self.filler_done {
						self.waiting.remove(&idx);
						self.dead.insert(idx);
					}
				}
			}
		}
		out
	}

	fn set_visible_range(&mut self, first: u32, last: u32) {
		self.visible.set(first, last);
	}
}

/// The read thread: turns `(uuid, version)` requests into pixel copies out of
/// the read-only mapping. Exits when the source (and its request sender) drop.
fn run_reader(path: &std::path::Path, rx: &Receiver<ReadRequest>, tx: &Sender<Delivery>) {
	let tile_len = (TILE * TILE * 4) as usize;
	let mut reader: Option<PvcacheReader> = None;
	while let Ok(request) = rx.recv() {
		let outcome = read_slot(path, &mut reader, tile_len, &request);
		if tx
			.send(Delivery {
				index: request.index,
				outcome,
			})
			.is_err()
		{
			return;
		}
	}
}

fn read_slot(
	path: &std::path::Path,
	reader: &mut Option<PvcacheReader>,
	tile_len: usize,
	request: &ReadRequest,
) -> ReadOutcome {
	// Two attempts: a failed mapping (the file was reinitialized under us, or
	// grew past what the handle can sync) drops the handle and reopens once
	// before reporting absent. Growth alone never errors — the reader remaps
	// itself on a generation change inside `get`.
	for _ in 0..2 {
		if reader.is_none() {
			// The writer may not have created the file yet (first launch); an
			// absent or geometry-mismatched file reads as no cache, and the
			// bake completion for this slot triggers the retry.
			*reader = PvcacheReader::open(path)
				.ok()
				.filter(|r| r.tile_len() == tile_len);
		}
		let Some(r) = reader.as_mut() else {
			return ReadOutcome::Absent;
		};
		let mut pixels = vec![0u8; tile_len];
		match r.get(request.uuid, request.version, &mut pixels) {
			Ok(TileState::Fresh) => return ReadOutcome::Fresh(pixels),
			Ok(TileState::Stale { .. }) => return ReadOutcome::Stale(pixels),
			Ok(TileState::Absent) => return ReadOutcome::Absent,
			Err(_) => *reader = None,
		}
	}
	ReadOutcome::Absent
}

#[cfg(test)]
mod tests {
	use super::*;
	use sd_pvcache::Pvcache;
	use std::path::Path;
	use std::time::{Duration, Instant};

	fn tile_bytes(b: u8) -> Vec<u8> {
		vec![b; (TILE * TILE * 4) as usize]
	}

	fn make_source(dir: &Path) -> (PvcacheSource, Sender<Vec<Entry>>, Sender<Completion>, PathBuf) {
		let path = dir.join("thumbs.pvcache");
		let (entries_tx, entries_rx) = channel();
		let (completions_tx, completions_rx) = channel();
		let source = PvcacheSource::new(
			path.clone(),
			entries_rx,
			completions_rx,
			VisibleRange::new(0, 64),
		);
		(source, entries_tx, completions_tx, path)
	}

	/// Poll and drain until one tile arrives or the deadline passes.
	fn drain_one(source: &mut PvcacheSource, deadline: Duration) -> Option<(u32, Vec<u8>)> {
		let end = Instant::now() + deadline;
		loop {
			source.poll();
			if let Some(tile) = source.drain(16).pop() {
				return Some(tile);
			}
			if Instant::now() > end {
				return None;
			}
			thread::sleep(Duration::from_millis(5));
		}
	}

	/// Poll and drain until the source reports idle or the deadline passes.
	fn settle(source: &mut PvcacheSource, deadline: Duration) -> bool {
		let end = Instant::now() + deadline;
		loop {
			source.poll();
			let _ = source.drain(usize::MAX);
			if !source.has_pending() {
				return true;
			}
			if Instant::now() > end {
				return false;
			}
			thread::sleep(Duration::from_millis(5));
		}
	}

	#[test]
	fn fresh_tile_is_read_through_the_reader() {
		let dir = tempfile::tempdir().expect("tempdir");
		let (mut source, entries_tx, _completions_tx, path) = make_source(dir.path());
		let uuid = Uuid::from_u128(1);

		let mut writer = Pvcache::open_or_create(&path, TILE, TILE).expect("create cache");
		writer.write(uuid, 7, &tile_bytes(0xAB)).expect("write");

		entries_tx
			.send(vec![Entry { uuid, version: 7 }])
			.expect("send entries");
		source.poll();
		assert_eq!(source.len(), 1);

		source.request(0);
		let (idx, pixels) =
			drain_one(&mut source, Duration::from_secs(10)).expect("tile delivered");
		assert_eq!(idx, 0);
		assert!(pixels.iter().all(|&b| b == 0xAB));
		assert!(!source.has_pending(), "fresh delivery settles the cell");
	}

	#[test]
	fn stale_pixels_show_then_refresh_on_completion() {
		let dir = tempfile::tempdir().expect("tempdir");
		let (mut source, entries_tx, completions_tx, path) = make_source(dir.path());
		let uuid = Uuid::from_u128(2);

		let mut writer = Pvcache::open_or_create(&path, TILE, TILE).expect("create cache");
		writer.write(uuid, 1, &tile_bytes(0x11)).expect("write v1");

		entries_tx
			.send(vec![Entry { uuid, version: 2 }])
			.expect("send entries");
		source.poll();
		source.request(0);

		let (_, pixels) =
			drain_one(&mut source, Duration::from_secs(10)).expect("stale tile delivered");
		assert!(pixels.iter().all(|&b| b == 0x11), "old pixels are shown");
		assert!(source.has_pending(), "a rebake is expected for the stale slot");

		writer.write(uuid, 2, &tile_bytes(0x22)).expect("write v2");
		completions_tx
			.send(Completion { index: 0, ok: true })
			.expect("send completion");

		let (_, pixels) =
			drain_one(&mut source, Duration::from_secs(10)).expect("fresh tile delivered");
		assert!(pixels.iter().all(|&b| b == 0x22));
		assert!(!source.has_pending());
	}

	#[test]
	fn absent_slot_fills_after_the_bake_publishes() {
		let dir = tempfile::tempdir().expect("tempdir");
		let (mut source, entries_tx, completions_tx, path) = make_source(dir.path());
		let uuid = Uuid::from_u128(3);

		// Request before the cache file even exists: the placeholder stays up.
		entries_tx
			.send(vec![Entry { uuid, version: 9 }])
			.expect("send entries");
		source.poll();
		source.request(0);
		assert!(
			drain_one(&mut source, Duration::from_millis(200)).is_none(),
			"nothing to deliver before the bake"
		);
		assert!(source.has_pending());

		// The writer side publishes the slot and reports completion.
		let mut writer = Pvcache::open_or_create(&path, TILE, TILE).expect("create cache");
		writer.write(uuid, 9, &tile_bytes(0xCD)).expect("write");
		completions_tx
			.send(Completion { index: 0, ok: true })
			.expect("send completion");

		let (idx, pixels) =
			drain_one(&mut source, Duration::from_secs(10)).expect("tile delivered");
		assert_eq!(idx, 0);
		assert!(pixels.iter().all(|&b| b == 0xCD));
		assert!(!source.has_pending());
	}

	#[test]
	fn failed_bake_retires_the_cell_to_idle() {
		let dir = tempfile::tempdir().expect("tempdir");
		let (mut source, entries_tx, completions_tx, _path) = make_source(dir.path());

		entries_tx
			.send(vec![Entry {
				uuid: Uuid::from_u128(4),
				version: 1,
			}])
			.expect("send entries");
		source.poll();
		source.request(0);
		completions_tx
			.send(Completion { index: 0, ok: false })
			.expect("send completion");

		assert!(
			settle(&mut source, Duration::from_secs(10)),
			"failed cell settles to idle"
		);
		// A re-request for the dead cell is dropped rather than re-armed.
		source.request(0);
		assert!(!source.has_pending());
	}

	#[test]
	fn filler_exit_retires_missing_tiles() {
		let dir = tempfile::tempdir().expect("tempdir");
		let (mut source, entries_tx, completions_tx, _path) = make_source(dir.path());

		entries_tx
			.send(vec![Entry {
				uuid: Uuid::from_u128(5),
				version: 1,
			}])
			.expect("send entries");
		drop(completions_tx);
		source.poll();
		source.request(0);

		assert!(
			settle(&mut source, Duration::from_secs(10)),
			"cells settle once the writer side is gone"
		);
	}
}
