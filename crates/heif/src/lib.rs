use std::{io::SeekFrom, path::Path};

use image::DynamicImage;
use libheif_rs::{ColorSpace, HeifContext, LibHeif, RgbChroma};
use std::io::Cursor;
use thiserror::Error;
use tokio::{
	fs,
	io::{AsyncReadExt, AsyncSeekExt, BufReader},
	task::{spawn_blocking, JoinError},
};

type HeifResult<T> = Result<T, HeifError>;

/// The maximum file size that an image can be in order to have a thumbnail generated.
///
/// This value is in MiB.
const HEIF_MAXIMUM_FILE_SIZE: u64 = 1048576 * 20;

#[derive(Error, Debug)]
pub enum HeifError {
	#[error("error with libheif: {0}")]
	LibHeif(#[from] libheif_rs::HeifError),
	#[error("error while loading the image (via the `image` crate): {0}")]
	Image(#[from] image::ImageError),
	#[error("io error: {0} at {}", .1.display())]
	Io(std::io::Error, Box<Path>),
	#[error("Blocking task failed to execute to completion.")]
	Join(#[from] JoinError),
	#[error("there was an error while converting the image to an `RgbImage`")]
	RgbImageConversion,
	#[error("the image provided is unsupported")]
	Unsupported,
	#[error("the image provided is too large (over 20MiB)")]
	TooLarge,
	#[error("the provided bit depth is invalid")]
	InvalidBitDepth,
	#[error("invalid path provided (non UTF-8)")]
	InvalidPath,
}

thread_local! {
	static HEIF: LibHeif = LibHeif::new();
}

pub async fn heif_to_dynamic_image(path: impl AsRef<Path>) -> HeifResult<DynamicImage> {
	let path = path.as_ref();

	if fs::metadata(path)
		.await
		.map_err(|e| HeifError::Io(e, path.to_path_buf().into_boxed_path()))?
		.len() > HEIF_MAXIMUM_FILE_SIZE
	{
		return Err(HeifError::TooLarge);
	}

	let data = fs::read(path)
		.await
		.map_err(|e| HeifError::Io(e, path.to_path_buf().into_boxed_path()))?;

	let (img_data, stride, height, width) = spawn_blocking(move || -> Result<_, HeifError> {
		// do this in a separate block so we drop the raw (potentially huge) image handle
		let ctx = HeifContext::read_from_bytes(&data)?;
		let handle = ctx.primary_image_handle()?;

		let img = HEIF.with(|heif| heif.decode(&handle, ColorSpace::Rgb(RgbChroma::Rgb), None))?;

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
			.map_err(|e| HeifError::Io(e, path.to_path_buf().into_boxed_path()))?;

		for _ in 0..width {
			reader
				.read_exact(&mut buffer)
				.await
				.map_err(|e| HeifError::Io(e, path.to_path_buf().into_boxed_path()))?;
			sequence.extend_from_slice(&buffer);
		}
	}

	let rgb_img =
		image::RgbImage::from_raw(width, height, sequence).ok_or(HeifError::RgbImageConversion)?;

	Ok(DynamicImage::ImageRgb8(rgb_img))
}
