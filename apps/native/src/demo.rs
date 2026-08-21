//! The in-process demo bake: stands in for the daemon's producer chain until
//! the hot tier is served for real (phase A3). Given a folder, it walks for
//! images, keys each file by a deterministic uuid (v5 of its absolute path)
//! with a version from `(size, mtime)`, and drives an `sd-bake` pool filling
//! the `Pvcache` writer — skipping slots that are already fresh, viewport
//! first. The grid never sees any of this: it reads the same file through
//! `PvcacheReader`, exactly as it will when the daemon owns the writer.
//!
//! Launch: `spacedrive-native <folder>` or `SD_NATIVE_FOLDER=<folder>`. The
//! cache lives at `~/Library/Caches/Spacedrive/native-demo/thumbs.pvcache`
//! (`SD_NATIVE_CACHE` overrides), and `SD_NATIVE_MAX_FILES` caps the walk.

use std::collections::{HashSet, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver, RecvTimeoutError, Sender};
use std::thread;
use std::time::{Duration, Instant, UNIX_EPOCH};

use sd_bake::{BakePool, BakeRequest, ImageProducer, Producer, ScaleMode, WorkItem};
use sd_core::infra::source_version::source_version;
use sd_pvcache::{Pvcache, DEFAULT_INITIAL_CAPACITY};
use uuid::Uuid;

use crate::source::{Completion, Entry, VisibleRange, TILE};

/// Namespace for demo record uuids; arbitrary but fixed, so a path maps to
/// the same uuid on every launch and the cache persists across runs. The
/// daemon replaces these with real record uuids when it takes over the writer.
const DEMO_NAMESPACE: Uuid = Uuid::from_u128(0xd5a3_1c3e_7f04_4bde_9b1a_2f60_c8f1_4e2d);

/// Extensions the demo walk treats as images. Decoding still goes through the
/// producer chain, which sniffs content, so this is a walk filter, not a
/// format contract.
const IMAGE_EXTENSIONS: &[&str] = &[
	"avif", "bmp", "gif", "heic", "heif", "jpeg", "jpg", "png", "tif", "tiff", "webp",
];

/// How long the fill loop waits for a bake before re-reading the visible
/// range, so scrolling reprioritizes even while every worker is busy.
const REPRIORITIZE_INTERVAL: Duration = Duration::from_millis(50);

/// The receiving ends the grid's `PvcacheSource` consumes.
pub struct DemoFill {
	pub entries_rx: Receiver<Vec<Entry>>,
	pub completions_rx: Receiver<Completion>,
}

/// The folder to browse: the first CLI argument, or `SD_NATIVE_FOLDER`.
pub fn folder_from_env() -> Option<PathBuf> {
	std::env::args_os()
		.nth(1)
		.map(PathBuf::from)
		.or_else(|| std::env::var_os("SD_NATIVE_FOLDER").map(PathBuf::from))
}

/// Where the demo cache file lives. `SD_NATIVE_CACHE` overrides for tests and
/// benchmarks that need an isolated cache.
pub fn cache_path() -> PathBuf {
	if let Some(path) = std::env::var_os("SD_NATIVE_CACHE") {
		return PathBuf::from(path);
	}
	std::env::home_dir()
		.map(|home| home.join("Library/Caches/Spacedrive/native-demo/thumbs.pvcache"))
		.unwrap_or_else(|| PathBuf::from("thumbs.pvcache"))
}

/// Walk cap from `SD_NATIVE_MAX_FILES`; no cap by default.
pub fn max_files_from_env() -> Option<usize> {
	std::env::var("SD_NATIVE_MAX_FILES")
		.ok()
		.and_then(|v| v.parse().ok())
}

/// Deterministic record uuid for a demo file: v5 over the path bytes, so the
/// same file resolves to the same cache slot on every launch.
pub fn uuid_for_path(path: &Path) -> Uuid {
	Uuid::new_v5(&DEMO_NAMESPACE, path.as_os_str().as_encoded_bytes())
}

/// Spawn the fill thread: walk `folder`, deliver the entry list, then bake
/// every non-fresh slot into the cache at `cache_path`, visible cells first.
pub fn spawn(
	folder: PathBuf,
	cache_path: PathBuf,
	visible: VisibleRange,
	max_files: Option<usize>,
) -> DemoFill {
	let (entries_tx, entries_rx) = channel();
	let (completions_tx, completions_rx) = channel();
	thread::Builder::new()
		.name("demo-fill".into())
		.spawn(move || {
			run_fill(
				&folder,
				&cache_path,
				&visible,
				max_files,
				&entries_tx,
				&completions_tx,
			);
		})
		.expect("failed to spawn demo-fill thread");
	DemoFill {
		entries_rx,
		completions_rx,
	}
}

struct DemoEntry {
	path: PathBuf,
	entry: Entry,
}

fn run_fill(
	folder: &Path,
	cache_path: &Path,
	visible: &VisibleRange,
	max_files: Option<usize>,
	entries_tx: &Sender<Vec<Entry>>,
	completions_tx: &Sender<Completion>,
) {
	let started = Instant::now();
	let files = walk(folder, max_files);
	let total = files.len();
	let _ = entries_tx.send(files.iter().map(|f| f.entry).collect());

	let mut writer = match Pvcache::open_or_create_with_capacity(
		cache_path,
		TILE,
		TILE,
		(total as u64).max(DEFAULT_INITIAL_CAPACITY),
	) {
		Ok(writer) => writer,
		Err(e) => {
			eprintln!("demo-fill: cannot open {}: {e}", cache_path.display());
			return;
		}
	};

	// Fresh slots (same uuid, same content version) survive from earlier runs
	// and need no work — this is what makes a relaunch instant.
	let mut remaining: HashSet<u32> = (0..total as u32)
		.filter(|&idx| {
			let entry = &files[idx as usize].entry;
			writer.lookup(entry.uuid, entry.version) != sd_pvcache::TileState::Fresh
		})
		.collect();
	let mut order: VecDeque<u32> = (0..total as u32).filter(|i| remaining.contains(i)).collect();
	eprintln!(
		"demo-fill: {total} images under {} | {} fresh in cache | baking {} ({:.0?} to scan)",
		folder.display(),
		total - remaining.len(),
		remaining.len(),
		started.elapsed(),
	);
	if remaining.is_empty() {
		return;
	}

	let workers = bake_workers();
	let (pool, baked_rx) = BakePool::<u32>::new(producer_chain(), TILE, workers);
	// Keep the pool's queue shallow: work is handed over in small doses so a
	// scroll can redirect the very next submissions at the new viewport.
	let max_in_flight = workers * 2;
	let mut outstanding = 0usize;
	let mut baked = 0usize;
	let mut failed = 0usize;
	let bake_started = Instant::now();

	while !remaining.is_empty() || outstanding > 0 {
		let (first, last) = visible.get();
		for idx in first..last.min(total as u32) {
			if outstanding >= max_in_flight {
				break;
			}
			if remaining.remove(&idx) {
				submit(&pool, &files, idx, 1);
				outstanding += 1;
			}
		}
		while outstanding < max_in_flight {
			let Some(idx) = order.pop_front() else {
				break;
			};
			// Entries plucked by the viewport pass above stay in `order` as
			// husks; membership in `remaining` is the truth.
			if remaining.remove(&idx) {
				submit(&pool, &files, idx, 0);
				outstanding += 1;
			}
		}

		match baked_rx.recv_timeout(REPRIORITIZE_INTERVAL) {
			Ok(done) => {
				outstanding -= 1;
				let file = &files[done.key as usize];
				let ok = match done.result {
					Ok(tile) => match writer.write(file.entry.uuid, file.entry.version, tile.bgra())
					{
						Ok(()) => true,
						Err(e) => {
							eprintln!("demo-fill: write {}: {e}", file.path.display());
							false
						}
					},
					Err(declines) => {
						eprintln!(
							"demo-fill: no producer for {}: {declines:?}",
							file.path.display()
						);
						false
					}
				};
				if ok {
					baked += 1;
				} else {
					failed += 1;
				}
				if completions_tx
					.send(Completion {
						index: done.key,
						ok,
					})
					.is_err()
				{
					// The grid is gone; the app is shutting down.
					return;
				}
			}
			Err(RecvTimeoutError::Timeout) => {}
			Err(RecvTimeoutError::Disconnected) => break,
		}
	}

	let _ = writer.flush();
	let elapsed = bake_started.elapsed();
	eprintln!(
		"demo-fill: baked {baked} tiles ({failed} failed) in {elapsed:.2?} ({:.0} tiles/s)",
		baked as f64 / elapsed.as_secs_f64().max(f64::EPSILON),
	);
}

fn submit(pool: &BakePool<u32>, files: &[DemoEntry], idx: u32, priority: u32) {
	pool.submit(BakeRequest {
		key: idx,
		item: WorkItem::file(&files[idx as usize].path),
		priority,
	});
}

/// The demo producer chain: raster decode, then (on macOS) the platform icon
/// tile so undecodable files still get a real cell instead of a placeholder.
fn producer_chain() -> Vec<Box<dyn Producer>> {
	let mut chain: Vec<Box<dyn Producer>> =
		vec![Box::new(ImageProducer::new(ScaleMode::Cover))];
	#[cfg(target_os = "macos")]
	chain.push(Box::new(sd_bake::IconProducer::new()));
	chain
}

/// Decode workers: leave headroom for the UI and compositor.
fn bake_workers() -> usize {
	std::thread::available_parallelism()
		.map(|n| n.get())
		.unwrap_or(4)
		.saturating_sub(2)
		.clamp(2, 8)
}

/// Collect image files under `root`, recursively, in a deterministic order
/// (each directory visited name-sorted) so grid indices are stable across
/// launches. Hidden entries are skipped; symlinks are not followed.
fn walk(root: &Path, cap: Option<usize>) -> Vec<DemoEntry> {
	let mut out = Vec::new();
	walk_dir(root, cap, &mut out);
	out
}

fn walk_dir(dir: &Path, cap: Option<usize>, out: &mut Vec<DemoEntry>) {
	let Ok(read) = std::fs::read_dir(dir) else {
		return;
	};
	let mut children: Vec<_> = read.flatten().collect();
	children.sort_by_key(|entry| entry.file_name());
	for child in children {
		if cap.is_some_and(|c| out.len() >= c) {
			return;
		}
		if child.file_name().as_encoded_bytes().starts_with(b".") {
			continue;
		}
		let Ok(file_type) = child.file_type() else {
			continue;
		};
		let path = child.path();
		if file_type.is_dir() {
			walk_dir(&path, cap, out);
		} else if file_type.is_file() && is_image(&path) {
			let Ok(meta) = child.metadata() else {
				continue;
			};
			let mtime = meta.modified().unwrap_or(UNIX_EPOCH);
			out.push(DemoEntry {
				entry: Entry {
					uuid: uuid_for_path(&path),
					version: source_version(meta.len(), mtime),
				},
				path,
			});
		}
	}
}

fn is_image(path: &Path) -> bool {
	path.extension()
		.and_then(|ext| ext.to_str())
		.is_some_and(|ext| {
			IMAGE_EXTENSIONS
				.iter()
				.any(|known| ext.eq_ignore_ascii_case(known))
		})
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn uuid_for_path_is_deterministic_and_distinct() {
		let a = uuid_for_path(Path::new("/photos/a.jpg"));
		assert_eq!(a, uuid_for_path(Path::new("/photos/a.jpg")));
		assert_ne!(a, uuid_for_path(Path::new("/photos/b.jpg")));
		assert_ne!(a, uuid_for_path(Path::new("/other/a.jpg")));
	}

	#[test]
	fn walk_finds_images_recursively_in_stable_order() {
		let dir = tempfile::tempdir().expect("tempdir");
		let root = dir.path();
		std::fs::create_dir(root.join("sub")).expect("mkdir");
		std::fs::create_dir(root.join(".git")).expect("mkdir");
		for name in [
			"a.jpg",
			"b.PNG",
			"notes.txt",
			".hidden.png",
			"sub/c.jpeg",
			".git/d.png",
		] {
			std::fs::write(root.join(name), b"x").expect("write");
		}

		let entries = walk(root, None);
		let names: Vec<_> = entries
			.iter()
			.map(|e| e.path.strip_prefix(root).unwrap().to_str().unwrap())
			.collect();
		assert_eq!(names, ["a.jpg", "b.PNG", "sub/c.jpeg"]);

		// Distinct paths get distinct uuids; the version reflects the file.
		assert_ne!(entries[0].entry.uuid, entries[1].entry.uuid);
		assert_eq!(entries[0].entry.uuid, uuid_for_path(&entries[0].path));

		let capped = walk(root, Some(2));
		let capped: Vec<_> = capped
			.iter()
			.map(|e| e.path.strip_prefix(root).unwrap().to_str().unwrap())
			.collect();
		assert_eq!(capped, ["a.jpg", "b.PNG"]);
	}

	#[test]
	fn version_tracks_file_changes() {
		let dir = tempfile::tempdir().expect("tempdir");
		let file = dir.path().join("img.png");
		std::fs::write(&file, b"one").expect("write");
		let before = walk(dir.path(), None)[0].entry;

		std::fs::write(&file, b"grown").expect("write");
		let after = walk(dir.path(), None)[0].entry;

		assert_eq!(before.uuid, after.uuid, "identity is the path");
		assert_ne!(before.version, after.version, "content change flips the version");
	}
}
