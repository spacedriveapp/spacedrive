use crate::{
	consts,
	error::{Error, Result},
	generic::GenericHandler,
	pdf::PdfHandler,
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
	match_to_handler(path.as_ref().extension())?.handle_image(path.as_ref())
}

pub fn convert_image(path: impl AsRef<Path>, desired_ext: &OsStr) -> Result<DynamicImage> {
	match_to_handler(path.as_ref().extension())?
		.convert_image(match_to_handler(Some(desired_ext))?, path.as_ref())
}

#[allow(clippy::useless_let_if_seq)]
fn match_to_handler(ext: Option<&OsStr>) -> Result<Box<dyn ImageHandler>> {
	let ext = ext.map(OsStr::to_ascii_lowercase).unwrap_or_default();
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

	if consts::PDF_EXTENSIONS
		.iter()
		.map(OsString::from)
		.any(|x| x == ext)
	{
		handler = Some(Box::new(PdfHandler {}));
	}

	handler.ok_or(Error::Unsupported)
}
