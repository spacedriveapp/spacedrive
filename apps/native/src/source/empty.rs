//! The source a window shows before it has a folder: no cells, nothing
//! pending, no work. Photos starts here and stays here until the window it
//! follows names a directory.

use super::{Bitmap, TileSource};

pub struct EmptySource;

impl TileSource for EmptySource {
	fn poll(&mut self) {}

	fn len(&self) -> u32 {
		0
	}

	fn tile(&self) -> u32 {
		super::TILE
	}

	fn request(&mut self, _idx: u32) {}

	fn has_pending(&self) -> bool {
		false
	}

	fn drain(&mut self, _max: usize) -> Vec<(u32, Bitmap)> {
		Vec::new()
	}

	fn set_visible_range(&mut self, _first: u32, _last: u32) {}
}
