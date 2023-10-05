use crate::{
	consts,
	error::{Error, Result},
	generic::GenericHandler,
	svg::SvgHandler,
	ImageHandler,
};
use image::DynamicImage;
use std::{
	ffi::{OsStr, OsString},
	path::Path,
};

#[cfg(feature = "heif")]
use crate::heif::HeifHandler;

pub fn format_image(path: impl AsRef<Path>) -> Result<DynamicImage> {
	match_to_handler(&get_ext(&path)?)?.handle_image(path.as_ref())
}

pub fn convert_image(path: impl AsRef<Path>, desired_ext: &OsStr) -> Result<DynamicImage> {
	match_to_handler(&get_ext(&path)?)?.convert_image(match_to_handler(desired_ext)?, path.as_ref())
}

#[inline]
fn get_ext(path: impl AsRef<Path>) -> Result<OsString> {
	path.as_ref()
		.extension()
		.map_or_else(|| Err(Error::NoExtension), |e| Ok(e.to_ascii_lowercase()))
}

#[allow(clippy::useless_let_if_seq)]
fn match_to_handler(ext: &OsStr) -> Result<Box<dyn ImageHandler>> {
	let mut handler: Option<Box<dyn ImageHandler>> = None;

	if consts::GENERIC_EXTENSIONS
		.iter()
		.map(OsString::from)
		.any(|x| x == ext)
	{
		handler = Some(Box::new(GenericHandler {}));
	}

	#[cfg(feature = "heif")]
	if consts::HEIF_EXTENSIONS
		.iter()
		.map(OsString::from)
		.any(|x| x == ext)
	{
		handler = Some(Box::new(HeifHandler {}));
	}

	if consts::SVG_EXTENSIONS
		.iter()
		.map(OsString::from)
		.any(|x| x == ext)
	{
		handler = Some(Box::new(SvgHandler {}));
	}

	handler.ok_or(Error::Unsupported)
}
