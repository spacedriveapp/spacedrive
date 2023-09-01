use crate::{
	consts::{HEIF_EXTENSIONS, SVG_EXTENSIONS},
	error::{Error, Result},
	generic::GenericHandler,
	svg::SvgHandler,
	ConvertImage,
};
use image::DynamicImage;
use std::{
	ffi::{OsStr, OsString},
	path::Path,
};

#[cfg(not(target_os = "linux"))]
use crate::heif::HeifHandler;

pub struct ImageFormatter;

impl ImageFormatter {
	fn match_to_handler(ext: &OsStr) -> Box<dyn ConvertImage> {
		let mut handler: Box<dyn ConvertImage> = Box::new(GenericHandler {});

		#[cfg(not(target_os = "linux"))]
		if HEIF_EXTENSIONS.iter().map(OsString::from).any(|x| x == ext) {
			handler = Box::new(HeifHandler {})
		}

		// raw next

		if SVG_EXTENSIONS.iter().map(OsString::from).any(|x| x == ext) {
			handler = Box::new(SvgHandler {})
		}

		handler
	}

	pub fn format_image(path: &Path) -> Result<DynamicImage> {
		let ext = path.extension().ok_or(Error::NoExtension)?;
		Self::match_to_handler(ext).handle_image(path)
	}
}
