// use crate::{
// 	consts::RAW_MAXIMUM_FILE_SIZE,
// 	error::{Error, Result},
// };
// use image::DynamicImage;
// use std::{
// 	fs,
// 	io::{Cursor, Write},
// 	path::Path,
// };

// pub fn raw_to_dynamic_image(path: &Path) -> Result<DynamicImage> {
// 	if fs::metadata(path).map_err(|_| Error::Io)?.len() > RAW_MAXIMUM_FILE_SIZE {
// 		return Err(Error::TooLarge);
// 	}

// 	let image = rawloader::decode_file(path).unwrap();
// 	let mut writer = Cursor::new(vec![]);

// 	if let rawloader::RawImageData::Integer(i) = image.data {
// 		for px in i {
// 			let high = (px >> 8) as u8;
// 			let lo = (px & 0x0ff) as u8;
// 			writer
// 				.write_all(&[high, lo, high, lo, high, lo])
// 				.map_err(|_| Error::Io)?;
// 		}

// 		let image = image::RgbImage::from_raw(
// 			image.width.try_into().map_err(Error::TryFromInt)?,
// 			image.height.try_into().map_err(Error::TryFromInt)?,
// 			writer.into_inner(),
// 		)
// 		.ok_or_else(|| Error::RawConversion)?;

// 		Ok(DynamicImage::ImageRgb8(image))
// 	} else {
// 		Err(Error::RawConversion)
// 	}
// }
