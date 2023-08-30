use image::DynamicImage;
use std::{
	ffi::{OsStr, OsString},
	path::Path,
};

use crate::{
	consts,
	error::{Error, Result},
	raw::raw_to_dynamic_image,
};

#[cfg(not(target_os = "linux"))]
use crate::heif_to_dynamic_image;

pub struct ImageFormatter;

impl ImageFormatter {
	// pub fn can_format_image(extension: &OsStr) -> bool {
	// 	[
	// 		#[cfg(not(target_os = "linux"))]
	// 		consts::HEIF_EXTENSIONS.to_vec(),
	// 		consts::RAW_EXTENSIONS.to_vec(),
	// 	]
	// 	.concat()
	// 	.iter()
	// 	.map(OsString::from)
	// 	.any(|x| x == extension)
	// }

	pub fn format_image(path: &Path) -> Result<DynamicImage> {
		let ext = path.extension().ok_or(Error::NoExtension)?;

		#[cfg(not(target_os = "linux"))]
		if consts::HEIF_EXTENSIONS
			.iter()
			.map(OsString::from)
			.any(|x| x == ext)
		{
			return heif_to_dynamic_image(path);
		}

		if consts::RAW_EXTENSIONS
			.iter()
			.map(OsString::from)
			.any(|x| x == ext)
		{
			return raw_to_dynamic_image(path);
		}

		Ok(image::open(path)?)
	}
}
