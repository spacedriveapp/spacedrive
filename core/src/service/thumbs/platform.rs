#![cfg(target_os = "macos")]

use sd_bake::{Decline, Producer, Tile, WorkItem};

/// Content previews from ImageIO and QuickLook, available in a default macOS
/// build without any bundled codec libraries.
pub(super) struct PlatformProducer;

impl Producer for PlatformProducer {
	fn produce(&self, item: &WorkItem, tile_size: u32) -> Result<Tile, Decline> {
		if item.is_dir {
			return Err(Decline::Unsupported);
		}
		let jpeg = sd_imageio::file_thumbnail_jpeg(&item.path, i64::from(tile_size), 0.82)
			.ok_or(Decline::Unsupported)?;
		let rgba = image::load_from_memory(&jpeg)
			.map_err(|error| Decline::Failed(format!("decode platform preview: {error}")))?
			.to_rgba8();
		let (width, height) = rgba.dimensions();
		let mut bgra = rgba.into_raw();
		for pixel in bgra.chunks_exact_mut(4) {
			pixel.swap(0, 2);
		}
		Ok(Tile::new(width, height, bgra).with_source(width, height))
	}
}
