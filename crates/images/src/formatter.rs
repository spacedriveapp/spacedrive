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
	let ext = path
		.as_ref()
		.extension()
		.map_or_else(|| Err(Error::NoExtension), |e| Ok(e.to_ascii_lowercase()))?;
	match_to_handler(&ext).handle_image(path.as_ref())
}

#[allow(clippy::useless_let_if_seq)]
fn match_to_handler(ext: &OsStr) -> Box<dyn ImageHandler> {
	let mut handler: Box<dyn ImageHandler> = Box::new(GenericHandler {});

	#[cfg(feature = "heif")]
	if consts::HEIF_EXTENSIONS
		.iter()
		.map(OsString::from)
		.any(|x| x == ext)
	{
		handler = Box::new(HeifHandler {});
	}

	if consts::SVG_EXTENSIONS
		.iter()
		.map(OsString::from)
		.any(|x| x == ext)
	{
		handler = Box::new(SvgHandler {});
	}

	if ext == consts::PDF_EXTENSION {
		handler = Box::new(PdfHandler {});
	}

	handler
}
