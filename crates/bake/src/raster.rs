//! The raster decode stage: read an image file, scale it to the cell, hand
//! back BGRA8. Decoding goes through `sd-images` first (which routes SVG, PDF,
//! and — when enabled — HEIF to their handlers), then falls back to a
//! content-sniffing decode so misnamed rasters (a JPEG behind a `.thm`
//! extension) still bake.

use std::path::Path;

use image::{imageops, DynamicImage, RgbaImage};

use crate::{Decline, Producer, Tile, WorkItem};

/// How a non-square source maps onto the square cell.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScaleMode {
	/// Aspect-fill: center-crop the source to a square, then resize. The cell
	/// is all image; edges of the long axis are lost.
	Cover,
	/// Aspect-fit: resize the whole source to fit, letterboxed over
	/// `background` (BGRA). Nothing is cropped.
	Contain { background: [u8; 4] },
}

/// Decodes image files into cells. Declines directories and anything no
/// decoder recognizes.
pub struct ImageProducer {
	mode: ScaleMode,
}

impl ImageProducer {
	pub fn new(mode: ScaleMode) -> Self {
		Self { mode }
	}
}

impl Producer for ImageProducer {
	fn produce(&self, item: &WorkItem, tile_size: u32) -> Result<Tile, Decline> {
		if item.is_dir {
			return Err(Decline::Unsupported);
		}
		let rgba = decode(&item.path)?.to_rgba8();
		let bgra = match self.mode {
			ScaleMode::Cover => cover(&rgba, tile_size),
			ScaleMode::Contain { background } => contain(&rgba, tile_size, background),
		};
		Ok(Tile::new(tile_size, tile_size, bgra))
	}
}

fn decode(path: &Path) -> Result<DynamicImage, Decline> {
	if let Ok(img) = sd_images::format_image(path) {
		return Ok(img);
	}
	image::ImageReader::open(path)
		.and_then(|reader| reader.with_guessed_format())
		.map_err(|e| Decline::Failed(format!("read {}: {e}", path.display())))?
		.decode()
		.map_err(|e| Decline::Failed(format!("decode {}: {e}", path.display())))
}

/// Aspect-fill center-crop to square, resize to `tile`, RGBA→BGRA.
fn cover(img: &RgbaImage, tile: u32) -> Vec<u8> {
	let (w, h) = img.dimensions();
	let side = w.min(h);
	let x = (w - side) / 2;
	let y = (h - side) / 2;
	let cropped = imageops::crop_imm(img, x, y, side, side).to_image();
	let square = imageops::resize(&cropped, tile, tile, imageops::FilterType::Triangle);
	rgba_into_bgra(square.into_raw())
}

/// Aspect-fit resize onto a `background`-filled square, RGBA→BGRA. The source
/// is alpha-composited over the background, centered, so the letterbox bars
/// and any transparency read as the cell background.
fn contain(img: &RgbaImage, tile: u32, background: [u8; 4]) -> Vec<u8> {
	let (w, h) = img.dimensions();
	let scale = f64::from(tile) / f64::from(w.max(h));
	let dw = ((f64::from(w) * scale).round() as u32).clamp(1, tile);
	let dh = ((f64::from(h) * scale).round() as u32).clamp(1, tile);
	let resized = imageops::resize(img, dw, dh, imageops::FilterType::Triangle);

	let bg_rgba = image::Rgba([background[2], background[1], background[0], background[3]]);
	let mut canvas = RgbaImage::from_pixel(tile, tile, bg_rgba);
	imageops::overlay(
		&mut canvas,
		&resized,
		i64::from((tile - dw) / 2),
		i64::from((tile - dh) / 2),
	);
	rgba_into_bgra(canvas.into_raw())
}

fn rgba_into_bgra(mut buf: Vec<u8>) -> Vec<u8> {
	for px in buf.chunks_exact_mut(4) {
		px.swap(0, 2);
	}
	buf
}

#[cfg(test)]
mod tests {
	use super::*;
	use image::Rgba;
	use std::path::PathBuf;

	const TILE: u32 = 32;

	/// A 200×100 strip: red left band, green center square, blue right band.
	/// A center crop keeps only the green; a fit keeps all three.
	fn banded_strip() -> RgbaImage {
		RgbaImage::from_fn(200, 100, |x, _| {
			if x < 50 {
				Rgba([255, 0, 0, 255])
			} else if x < 150 {
				Rgba([0, 255, 0, 255])
			} else {
				Rgba([0, 0, 255, 255])
			}
		})
	}

	fn save_png(img: &RgbaImage, dir: &Path, name: &str) -> PathBuf {
		let path = dir.join(name);
		img.save(&path).expect("write test image");
		path
	}

	fn pixel(bgra: &[u8], tile: u32, x: u32, y: u32) -> [u8; 4] {
		let i = ((y * tile + x) * 4) as usize;
		[bgra[i], bgra[i + 1], bgra[i + 2], bgra[i + 3]]
	}

	#[test]
	fn cover_center_crops_to_a_square_bgra_tile() {
		let dir = tempfile::tempdir().expect("tempdir");
		let path = save_png(&banded_strip(), dir.path(), "strip.png");

		let tile = ImageProducer::new(ScaleMode::Cover)
			.produce(&WorkItem::file(path), TILE)
			.expect("png decodes");

		assert_eq!((tile.width(), tile.height()), (TILE, TILE));
		assert_eq!(tile.bgra().len(), (TILE * TILE * 4) as usize);
		// The 100×100 center of the strip is solid green, so every pixel of
		// the covered tile is green — stored BGRA.
		assert!(tile.bgra().chunks_exact(4).all(|px| px == [0, 255, 0, 255]));
	}

	#[test]
	fn contain_letterboxes_over_the_background() {
		let dir = tempfile::tempdir().expect("tempdir");
		let path = save_png(&banded_strip(), dir.path(), "strip.png");
		let background = [10, 20, 30, 255];

		let tile = ImageProducer::new(ScaleMode::Contain { background })
			.produce(&WorkItem::file(path), TILE)
			.expect("png decodes");

		// A 200×100 source fits as 32×16, vertically centered: rows 0..8 and
		// 24..32 are letterbox bars, the middle rows are image.
		assert_eq!(pixel(tile.bgra(), TILE, 16, 2), background);
		assert_eq!(pixel(tile.bgra(), TILE, 16, 30), background);
		assert_eq!(pixel(tile.bgra(), TILE, 16, 16), [0, 255, 0, 255]);
		assert_eq!(pixel(tile.bgra(), TILE, 2, 16), [0, 0, 255, 255]); // red band → B,G,R
		assert_eq!(pixel(tile.bgra(), TILE, 30, 16), [255, 0, 0, 255]); // blue band
	}

	#[test]
	fn jpeg_roundtrips_within_lossy_tolerance() {
		let dir = tempfile::tempdir().expect("tempdir");
		let path = dir.path().join("flat.jpg");
		image::RgbImage::from_pixel(64, 64, image::Rgb([0, 255, 0]))
			.save(&path)
			.expect("write jpeg");

		let tile = ImageProducer::new(ScaleMode::Cover)
			.produce(&WorkItem::file(path), TILE)
			.expect("jpeg decodes");

		for px in tile.bgra().chunks_exact(4) {
			assert!(
				px[0] < 16 && px[1] > 239 && px[2] < 16,
				"expected ~green, got {px:?}"
			);
		}
	}

	#[test]
	fn misnamed_extension_decodes_by_content() {
		let dir = tempfile::tempdir().expect("tempdir");
		let png = save_png(
			&RgbaImage::from_pixel(16, 16, Rgba([255, 0, 0, 255])),
			dir.path(),
			"actually-a-png.png",
		);
		let thm = dir.path().join("shot.thm");
		std::fs::copy(&png, &thm).expect("copy");

		let tile = ImageProducer::new(ScaleMode::Cover)
			.produce(&WorkItem::file(thm), TILE)
			.expect("content sniffing finds the PNG");
		assert_eq!(&tile.bgra()[..4], &[0, 0, 255, 255]);
	}

	#[test]
	fn declines_directories_and_unreadable_sources() {
		let producer = ImageProducer::new(ScaleMode::Cover);
		assert!(matches!(
			producer.produce(&WorkItem::dir("/"), TILE),
			Err(Decline::Unsupported)
		));

		let dir = tempfile::tempdir().expect("tempdir");
		let garbage = dir.path().join("noise.png");
		std::fs::write(&garbage, b"not an image at all").expect("write");
		assert!(matches!(
			producer.produce(&WorkItem::file(garbage), TILE),
			Err(Decline::Failed(_))
		));
		assert!(matches!(
			producer.produce(&WorkItem::file(dir.path().join("missing.png")), TILE),
			Err(Decline::Failed(_))
		));
	}
}
