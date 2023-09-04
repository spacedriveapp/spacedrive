use crate::consts::RAW_MAXIMUM_FILE_SIZE;
pub use crate::error::{Error, Result};
use crate::ToImage;
use image::DynamicImage;
use std::{
	io::{Cursor, Write},
	path::Path,
};

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
		let mut writer = Cursor::new(vec![]);

		#[allow(clippy::as_conversions)]
		if let rawloader::RawImageData::Integer(i) = image.data {
			for px in i {
				let high = (px >> 8) as u8;
				let lo = (px & 0x0ff) as u8;
				writer.write_all(&[high, lo, high, lo, high, lo])?;
			}

			let image = image::RgbImage::from_raw(
				image.width.try_into().map_err(Error::TryFromInt)?,
				image.height.try_into().map_err(Error::TryFromInt)?,
				writer.into_inner(),
			)
			.ok_or_else(|| Error::RawConversion)?;

			Ok(DynamicImage::ImageRgb8(image))
		} else {
			Err(Error::RawConversion)
		}
	}
}
