//! Decoding and tensor preparation shared by the image models.

use std::io::Cursor;

use image::{imageops::FilterType, DynamicImage, ImageDecoder, ImageReader, RgbImage};

use super::AiError;

/// Decodes a file's bytes upright, at most `max_side` on its long edge.
///
/// EXIF orientation is applied so boxes match what the user sees. A RAW
/// file the image crate cannot read falls back to its embedded preview.
pub fn decode(bytes: &[u8], max_side: u32) -> Result<RgbImage, AiError> {
	let image = decode_oriented(bytes)
		.or_else(|e| sd_images::embedded_preview(bytes).ok_or(e))
		.map_err(|e| AiError::Input(format!("unreadable image: {e}")))?;
	let image = if image.width().max(image.height()) > max_side {
		image.resize(max_side, max_side, FilterType::Triangle)
	} else {
		image
	};
	Ok(image.into_rgb8())
}

fn decode_oriented(bytes: &[u8]) -> Result<DynamicImage, image::ImageError> {
	let mut decoder = ImageReader::new(Cursor::new(bytes))
		.with_guessed_format()?
		.into_decoder()?;
	let orientation = decoder.orientation()?;
	let mut image = DynamicImage::from_decoder(decoder)?;
	image.apply_orientation(orientation);
	Ok(image)
}

/// Where a letterboxed image sits inside its square model input, so model
/// coordinates map back onto the image.
#[derive(Debug, Clone, Copy)]
pub struct Letterbox {
	pub scale: f32,
	pub pad_x: f32,
	pub pad_y: f32,
	pub side: f32,
}

impl Letterbox {
	/// A point in model-input fractions, as fractions of the source image.
	pub fn unmap(&self, x: f32, y: f32, width: u32, height: u32) -> (f32, f32) {
		let px = (x * self.side - self.pad_x) / self.scale;
		let py = (y * self.side - self.pad_y) / self.scale;
		(px / width as f32, py / height as f32)
	}
}

/// Scales `image` to fit a `side` square, padding with black, and writes it
/// as interleaved RGB (NHWC) scaled to `[lo, hi]`.
pub fn letterbox_nhwc(image: &RgbImage, side: u32, lo: f32, hi: f32) -> (Vec<f32>, Letterbox) {
	let scale = side as f32 / image.width().max(image.height()) as f32;
	let w = ((image.width() as f32 * scale).round() as u32).clamp(1, side);
	let h = ((image.height() as f32 * scale).round() as u32).clamp(1, side);
	let resized = image::imageops::resize(image, w, h, FilterType::Triangle);
	let pad_x = (side - w) / 2;
	let pad_y = (side - h) / 2;
	let mut out = vec![lo; (side * side * 3) as usize];
	for (x, y, p) in resized.enumerate_pixels() {
		let i = (((y + pad_y) * side + x + pad_x) * 3) as usize;
		for c in 0..3 {
			out[i + c] = lo + (hi - lo) * p[c] as f32 / 255.0;
		}
	}
	(
		out,
		Letterbox {
			scale,
			pad_x: pad_x as f32,
			pad_y: pad_y as f32,
			side: side as f32,
		},
	)
}

/// Stretches `image` to `side` x `side` and appends it as planar RGB (NCHW)
/// normalized per channel, the layout CLIP-style vision towers expect.
pub fn push_nchw(out: &mut Vec<f32>, image: &RgbImage, side: u32, mean: [f32; 3], std: [f32; 3]) {
	let resized = image::imageops::resize(image, side, side, FilterType::Triangle);
	let plane = (side * side) as usize;
	let start = out.len();
	out.resize(start + plane * 3, 0.0);
	for (i, p) in resized.pixels().enumerate() {
		for c in 0..3 {
			out[start + c * plane + i] = (p[c] as f32 / 255.0 - mean[c]) / std[c];
		}
	}
}
