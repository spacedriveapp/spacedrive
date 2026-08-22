//! Cell renderers: bake a work item into a BGRA8 tile — the byte layout the
//! thumbnail hot tier stores and a GPU atlas uploads without decoding on the
//! render path.
//!
//! `cell` is an envelope, not a shape: an aspect-fit tile comes back at its own
//! proportions with its long edge no larger than the cell, so the tile carries
//! the dimensions a caller needs to store and draw it.
//!
//! A [`Producer`] is one stage of a fall-through chain ordered by cost: each
//! stage either yields a [`Tile`] or returns a typed [`Decline`], and the
//! caller moves on to the next stage. The full chain shape is sidecar decode →
//! platform decode → FFmpeg → icon tile; this crate carries the raster decode
//! stage ([`ImageProducer`]) and, on macOS, the icon/text tile stage
//! ([`IconProducer`]). [`BakePool`] drives a chain from a small worker pool,
//! completing higher-priority requests (the viewport) first.
//!
//! The crate knows nothing about where tiles are stored: the contract is a
//! tight BGRA8 byte buffer plus dimensions, which is exactly what a
//! `thumbs.pvcache` slot holds.

mod pool;
mod raster;

#[cfg(target_os = "macos")]
mod mac_icons;

use std::path::PathBuf;

pub use pool::{BakePool, BakeRequest, Baked};
pub use raster::{ImageProducer, ScaleMode};

#[cfg(target_os = "macos")]
pub use mac_icons::{
	name_width_frac, render_file_cell, render_file_icon, IconKey, IconProducer, NAME_BAND_Y0,
	NAME_BAND_Y1,
};

/// A finished cell: a tight BGRA8 pixel buffer, its dimensions, and the
/// dimensions of the source it depicts.
#[derive(Clone)]
pub struct Tile {
	width: u32,
	height: u32,
	source_width: u32,
	source_height: u32,
	bgra: Vec<u8>,
}

impl Tile {
	/// Wrap a tight BGRA8 buffer. `bgra.len()` must be `width * height * 4`.
	/// The source dimensions default to the tile's own; a producer that scaled
	/// something down reports the original through [`Self::with_source`].
	pub fn new(width: u32, height: u32, bgra: Vec<u8>) -> Self {
		assert_eq!(
			bgra.len(),
			width as usize * height as usize * 4,
			"tile buffer must be tight BGRA8 for its dimensions"
		);
		Self {
			width,
			height,
			source_width: width,
			source_height: height,
			bgra,
		}
	}

	/// Record the dimensions of what this tile was baked from, so a view can
	/// size a cell from the original's aspect rather than the tile's.
	pub fn with_source(mut self, width: u32, height: u32) -> Self {
		self.source_width = width;
		self.source_height = height;
		self
	}

	/// A square tile filled with one BGRA color.
	pub fn solid(size: u32, bgra: [u8; 4]) -> Self {
		let mut buf = vec![0u8; size as usize * size as usize * 4];
		for px in buf.chunks_exact_mut(4) {
			px.copy_from_slice(&bgra);
		}
		Self::new(size, size, buf)
	}

	pub fn width(&self) -> u32 {
		self.width
	}

	pub fn height(&self) -> u32 {
		self.height
	}

	pub fn source_width(&self) -> u32 {
		self.source_width
	}

	pub fn source_height(&self) -> u32 {
		self.source_height
	}

	pub fn bgra(&self) -> &[u8] {
		&self.bgra
	}

	pub fn into_bgra(self) -> Vec<u8> {
		self.bgra
	}
}

impl std::fmt::Debug for Tile {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("Tile")
			.field("width", &self.width)
			.field("height", &self.height)
			.field("source_width", &self.source_width)
			.field("source_height", &self.source_height)
			.field("bytes", &self.bgra.len())
			.finish()
	}
}

/// What a producer is asked to depict: a filesystem entry, plus the optional
/// text an icon tile paints beneath the pictogram.
#[derive(Debug, Clone)]
pub struct WorkItem {
	/// Absolute path of the entry the tile depicts.
	pub path: PathBuf,
	/// Directories take the platform folder icon and are never decoded.
	pub is_dir: bool,
	/// Baked into icon tiles when present; content producers ignore it.
	pub label: Option<Label>,
}

impl WorkItem {
	pub fn file(path: impl Into<PathBuf>) -> Self {
		Self {
			path: path.into(),
			is_dir: false,
			label: None,
		}
	}

	pub fn dir(path: impl Into<PathBuf>) -> Self {
		Self {
			path: path.into(),
			is_dir: true,
			label: None,
		}
	}

	pub fn with_label(mut self, name: impl Into<String>, detail: impl Into<String>) -> Self {
		self.label = Some(Label {
			name: name.into(),
			detail: detail.into(),
		});
		self
	}
}

/// The text an icon tile carries: the entry name, and a secondary detail line
/// (size, date — whatever the view puts there).
#[derive(Debug, Clone)]
pub struct Label {
	pub name: String,
	pub detail: String,
}

/// Why a producer did not yield a tile. A decline is a routing signal, not a
/// chain failure: the caller falls through to the next producer.
#[derive(Debug, thiserror::Error)]
pub enum Decline {
	/// This producer does not handle the item (a directory offered to the
	/// raster decoder, a format outside the stage's coverage).
	#[error("producer does not handle this item")]
	Unsupported,
	/// The producer's backing facility is missing on this platform or build.
	#[error("producer unavailable on this platform")]
	Unavailable,
	/// The producer handles items of this kind but could not turn this one
	/// into pixels.
	#[error("failed to produce tile: {0}")]
	Failed(String),
}

/// One stage of the bake chain.
pub trait Producer: Send + Sync {
	/// Bake `item` into a BGRA8 tile that fits a `tile_size` × `tile_size`
	/// envelope, or decline so the caller can fall through to the next
	/// producer. A producer is free to return less than the full envelope;
	/// what it returns is described by the tile's own dimensions.
	fn produce(&self, item: &WorkItem, tile_size: u32) -> Result<Tile, Decline>;
}

/// Run `item` through `chain` in order, returning the first produced tile.
///
/// If every producer declines (or the chain is empty), the declines come back
/// in chain order so the caller can tell "nothing handles this" from "the
/// source is unreadable".
pub fn bake(
	chain: &[Box<dyn Producer>],
	item: &WorkItem,
	tile_size: u32,
) -> Result<Tile, Vec<Decline>> {
	let mut declines = Vec::with_capacity(chain.len());
	for producer in chain {
		match producer.produce(item, tile_size) {
			Ok(tile) => return Ok(tile),
			Err(decline) => declines.push(decline),
		}
	}
	Err(declines)
}

#[cfg(test)]
mod tests {
	use super::*;

	struct Declines(Decline);

	impl Producer for Declines {
		fn produce(&self, _item: &WorkItem, _tile_size: u32) -> Result<Tile, Decline> {
			Err(match &self.0 {
				Decline::Unsupported => Decline::Unsupported,
				Decline::Unavailable => Decline::Unavailable,
				Decline::Failed(m) => Decline::Failed(m.clone()),
			})
		}
	}

	struct Solid([u8; 4]);

	impl Producer for Solid {
		fn produce(&self, _item: &WorkItem, tile_size: u32) -> Result<Tile, Decline> {
			Ok(Tile::solid(tile_size, self.0))
		}
	}

	#[test]
	fn bake_falls_through_to_the_first_producing_stage() {
		let chain: Vec<Box<dyn Producer>> = vec![
			Box::new(Declines(Decline::Unsupported)),
			Box::new(Solid([1, 2, 3, 255])),
			Box::new(Solid([9, 9, 9, 255])),
		];
		let tile = bake(&chain, &WorkItem::file("/nowhere"), 4).expect("second stage produces");
		assert_eq!(tile.width(), 4);
		assert_eq!(&tile.bgra()[..4], &[1, 2, 3, 255]);
	}

	#[test]
	fn bake_reports_declines_in_chain_order() {
		let chain: Vec<Box<dyn Producer>> = vec![
			Box::new(Declines(Decline::Unsupported)),
			Box::new(Declines(Decline::Failed("boom".into()))),
		];
		let declines = bake(&chain, &WorkItem::file("/nowhere"), 4).unwrap_err();
		assert!(matches!(declines[0], Decline::Unsupported));
		assert!(matches!(&declines[1], Decline::Failed(m) if m == "boom"));
	}

	#[test]
	fn empty_chain_declines_with_nothing() {
		let declines = bake(&[], &WorkItem::file("/nowhere"), 4).unwrap_err();
		assert!(declines.is_empty());
	}

	#[test]
	fn source_dimensions_default_to_the_tile_and_can_be_overridden() {
		let tile = Tile::solid(4, [0, 0, 0, 255]);
		assert_eq!((tile.source_width(), tile.source_height()), (4, 4));
		let scaled = tile.with_source(4032, 3024);
		assert_eq!(
			(scaled.source_width(), scaled.source_height()),
			(4032, 3024)
		);
		assert_eq!((scaled.width(), scaled.height()), (4, 4));
	}

	#[test]
	fn solid_tile_is_tight_bgra() {
		let tile = Tile::solid(8, [10, 20, 30, 255]);
		assert_eq!(tile.bgra().len(), 8 * 8 * 4);
		assert!(tile
			.bgra()
			.chunks_exact(4)
			.all(|px| px == [10, 20, 30, 255]));
	}
}
