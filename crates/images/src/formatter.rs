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

#[cfg(all(
	feature = "heif",
	any(not(any(target_os = "linux", target_os = "windows")), heif_images)
))]
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

	#[cfg(all(
		feature = "heif",
		any(not(any(target_os = "linux", target_os = "windows")), heif_images)
	))]
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

	handler
}
