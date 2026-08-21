//! Tile sources: where the grid's cells come from.
//!
//! The grid is a pure consumer — it culls, paints, and evicts; a [`TileSource`]
//! decides what a cell index means and produces its pixels off the UI thread.
//! Two sources exist: [`SyntheticSource`] generates procedural tiles for
//! benchmarking, and [`PvcacheSource`] reads real thumbnails out of a
//! `thumbs.pvcache` file through the cross-process reader contract.

mod pvcache;
mod synthetic;

pub use pvcache::{Completion, Entry, PvcacheSource};
pub use synthetic::SyntheticSource;

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

/// Tile edge for every source, in physical pixels. Also the geometry of the
/// demo pvcache file; the reader rejects a file with any other geometry.
pub const TILE: u32 = 256;

/// Feeds the grid: cell count, pixels, and priority hints. Methods are called
/// on the UI thread from the grid's paint pass; implementations do their real
/// work on background threads and hand results back through
/// [`TileSource::drain`].
pub trait TileSource {
	/// Absorb background progress (walk results, bake completions). Called
	/// once at the top of each paint, before anything reads [`Self::len`].
	fn poll(&mut self);

	/// Total cell count. May grow after construction (a folder walk landing).
	fn len(&self) -> u32;

	/// Tile edge in physical pixels.
	fn tile(&self) -> u32;

	/// Ask for the tile at `idx`. Idempotent while a request is outstanding,
	/// so the grid can call it every frame for unfilled visible cells.
	fn request(&mut self, idx: u32);

	/// Whether tiles are still expected. This drives the grid's frame
	/// watchdog, so it must settle to false once nothing more will arrive —
	/// that is what keeps the app at 0% CPU when idle.
	fn has_pending(&self) -> bool;

	/// Up to `max` finished tiles as tight BGRA8 buffers. A tile may arrive
	/// more than once for the same index (stale pixels first, the rebake
	/// after); the latest delivery wins.
	fn drain(&mut self, max: usize) -> Vec<(u32, Vec<u8>)>;

	/// The cell range currently on screen (`last` exclusive), for bake
	/// prioritization.
	fn set_visible_range(&mut self, first: u32, last: u32);
}

/// The visible cell range shared between the UI thread (writer) and bake
/// scheduling (reader), packed into one atomic word so neither side locks.
#[derive(Clone)]
pub struct VisibleRange(Arc<AtomicU64>);

impl VisibleRange {
	pub fn new(first: u32, last: u32) -> Self {
		let range = Self(Arc::new(AtomicU64::new(0)));
		range.set(first, last);
		range
	}

	pub fn set(&self, first: u32, last: u32) {
		self.0
			.store(u64::from(first) << 32 | u64::from(last), Ordering::Relaxed);
	}

	/// `(first, last)`, `last` exclusive.
	pub fn get(&self) -> (u32, u32) {
		let packed = self.0.load(Ordering::Relaxed);
		((packed >> 32) as u32, packed as u32)
	}
}

/// Worker count for a source's background pool: every core, matching the
/// reference renderer's bake saturation.
pub fn num_workers() -> usize {
	std::thread::available_parallelism()
		.map(|n| n.get())
		.unwrap_or(4)
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn visible_range_roundtrips() {
		let range = VisibleRange::new(3, 17);
		assert_eq!(range.get(), (3, 17));
		range.set(120_000, 129_000);
		assert_eq!(range.get(), (120_000, 129_000));
		range.set(0, 0);
		assert_eq!(range.get(), (0, 0));
	}
}
