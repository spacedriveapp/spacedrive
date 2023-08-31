use std::{
	io::{Cursor, SeekFrom},
	sync::Arc,
};

use image::DynamicImage;
use libheif_rs::{ColorSpace, HeifContext, LibHeif, RgbChroma};
use once_cell::sync::Lazy;
use thiserror::Error;
use tokio::{
	io::{AsyncReadExt, AsyncSeekExt, BufReader},
	task::{spawn_blocking, JoinError},
};

type HeifResult<T> = Result<T, HeifError>;

// The maximum file size that an image can be in order to have a thumbnail generated.
pub const MAXIMUM_FILE_SIZE: u64 = 20 * 1024 * 1024; // 20MB

#[derive(Error, Debug)]
pub enum HeifError {
	#[error("error with libheif: {0}")]
	LibHeif(#[from] libheif_rs::HeifError),
	#[error("error while loading the image (via the `image` crate): {0}")]
	Image(#[from] image::ImageError),
	#[error("Blocking task failed to execute to completion.")]
	Join(#[from] JoinError),
	#[error("there was an error while converting the image to an `RgbImage`")]
	RgbImageConversion,
	#[error("the image provided is unsupported")]
	Unsupported,
	#[error("the provided bit depth is invalid")]
	InvalidBitDepth,
	#[error("invalid path provided (non UTF-8)")]
	InvalidPath,
}

static HEIF: Lazy<LibHeif> = Lazy::new(LibHeif::new);

pub async fn heif_to_dynamic_image(data: Arc<Vec<u8>>) -> HeifResult<DynamicImage> {
	let (img_data, stride, height, width) = spawn_blocking(move || -> Result<_, HeifError> {
		let ctx = HeifContext::read_from_bytes(&data)?;
		let handle = ctx.primary_image_handle()?;
		let img = HEIF.decode(&handle, ColorSpace::Rgb(RgbChroma::Rgb), None)?;

		// TODO(brxken128): add support for images with individual r/g/b channels
		// i'm unable to find a sample to test with, but it should follow the same principles as this one
		let Some(planes) = img.planes().interleaved else {
			return Err(HeifError::Unsupported);
		};

		if planes.bits_per_pixel != 8 {
			return Err(HeifError::InvalidBitDepth);
		}

		Ok((
			planes.data.to_vec(),
			planes.stride,
			img.height(),
			img.width(),
		))
	})
	.await??;

	let mut buffer = [0u8; 3]; // [r, g, b]
	let mut reader = BufReader::new(Cursor::new(img_data));
	let mut sequence = vec![];

	// this is the interpolation stuff, it essentially just makes the image correct
	// in regards to stretching/resolution, etc
	for y in 0..height {
		reader
			.seek(SeekFrom::Start((stride * y as usize) as u64))
			.await
			.map_err(|_| HeifError::RgbImageConversion)?;

		for _ in 0..width {
			reader
				.read_exact(&mut buffer)
				.await
				.map_err(|_| HeifError::RgbImageConversion)?;
			sequence.extend_from_slice(&buffer);
		}
	}

	let rgb_img =
		image::RgbImage::from_raw(width, height, sequence).ok_or(HeifError::RgbImageConversion)?;

	Ok(DynamicImage::ImageRgb8(rgb_img))
}
