//! The pvcache-backed tile source — the grid's half of the cross-process
//! thumbnail contract.
//!
//! The source opens `thumbs.pvcache` with [`PvcacheReader`] (read-only mmap)
//! and never touches the writer handle: the daemon owns the writer, because
//! the format is single-writer by contract. Grid index → record uuid arrives
//! from the daemon in windows as the viewport moves, and bake completions
//! arrive as daemon events to re-read slots the grid is showing placeholders
//! or stale pixels for.
//!
//! Identities are sparse on purpose. A cell with no identity yet has nothing
//! to read, so it holds its placeholder until the window covering it lands.
//!
//! Slot outcomes map to the grid like this: `Fresh` and `Stale` both deliver
//! pixels (stale pixels are valid old content, shown while the rebake runs);
//! `Absent` keeps the placeholder up until the bake for that slot completes.
//! Every wait state settles — a failed bake or the fill ending retires its
//! cells — so [`TileSource::has_pending`] reaches false and the grid idles at
//! true 0% CPU.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::mpsc::{channel, Receiver, Sender, TryRecvError};
use std::thread;

use sd_pvcache::{PvcacheReader, TileState};
use uuid::Uuid;

use super::{Bitmap, TileSource, VisibleRange, TILE};

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
	Fresh(Bitmap),
	Stale(Bitmap),
	Absent,
}

pub struct PvcacheSource {
	/// Cell count, from the directory listing that opened this folder.
	len: u32,
	/// Identities for the cells the daemon has answered for, by grid index.
	entries: HashMap<u32, Entry>,
	/// Identity windows as the daemon answers them.
	entries_rx: Receiver<Vec<(u32, Entry)>>,
	completions_rx: Receiver<Completion>,
	/// The completion channel disconnected: the data plane is done and no
	/// further slots will be published for this folder.
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
	/// Open a source over the cache file at `path`, for a folder of `len`
	/// cells. `entries_rx` delivers index → `(uuid, version)` in windows as
	/// the viewport moves; `completions_rx` streams bake completions.
	pub fn new(
		path: PathBuf,
		len: u32,
		entries_rx: Receiver<Vec<(u32, Entry)>>,
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
			len,
			entries: HashMap::new(),
			entries_rx,
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
		if let Some(entry) = self.entries.get(&index) {
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
		loop {
			match self.entries_rx.try_recv() {
				Ok(window) => {
					for (index, entry) in window {
						// A cell whose identity changed under it (the file was
						// rewritten) drops any wait so the new version is read.
						if self.entries.insert(index, entry) != Some(entry) {
							self.waiting.remove(&index);
							self.stale_wait.remove(&index);
							self.failed.remove(&index);
							self.dead.remove(&index);
						}
					}
				}
				Err(TryRecvError::Disconnected) | Err(TryRecvError::Empty) => break,
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
		self.len
	}

	fn tile(&self) -> u32 {
		TILE
	}

	fn request(&mut self, idx: u32) {
		// A cell the daemon has not answered for yet has nothing to read; the
		// window covering it delivers its identity and the grid asks again.
		if !self.entries.contains_key(&idx)
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
		!self.waiting.is_empty() || !self.stale_wait.is_empty()
	}

	fn drain(&mut self, max: usize) -> Vec<(u32, Bitmap)> {
		let mut out = Vec::new();
		while out.len() < max {
			let Ok(delivery) = self.delivery_rx.try_recv() else {
				break;
			};
			let idx = delivery.index;
			match delivery.outcome {
				ReadOutcome::Fresh(bitmap) => {
					self.waiting.remove(&idx);
					self.stale_wait.remove(&idx);
					out.push((idx, bitmap));
				}
				ReadOutcome::Stale(bitmap) => {
					self.waiting.remove(&idx);
					if self.failed.contains(&idx) || self.filler_done {
						// No rebake is coming; the stale pixels are final for
						// this run.
						self.stale_wait.remove(&idx);
					} else {
						self.stale_wait.insert(idx);
					}
					out.push((idx, bitmap));
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
			Ok(TileState::Fresh { frame }) => return ReadOutcome::Fresh(crop(pixels, frame)),
			Ok(TileState::Stale { frame, .. }) => return ReadOutcome::Stale(crop(pixels, frame)),
			Ok(TileState::Absent) => return ReadOutcome::Absent,
			Err(_) => *reader = None,
		}
	}
	ReadOutcome::Absent
}

/// Trim an envelope-sized buffer to the frame the slot actually holds. The
/// tail was never written, so keeping it would upload padding to the atlas and
/// skew every row after the first.
fn crop(mut pixels: Vec<u8>, frame: sd_pvcache::Frame) -> Bitmap {
	pixels.truncate(frame.len());
	Bitmap {
		width: frame.content_width,
		height: frame.content_height,
		bgra: pixels,
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use sd_pvcache::Pvcache;
	use std::path::Path;
	use std::time::{Duration, Instant};

	/// A landscape frame inside the envelope, so the tests exercise the same
	/// packed layout the baker writes rather than a full square.
	const FRAME_H: u32 = TILE / 2;
	const FRAME: sd_pvcache::Frame = sd_pvcache::Frame {
		content_width: TILE,
		content_height: FRAME_H,
		source_width: 4032,
		source_height: 2016,
	};

	fn tile_bytes(b: u8) -> Vec<u8> {
		vec![b; (TILE * FRAME_H * 4) as usize]
	}

	/// Cell count for the test folder; the tests only ever address cell 0.
	const TEST_CELLS: u32 = 1;

	fn make_source(
		dir: &Path,
	) -> (
		PvcacheSource,
		Sender<Vec<(u32, Entry)>>,
		Sender<Completion>,
		PathBuf,
	) {
		let path = dir.join("thumbs.pvcache");
		let (entries_tx, entries_rx) = channel();
		let (completions_tx, completions_rx) = channel();
		let source = PvcacheSource::new(
			path.clone(),
			TEST_CELLS,
			entries_rx,
			completions_rx,
			VisibleRange::new(0, 64),
		);
		(source, entries_tx, completions_tx, path)
	}

	/// Poll and drain until one tile arrives or the deadline passes.
	fn drain_one(source: &mut PvcacheSource, deadline: Duration) -> Option<(u32, Bitmap)> {
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
		writer
			.write(uuid, 7, FRAME, &tile_bytes(0xAB))
			.expect("write");

		entries_tx
			.send(vec![(0, Entry { uuid, version: 7 })])
			.expect("send entries");
		source.poll();
		assert_eq!(source.len(), 1);

		source.request(0);
		let (idx, bitmap) =
			drain_one(&mut source, Duration::from_secs(10)).expect("tile delivered");
		assert_eq!(idx, 0);
		assert!(bitmap.bgra.iter().all(|&b| b == 0xAB));
		assert_eq!((bitmap.width, bitmap.height), (TILE, FRAME_H));
		assert!(!source.has_pending(), "fresh delivery settles the cell");
	}

	#[test]
	fn stale_pixels_show_then_refresh_on_completion() {
		let dir = tempfile::tempdir().expect("tempdir");
		let (mut source, entries_tx, completions_tx, path) = make_source(dir.path());
		let uuid = Uuid::from_u128(2);

		let mut writer = Pvcache::open_or_create(&path, TILE, TILE).expect("create cache");
		writer
			.write(uuid, 1, FRAME, &tile_bytes(0x11))
			.expect("write v1");

		entries_tx
			.send(vec![(0, Entry { uuid, version: 2 })])
			.expect("send entries");
		source.poll();
		source.request(0);

		let (_, bitmap) =
			drain_one(&mut source, Duration::from_secs(10)).expect("stale tile delivered");
		assert!(
			bitmap.bgra.iter().all(|&b| b == 0x11),
			"old pixels are shown"
		);
		assert!(
			source.has_pending(),
			"a rebake is expected for the stale slot"
		);

		writer
			.write(uuid, 2, FRAME, &tile_bytes(0x22))
			.expect("write v2");
		completions_tx
			.send(Completion { index: 0, ok: true })
			.expect("send completion");

		let (_, bitmap) =
			drain_one(&mut source, Duration::from_secs(10)).expect("fresh tile delivered");
		assert!(bitmap.bgra.iter().all(|&b| b == 0x22));
		assert!(!source.has_pending());
	}

	#[test]
	fn absent_slot_fills_after_the_bake_publishes() {
		let dir = tempfile::tempdir().expect("tempdir");
		let (mut source, entries_tx, completions_tx, path) = make_source(dir.path());
		let uuid = Uuid::from_u128(3);

		// Request before the cache file even exists: the placeholder stays up.
		entries_tx
			.send(vec![(0, Entry { uuid, version: 9 })])
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
		writer
			.write(uuid, 9, FRAME, &tile_bytes(0xCD))
			.expect("write");
		completions_tx
			.send(Completion { index: 0, ok: true })
			.expect("send completion");

		let (idx, bitmap) =
			drain_one(&mut source, Duration::from_secs(10)).expect("tile delivered");
		assert_eq!(idx, 0);
		assert!(bitmap.bgra.iter().all(|&b| b == 0xCD));
		assert!(!source.has_pending());
	}

	#[test]
	fn failed_bake_retires_the_cell_to_idle() {
		let dir = tempfile::tempdir().expect("tempdir");
		let (mut source, entries_tx, completions_tx, _path) = make_source(dir.path());

		entries_tx
			.send(vec![(
				0,
				Entry {
					uuid: Uuid::from_u128(4),
					version: 1,
				},
			)])
			.expect("send entries");
		source.poll();
		source.request(0);
		completions_tx
			.send(Completion {
				index: 0,
				ok: false,
			})
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
			.send(vec![(
				0,
				Entry {
					uuid: Uuid::from_u128(5),
					version: 1,
				},
			)])
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
