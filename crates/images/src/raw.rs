use std::{
	fs::File,
	io::{Cursor, Write},
	path::Path,
};

use image::{DynamicImage, ImageDecoder};

use crate::error::{Error, Result};

pub fn raw_to_dynamic_image(path: impl AsRef<Path>) -> Result<DynamicImage> {
	let image = rawloader::decode_file(path).unwrap();

	// let mut writer = Cursor::new(vec![]);

	// let x = image::ImageBuffer::from_raw(
	// 	image.width.try_into().expect("unable to convert usize"),
	// 	image.height.try_into().expect("unable to convert usize"),
	// 	&image.xyz_to_cam[..],
	// );

	// if let rawloader::RawImageData::Integer(i) = image.data {
	// 	for px in i {
	// 		let pixhigh = (px >> 8) as u8;
	// 		let pixlow = (px & 0x0ff) as u8;
	// 		writer
	// 			.write_all(&[pixhigh, pixlow, pixhigh, pixlow, pixhigh, pixlow])
	// 			.unwrap()
	// 	}
	// }

	todo!()
}
