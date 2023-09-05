use crate::consts::RAW_MAXIMUM_FILE_SIZE;
pub use crate::error::{Error, Result};
use crate::ToImage;
use image::{DynamicImage, Rgb};
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
		let mut writer = image::ImageBuffer::new(image.width.try_into()?, image.height.try_into()?);

		#[allow(clippy::as_conversions)]
		match image.cpp {
			3 => {
				for px in writer.pixels_mut() {
					if let rawloader::RawImageData::Integer(ref i) = image.data {
						let mut first = FirstPx::Hi;
						for raw_px in i {
							if first == FirstPx::Hi {
								let hi = (raw_px >> 8) as u8;
								let lo = (raw_px & 0x0ff) as u8;

								*px = Rgb([hi, lo, hi]);
								first = FirstPx::Lo;
							} else {
								let lo = (raw_px & 0x0ff) as u8;
								let hi = (raw_px >> 8) as u8;
								*px = Rgb([lo, hi, lo]);
								first = FirstPx::Hi;
							}
						}
					}
				}

				Ok(DynamicImage::ImageRgb8(writer))
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
