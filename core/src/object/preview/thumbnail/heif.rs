use std::{
	fs,
	io::{Cursor, Read, Seek, SeekFrom},
	path::Path,
};

use image::DynamicImage;
use libheif_rs::{ColorSpace, HeifContext, LibHeif, RgbChroma};
use png::{BitDepth, ColorType};
use thiserror::Error;

type HeifResult<T> = Result<T, HeifError>;

/// The maximum file size that an image can be in order to have a thumbnail generated.
///
/// This value is in MiB.
const HEIF_MAXIMUM_FILE_SIZE: u64 = 1048576 * 20;

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
	#[error("the image provided is too large (over 20MiB)")]
	TooLarge,
	#[error("the provided bit depth is invalid")]
	InvalidBitDepth,
	#[error("invalid path provided (non UTF-8)")]
	InvalidPath,
}

pub fn heif_to_dynamic_image(path: &Path) -> HeifResult<DynamicImage> {
	if fs::metadata(path)?.len() > HEIF_MAXIMUM_FILE_SIZE {
		return Err(HeifError::TooLarge);
	}

	let img = {
		// do this in a separate block so we drop the raw (potentially huge) image
		let ctx = HeifContext::read_from_file(path.to_str().ok_or(HeifError::InvalidPath)?)?;
		let heif = LibHeif::new();
		let handle = ctx.primary_image_handle()?;

		heif.decode(&handle, ColorSpace::Rgb(RgbChroma::Rgb), None)?
	};

	// TODO(brxken128): add support for images with individual r/g/b channels
	// i'm unable to find a sample to test with, but it should follow the same principles as this one
	if let Some(i) = img.planes().interleaved {
		let data = i.data.to_vec();
		let mut reader = Cursor::new(data);

		let mut sequence = vec![];
		let mut buffer = [0u8; 3]; // [r, g, b]

		// this is the interpolation stuff, it essentially just makes the image correct
		// in regards to stretching/resolution, etc
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
		png_encoder
			.set_depth(BitDepth::from_u8(i.bits_per_pixel).ok_or(HeifError::InvalidBitDepth)?);

		let mut png_writer = png_encoder.write_header()?;
		png_writer.write_image_data(&sequence)?;
		png_writer.finish()?;

		image::load_from_memory_with_format(&writer.into_inner(), image::ImageFormat::Png)
			.map_err(HeifError::Image)
	} else {
		Err(HeifError::Unsupported)
	}
}
