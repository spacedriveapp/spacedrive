use image::DynamicImage;
use std::{
	ffi::{OsStr, OsString},
	path::Path,
};

use crate::{
	consts,
	error::{Error, Result},
	heif_to_dynamic_image,
	raw::raw_to_dynamic_image,
};

pub struct ImageFormatter;

impl ImageFormatter {
	pub fn can_format_image(extension: &OsStr) -> bool {
		[
			consts::HEIF_EXTENSIONS.to_vec(),
			consts::RAW_EXTENSIONS.to_vec(),
		]
		.concat()
		.iter()
		.map(OsString::from)
		.any(|x| x == extension)
	}

	pub fn format_image(path: &Path) -> Result<DynamicImage> {
		let ext = path.extension().ok_or(Error::NoExtension)?;

		if !Self::can_format_image(ext) {
			return Err(Error::Unsupported);
		}

		if consts::HEIF_EXTENSIONS
			.iter()
			.map(OsString::from)
			.any(|x| x == ext)
		{
			heif_to_dynamic_image(path)?;
		}

		if consts::RAW_EXTENSIONS
			.iter()
			.map(OsString::from)
			.any(|x| x == ext)
		{
			raw_to_dynamic_image(path)?;
		}

		Ok(())
	}
}
