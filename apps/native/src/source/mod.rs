//! Tile sources: where the grid's cells come from.
//!
//! The grid is a pure consumer — it culls, paints, and evicts; a [`TileSource`]
//! decides what a cell index means and produces its pixels off the UI thread.
//! Three sources exist: [`PvcacheSource`] reads real thumbnails out of a
//! `thumbs.pvcache` file through the cross-process reader contract,
//! [`EmptySource`] stands in for a window with no folder, and
//! [`SyntheticSource`] generates procedural tiles for the benchmark only.

mod empty;
mod pvcache;
mod synthetic;

pub use empty::EmptySource;
pub use pvcache::{Completion, Entry, Feed, PvcacheSource};
pub use synthetic::SyntheticSource;

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

/// Envelope edge for every source, in physical pixels: the largest frame a
/// tile can occupy on either axis. The daemon bakes into this geometry and the
/// reader rejects a cache file with any other, so the constant is taken from
/// the writer rather than restated here.
pub use sd_core::service::thumbs::TILE;

/// A finished tile: tight BGRA8 at the image's own proportions, which is what
/// the grid draws. Dimensions are the frame's, never the envelope's — a
/// landscape photo arrives wider than it is tall.
pub struct Bitmap {
	pub width: u32,
	pub height: u32,
	pub bgra: Vec<u8>,
}

impl Bitmap {
	/// A square bitmap filling the envelope, which is what the synthetic and
	/// icon sources produce.
	pub fn square(edge: u32, bgra: Vec<u8>) -> Self {
		Self {
			width: edge,
			height: edge,
			bgra,
		}
	}
}

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

	/// Envelope edge in physical pixels: the buffer size a reader allocates,
	/// and the ceiling on any delivered [`Bitmap`].
	#[cfg_attr(not(test), allow(dead_code))]
	fn tile(&self) -> u32;

	/// Ask for the tile at `idx`. Idempotent while a request is outstanding,
	/// so the grid can call it every frame for unfilled visible cells.
	fn request(&mut self, idx: u32);

	/// Whether tiles are still expected. This drives the grid's frame
	/// watchdog, so it must settle to false once nothing more will arrive —
	/// that is what keeps the app at 0% CPU when idle.
	fn has_pending(&self) -> bool;

	/// Up to `max` finished tiles. A tile may arrive more than once for the
	/// same index (stale pixels first, the rebake after); the latest delivery
	/// wins.
	fn drain(&mut self, max: usize) -> Vec<(u32, Bitmap)>;

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
