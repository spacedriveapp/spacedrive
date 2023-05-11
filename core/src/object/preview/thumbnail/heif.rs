use std::{
	fs,
	io::{Cursor, Read, Seek, SeekFrom},
	path::Path,
};

use image::DynamicImage;
use libheif_rs::{Channel, ColorSpace, HeifContext, LibHeif, RgbChroma};
use png::{BitDepth, ColorType};
use thiserror::Error;

type HeifResult<T> = Result<T, HeifError>;

#[derive(Error, Debug)]
pub enum HeifError {
	#[error("error with libheif: {0}")]
	LibHeif(#[from] libheif_rs::HeifError),
	#[error("error while encoding to png: {0}")]
	PngEncode(#[from] png::EncodingError),
	#[error("error while loading the image (via the `image` crate): {0}")]
	Image(#[from] image::ImageError),
	#[error("io error: {0}")]
	Io(#[from] std::io::Error),
	#[error("the image provided is unsupported")]
	Unsupported,
	#[error("the image provided is too large (over 30mb)")]
	TooLarge,
}

pub fn heif_to_dynamic_image(path: &Path) -> HeifResult<DynamicImage> {
	if fs::metadata(path)?.len() > 1048576 * 30 {
		return Err(HeifError::TooLarge);
	}

	let img = {
		// do this in a separate block so we drop the raw (potentially huge) image
		let ctx = HeifContext::read_from_file(path.to_str().unwrap())?;
		let heif = LibHeif::new();
		let handle = ctx.primary_image_handle()?;

		let img_raw = heif.decode(&handle, ColorSpace::Rgb(RgbChroma::Rgb), None)?;

		// TODO(brxken128): handle the scaling better here, and limit it to x bytes
		img_raw.scale(img_raw.width() / 3, img_raw.height() / 3, None)?
	};

	// TODO(brxken128): add support for images with individual r/g/b channels
	if img.has_channel(Channel::Interleaved) {
		let i = img.planes().interleaved.unwrap();
		let data = i.data.to_vec();
		let mut reader = Cursor::new(data);

		let mut sequence = vec![];
		let mut buffer = [0u8; 3]; // [r, g, b]

		for y in 0..img.height() {
			reader.seek(SeekFrom::Start((i.stride * y as usize) as u64))?;

			for _ in 0..img.width() {
				reader.read_exact(&mut buffer)?;
				sequence.extend_from_slice(&buffer);
			}
		}

		let mut writer = Cursor::new(vec![]);

		let mut png_encoder = png::Encoder::new(&mut writer, i.width, i.height);
		png_encoder.set_color(ColorType::Rgb);
		png_encoder.set_depth(BitDepth::from_u8(i.bits_per_pixel).unwrap());

		let mut png_writer = png_encoder.write_header()?;
		png_writer.write_image_data(&sequence)?;
		png_writer.finish()?;

		image::load_from_memory_with_format(&writer.into_inner(), image::ImageFormat::Png)
			.map_err(HeifError::Image)
	} else {
		Err(HeifError::Unsupported)
	}
}
