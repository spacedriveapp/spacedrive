//! The raster decode stage: read an image file, scale it to the cell, hand
//! back BGRA8. Decoding goes through `sd-images` first (which routes SVG, PDF,
//! and — when enabled — HEIF to their handlers), then falls back to a
//! content-sniffing decode so misnamed rasters (a JPEG behind a `.thm`
//! extension) still bake. EXIF orientation is resolved here, so a tile's
//! dimensions are the ones the image is displayed at.

use std::path::Path;

use image::{imageops, DynamicImage, RgbaImage};
use sd_media_metadata::exif::Orientation;

use crate::{Decline, Producer, Tile, WorkItem};

/// How a source maps onto the cell.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScaleMode {
	/// Aspect-fill: center-crop the source to a square, then resize. The cell
	/// is all image; edges of the long axis are lost.
	Cover,
	/// Aspect-fit: scale the whole source so its long edge meets the cell and
	/// return it at its own proportions. Nothing is cropped and no padding is
	/// stored, so the tile is the image and not the cell it fits inside.
	Fit,
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
		let (source_width, source_height) = rgba.dimensions();
		let tile = match self.mode {
			ScaleMode::Cover => Tile::new(tile_size, tile_size, cover(&rgba, tile_size)),
			ScaleMode::Fit => {
				let (width, height, bgra) = fit(&rgba, tile_size);
				Tile::new(width, height, bgra)
			}
		};
		Ok(tile.with_source(source_width, source_height))
	}
}

fn decode(path: &Path) -> Result<DynamicImage, Decline> {
	let decoded = match sd_images::format_image(path) {
		Ok(image) => image,
		Err(_) => image::ImageReader::open(path)
			.and_then(|reader| reader.with_guessed_format())
			.map_err(|e| Decline::Failed(format!("read {}: {e}", path.display())))?
			.decode()
			.map_err(|e| Decline::Failed(format!("decode {}: {e}", path.display())))?,
	};
	// A rotated camera frame decodes to its stored orientation, so without this
	// a portrait photo bakes landscape and reports the wrong aspect.
	Ok(match Orientation::from_path(path) {
		Some(orientation) => orientation.correct_thumbnail(decoded),
		None => decoded,
	})
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

/// Aspect-fit resize to the cell's long edge, RGBA→BGRA, returning the fitted
/// extent along with the pixels. A source already smaller than the cell keeps
/// its own size rather than being scaled up, since upscaling here only spends
/// bytes on resolution the file never had.
fn fit(img: &RgbaImage, cell: u32) -> (u32, u32, Vec<u8>) {
	let (w, h) = img.dimensions();
	let scale = (f64::from(cell) / f64::from(w.max(h))).min(1.0);
	let dw = ((f64::from(w) * scale).round() as u32).clamp(1, cell);
	let dh = ((f64::from(h) * scale).round() as u32).clamp(1, cell);
	if (dw, dh) == (w, h) {
		return (dw, dh, rgba_into_bgra(img.clone().into_raw()));
	}
	let resized = imageops::resize(img, dw, dh, imageops::FilterType::Triangle);
	(dw, dh, rgba_into_bgra(resized.into_raw()))
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

	/// Splice a minimal APP1/EXIF segment carrying just an orientation tag in
	/// after a baseline JPEG's SOI, which is all `Orientation::from_path` reads.
	fn write_exif_orientation(path: &Path, orientation: u16) {
		let jpeg = std::fs::read(path).expect("read jpeg");
		assert_eq!(&jpeg[..2], &[0xFF, 0xD8], "expected a baseline JPEG");

		let mut tiff = Vec::new();
		tiff.extend_from_slice(b"MM\x00\x2a"); // big-endian TIFF header
		tiff.extend_from_slice(&8u32.to_be_bytes()); // offset of IFD0
		tiff.extend_from_slice(&1u16.to_be_bytes()); // one entry
		tiff.extend_from_slice(&0x0112u16.to_be_bytes()); // Orientation
		tiff.extend_from_slice(&3u16.to_be_bytes()); // SHORT
		tiff.extend_from_slice(&1u32.to_be_bytes()); // count
		tiff.extend_from_slice(&orientation.to_be_bytes());
		tiff.extend_from_slice(&[0, 0]); // value padded to four bytes
		tiff.extend_from_slice(&0u32.to_be_bytes()); // no next IFD

		let mut payload = b"Exif\x00\x00".to_vec();
		payload.extend_from_slice(&tiff);

		let mut out = vec![0xFF, 0xD8, 0xFF, 0xE1];
		out.extend_from_slice(&((payload.len() + 2) as u16).to_be_bytes());
		out.extend_from_slice(&payload);
		out.extend_from_slice(&jpeg[2..]);
		std::fs::write(path, out).expect("write jpeg with exif");
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
	fn fit_keeps_the_source_aspect_and_stores_no_padding() {
		let dir = tempfile::tempdir().expect("tempdir");
		let path = save_png(&banded_strip(), dir.path(), "strip.png");

		let tile = ImageProducer::new(ScaleMode::Fit)
			.produce(&WorkItem::file(path), TILE)
			.expect("png decodes");

		// A 200×100 source fits as 32×16: the long edge meets the cell, the
		// short edge follows the ratio, and the buffer holds nothing else.
		assert_eq!((tile.width(), tile.height()), (TILE, TILE / 2));
		assert_eq!(tile.bgra().len(), (TILE * (TILE / 2) * 4) as usize);
		assert_eq!((tile.source_width(), tile.source_height()), (200, 100));

		// All three bands survive, which a center crop would not have kept.
		assert_eq!(pixel(tile.bgra(), TILE, 2, 8), [0, 0, 255, 255]); // red band → B,G,R
		assert_eq!(pixel(tile.bgra(), TILE, 16, 8), [0, 255, 0, 255]);
		assert_eq!(pixel(tile.bgra(), TILE, 30, 8), [255, 0, 0, 255]); // blue band
	}

	#[test]
	fn fit_never_scales_a_small_source_up() {
		let dir = tempfile::tempdir().expect("tempdir");
		let path = save_png(
			&RgbaImage::from_pixel(12, 9, Rgba([0, 255, 0, 255])),
			dir.path(),
			"small.png",
		);

		let tile = ImageProducer::new(ScaleMode::Fit)
			.produce(&WorkItem::file(path), TILE)
			.expect("png decodes");

		assert_eq!((tile.width(), tile.height()), (12, 9));
	}

	#[test]
	fn a_portrait_frame_tagged_rotated_bakes_upright() {
		// EXIF orientation 6 means the stored pixels are rotated 90° CW from
		// how the image is displayed, so a tile that ignores it reports the
		// wrong aspect for the file.
		let dir = tempfile::tempdir().expect("tempdir");
		let path = dir.path().join("rotated.jpg");
		image::RgbImage::from_pixel(200, 100, image::Rgb([0, 255, 0]))
			.save(&path)
			.expect("write jpeg");
		write_exif_orientation(&path, 6);

		let tile = ImageProducer::new(ScaleMode::Fit)
			.produce(&WorkItem::file(&path), TILE)
			.expect("jpeg decodes");

		assert_eq!(
			(tile.width(), tile.height()),
			(TILE / 2, TILE),
			"a frame tagged 90° CW is stored upright, so it bakes portrait"
		);
		assert_eq!((tile.source_width(), tile.source_height()), (100, 200));
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
