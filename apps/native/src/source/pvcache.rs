//! The pvcache-backed tile source — the grid's half of the cross-process
//! thumbnail contract.
//!
//! The source opens each drive's `thumbs.pvcache` with [`PvcacheReader`]
//! (read-only mmap) and never touches a writer handle: the daemon owns the
//! writers, because the format is single-writer by contract. Grid index →
//! record uuid arrives from the daemon in windows as the viewport moves, each
//! identity naming the cache its tile lives in, since a listing can span
//! drives. Bake completions arrive as daemon events to re-read slots the grid
//! is showing placeholders or stale pixels for.
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
use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver, Sender, TryRecvError};
use std::sync::Arc;
use std::thread;

use sd_pvcache::{PvcacheReader, TileState};
use uuid::Uuid;

use super::{Bitmap, TileSource, VisibleRange, TILE};

/// Completions absorbed per poll, bounding the per-frame cost of a large
/// background fill finishing while the grid sits still.
const COMPLETIONS_PER_POLL: usize = 4096;

/// One grid cell's identity: the cache its tile lives in, the record uuid,
/// and the content version the tile must match to be fresh.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Entry {
	pub cache: Uuid,
	pub uuid: Uuid,
	pub version: u64,
}

/// What the data plane tells the source about the listing it shows.
pub enum Feed {
	/// A later page landed; the listing now has this many cells.
	Grew(u32),
	/// Identities for a window of cells, with the file behind each cache
	/// they name.
	Window {
		caches: Vec<(Uuid, PathBuf)>,
		entries: Vec<(u32, Entry)>,
	},
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
	cache: Arc<Path>,
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
	/// Cell count, from the pages of the listing landed so far.
	len: u32,
	/// Identities for the cells the daemon has answered for, by grid index.
	entries: HashMap<u32, Entry>,
	/// The file behind every cache an identity has named.
	caches: HashMap<Uuid, Arc<Path>>,
	/// Growth and identity windows, as the plane learns them.
	feed_rx: Receiver<Feed>,
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
	/// Open a source for a listing of `len` cells. `feed_rx` delivers growth
	/// and index → identity windows as pages land and the viewport moves;
	/// `completions_rx` streams bake completions.
	pub fn new(
		len: u32,
		feed_rx: Receiver<Feed>,
		completions_rx: Receiver<Completion>,
		visible: VisibleRange,
	) -> Self {
		let (read_tx, read_rx) = channel();
		let (delivery_tx, delivery_rx) = channel();
		thread::Builder::new()
			.name("pvcache-read".into())
			.spawn(move || run_reader(&read_rx, &delivery_tx))
			.expect("failed to spawn pvcache read thread");
		Self {
			len,
			entries: HashMap::new(),
			caches: HashMap::new(),
			feed_rx,
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

	/// The cache file behind the cell at `index`, once its identity and the
	/// cache it names are both known.
	fn cache_of(&self, index: u32) -> Option<(&Entry, &Arc<Path>)> {
		let entry = self.entries.get(&index)?;
		Some((entry, self.caches.get(&entry.cache)?))
	}

	fn send_read(&self, index: u32) {
		if let Some((entry, cache)) = self.cache_of(index) {
			let _ = self.read_tx.send(ReadRequest {
				index,
				cache: cache.clone(),
				uuid: entry.uuid,
				version: entry.version,
			});
		}
	}
}

impl TileSource for PvcacheSource {
	fn poll(&mut self) {
		loop {
			match self.feed_rx.try_recv() {
				Ok(Feed::Grew(len)) => self.len = len,
				Ok(Feed::Window { caches, entries }) => {
					for (id, path) in caches {
						self.caches.entry(id).or_insert_with(|| Arc::from(path));
					}
					for (index, entry) in entries {
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
		if self.cache_of(idx).is_none()
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
/// each cache's read-only mapping, opened the first time a request names it.
/// Exits when the source (and its request sender) drop.
fn run_reader(rx: &Receiver<ReadRequest>, tx: &Sender<Delivery>) {
	let tile_len = (TILE * TILE * 4) as usize;
	let mut readers: HashMap<Arc<Path>, Option<PvcacheReader>> = HashMap::new();
	while let Ok(request) = rx.recv() {
		let reader = readers.entry(request.cache.clone()).or_default();
		let outcome = read_slot(&request.cache, reader, tile_len, &request);
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
	path: &Path,
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

	/// Cell count for the test folder; most tests only ever address cell 0.
	const TEST_CELLS: u32 = 1;

	/// The cache the test folder's tiles live in.
	const CACHE: Uuid = Uuid::from_u128(0xCAC4E);

	fn make_source(dir: &Path) -> (PvcacheSource, Sender<Feed>, Sender<Completion>, PathBuf) {
		let path = dir.join("thumbs.pvcache");
		let (feed_tx, feed_rx) = channel();
		let (completions_tx, completions_rx) = channel();
		let source = PvcacheSource::new(
			TEST_CELLS,
			feed_rx,
			completions_rx,
			VisibleRange::new(0, 64),
		);
		(source, feed_tx, completions_tx, path)
	}

	/// The identity window naming cell `index` as `uuid` at `version`, in the
	/// cache file at `path`.
	fn window(path: &Path, index: u32, uuid: Uuid, version: u64) -> Feed {
		Feed::Window {
			caches: vec![(CACHE, path.to_path_buf())],
			entries: vec![(
				index,
				Entry {
					cache: CACHE,
					uuid,
					version,
				},
			)],
		}
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
		let (mut source, feed_tx, _completions_tx, path) = make_source(dir.path());
		let uuid = Uuid::from_u128(1);

		let mut writer = Pvcache::open_or_create(&path, TILE, TILE).expect("create cache");
		writer
			.write(uuid, 7, FRAME, &tile_bytes(0xAB))
			.expect("write");

		feed_tx
			.send(window(&path, 0, uuid, 7))
			.expect("send window");
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
		let (mut source, feed_tx, completions_tx, path) = make_source(dir.path());
		let uuid = Uuid::from_u128(2);

		let mut writer = Pvcache::open_or_create(&path, TILE, TILE).expect("create cache");
		writer
			.write(uuid, 1, FRAME, &tile_bytes(0x11))
			.expect("write v1");

		feed_tx
			.send(window(&path, 0, uuid, 2))
			.expect("send window");
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
		let (mut source, feed_tx, completions_tx, path) = make_source(dir.path());
		let uuid = Uuid::from_u128(3);

		// Request before the cache file even exists: the placeholder stays up.
		feed_tx
			.send(window(&path, 0, uuid, 9))
			.expect("send window");
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
		let (mut source, feed_tx, completions_tx, path) = make_source(dir.path());

		feed_tx
			.send(window(&path, 0, Uuid::from_u128(4), 1))
			.expect("send window");
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
		let (mut source, feed_tx, completions_tx, path) = make_source(dir.path());

		feed_tx
			.send(window(&path, 0, Uuid::from_u128(5), 1))
			.expect("send window");
		drop(completions_tx);
		source.poll();
		source.request(0);

		assert!(
			settle(&mut source, Duration::from_secs(10)),
			"cells settle once the writer side is gone"
		);
	}

	/// Cells read from the cache their identity names, so a listing that
	/// spans drives fills from each drive's file.
	#[test]
	fn each_cell_reads_from_the_cache_its_identity_names() {
		let dir = tempfile::tempdir().expect("tempdir");
		let (mut source, feed_tx, _completions_tx, first_path) = make_source(dir.path());
		let second_path = dir.path().join("second.pvcache");
		let second_cache = Uuid::from_u128(0x5EC0D);
		let (first, second) = (Uuid::from_u128(6), Uuid::from_u128(7));

		let mut first_writer =
			Pvcache::open_or_create(&first_path, TILE, TILE).expect("first cache");
		first_writer
			.write(first, 1, FRAME, &tile_bytes(0x0A))
			.expect("write first");
		let mut second_writer =
			Pvcache::open_or_create(&second_path, TILE, TILE).expect("second cache");
		second_writer
			.write(second, 1, FRAME, &tile_bytes(0x0B))
			.expect("write second");

		feed_tx.send(Feed::Grew(2)).expect("send growth");
		feed_tx
			.send(Feed::Window {
				caches: vec![(CACHE, first_path), (second_cache, second_path)],
				entries: vec![
					(
						0,
						Entry {
							cache: CACHE,
							uuid: first,
							version: 1,
						},
					),
					(
						1,
						Entry {
							cache: second_cache,
							uuid: second,
							version: 1,
						},
					),
				],
			})
			.expect("send window");
		source.poll();
		assert_eq!(source.len(), 2, "the growth landed");

		source.request(0);
		source.request(1);
		let deadline = Instant::now() + Duration::from_secs(10);
		let mut delivered = Vec::new();
		while delivered.len() < 2 && Instant::now() < deadline {
			source.poll();
			delivered.extend(
				source
					.drain(usize::MAX)
					.into_iter()
					.map(|(index, bitmap)| (index, bitmap.bgra[0])),
			);
			thread::sleep(Duration::from_millis(5));
		}
		delivered.sort_unstable();
		assert_eq!(delivered, [(0, 0x0A), (1, 0x0B)]);
	}
}
