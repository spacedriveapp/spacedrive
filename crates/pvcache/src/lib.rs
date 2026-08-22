//! Memory-mapped thumbnail tile cache — the hot tier behind `thumbs.pvcache`.
//!
//! One cache file holds fixed-size BGRA8 tiles keyed by record uuid. The
//! daemon owns the single writer handle ([`Pvcache`]); any number of other
//! processes map the same file read-only ([`PvcacheReader`]) and copy tiles
//! straight out of the mapping — a GPU client uploads them to its texture
//! atlas with zero decode on the hot path. The crate takes a caller-supplied
//! path (one cache file per source, e.g. `sources/<id>/thumbs.pvcache`) and
//! knows nothing about the store layout around it.
//!
//! # File layout
//!
//! All fields little-endian:
//!
//! ```text
//! [ header ]  4096 bytes reserved — magic, format version, envelope
//!             geometry, plus capacity / slot count / generation, updated
//!             atomically
//! [ slot 0 ]  64-byte record header + BGRA8 pixels, padded to 64 bytes
//! [ slot 1 ]  ...
//! ```
//!
//! Slots are allocated append-only in first-write order. Each record header
//! carries the entry uuid, a caller-supplied content version (derived from the
//! source file's size and mtime at bake time, via `sd_core::infra::source_version`), the
//! [`Frame`] describing what the slot actually holds, and a sequence word. The
//! file is self-describing: the uuid → slot index is rebuilt by scanning record
//! headers at open, so it persists across reopen without a separate index
//! structure.
//!
//! # Envelope and frame
//!
//! A slot's geometry is an *envelope*, not the tile's shape. An aspect-fit tile
//! occupies a [`Frame`] no larger than the envelope on either axis, packed
//! tight at `content_width * 4` bytes per row from the start of the slot's
//! pixels. The remainder of the slot is never written, so a landscape tile
//! leaves its tail sparse and costs no disk. The frame travels with every read,
//! so a caller always knows the valid extent and row stride.
//!
//! # Publish protocol
//!
//! Readers in other processes must never observe torn tiles, including under
//! rebake of an existing slot, so every record is guarded by a seqlock:
//!
//! - The writer bumps the slot's sequence word to an odd value, writes the
//!   record fields and pixels, then bumps it back to even with release
//!   ordering.
//! - A reader samples the word (acquire), skips the slot while odd, copies
//!   fields and pixels, then re-checks the word; any change means the copy
//!   raced a rewrite and is retried.
//!
//! A newly allocated slot becomes visible only through the header's slot
//! count, stored with release ordering after the record's first write
//! completes — so a record below the published count always holds a settled
//! uuid, and the key never changes for the life of the file.
//!
//! # Growth
//!
//! When full, capacity doubles: the file is extended with `set_len` (sparse
//! where the filesystem supports it, so unwritten slots cost no disk), the
//! writer remaps, and the header's capacity and generation are published.
//! Existing slots never move, so a reader's older mapping stays valid for the
//! slots it covers; on a generation change the reader remaps to see the rest.
//!
//! # Durability
//!
//! The cache is disposable by design — every tile can be rebaked from its
//! source. A file that fails validation at open (truncated, bad magic,
//! unsupported format version) is reinitialized by the writer and reported
//! absent by the reader; a record left mid-write by a crash is retired at the
//! writer's next open. No checksums, no journaling.

mod error;
mod layout;
mod reader;
mod writer;

pub use error::{Error, Result};
pub use reader::PvcacheReader;
pub use writer::{Pvcache, DEFAULT_INITIAL_CAPACITY};

/// What a slot holds: the extent of its pixels, and the dimensions of the
/// source they were baked from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Frame {
	/// Extent of the stored image within the slot's envelope. Rows are packed
	/// tight at `content_width * 4` bytes.
	pub content_width: u32,
	pub content_height: u32,
	/// Dimensions of the file the tile was baked from, so a view can size a
	/// cell before it has read the tile — or before one exists.
	pub source_width: u32,
	pub source_height: u32,
}

impl Frame {
	/// Bytes of BGRA8 the stored image occupies.
	pub fn len(&self) -> usize {
		self.content_width as usize * self.content_height as usize * 4
	}

	pub fn is_empty(&self) -> bool {
		self.content_width == 0 || self.content_height == 0
	}

	/// Row stride in bytes.
	pub fn row_stride(&self) -> usize {
		self.content_width as usize * 4
	}
}

/// Outcome of a tile lookup for a `(uuid, expected_version)` pair.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TileState {
	/// The stored version matches the expected version; the tile is current.
	Fresh { frame: Frame },
	/// A tile exists but was baked at a different version. Calls that copy
	/// pixels still fill the buffer, so the stale tile can be shown while the
	/// caller rebakes.
	Stale { version: u64, frame: Frame },
	/// No tile for this uuid (never baked, mid-first-write, or the record was
	/// retired after a crash).
	Absent,
}

impl TileState {
	/// The frame behind a hit, or `None` when the slot holds nothing.
	pub fn frame(&self) -> Option<Frame> {
		match self {
			Self::Fresh { frame } | Self::Stale { frame, .. } => Some(*frame),
			Self::Absent => None,
		}
	}
}
