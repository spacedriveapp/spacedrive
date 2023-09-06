use crate::consts::RAW_MAXIMUM_FILE_SIZE;
pub use crate::error::{Error, Result};
use crate::ToImage;
use image::{DynamicImage, ImageBuffer, Rgb};
use std::{io::Cursor, path::Path};

#[derive(PartialEq, Eq)]
enum FirstPx {
	Hi,
	Lo,
}

pub struct RawHandler {}

impl ToImage for RawHandler {
	fn maximum_size(&self) -> u64 {
		RAW_MAXIMUM_FILE_SIZE
	}

	fn validate_image(&self, _bits_per_pixel: u8, _length: usize) -> Result<()> {
		Ok(())
	}

	fn handle_image(&self, path: &Path) -> Result<DynamicImage> {
		let mut data = Cursor::new(self.get_data(path)?); // this also makes sure the file isn't above the maximum size

		let image = rawloader::decode(&mut data)?;
		let width = image.width;
		let height = image.height;

		let mut buffer = image::ImageBuffer::new(width.try_into()?, height.try_into()?);

		#[allow(clippy::as_conversions)]
		match image.cpp {
			3 => {
				if let rawloader::RawImageData::Integer(data) = image.data {
					for (x, y, px) in buffer.enumerate_pixels_mut() {
						let i = (y as usize * width + x as usize) * 3; // get the current pixel lcoation, assuming rgb

						*px = Rgb([data[i] as u8, data[i + 1] as u8, data[i + 2] as u8]);
					}
				}

				Ok(DynamicImage::ImageRgb8(buffer))

				// todo!()
			}
			1 => todo!(),
			_ => unreachable!(),
		}

		// #[allow(clippy::as_conversions)]
		// if let rawloader::RawImageData::Integer(i) = image.data {
		// 	for px in i {
		// 		let high = (px >> 8) as u8;
		// 		let lo = (px & 0x0ff) as u8;
		// 		writer.write_all(&[high, lo, high, lo, high, lo])?;
		// 	}

		// 	writer.flush()?;

		// 	let image = image::RgbImage::from_raw(
		// 		image.width.try_into().map_err(Error::TryFromInt)?,
		// 		image.height.try_into().map_err(Error::TryFromInt)?,
		// 		writer.into_inner(),
		// 	)
		// 	.ok_or_else(|| Error::RawConversion)?;

		// 	Ok(DynamicImage::ImageRgb8(image))
		// } else {
		// 	Err(Error::RawConversion)
		// }
	}
}
