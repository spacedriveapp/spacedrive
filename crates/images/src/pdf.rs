use std::{
	borrow::ToOwned,
	env::current_exe,
	path::{Path, PathBuf},
};

use crate::{consts::PDF_RENDER_SIZE, Error::PdfiumBinding, ImageHandler, Result};
use image::DynamicImage;
use once_cell::sync::Lazy;
use pdfium_render::prelude::{PdfPageRenderRotation, PdfRenderConfig, Pdfium};
use tracing::error;

// This path must be relative to the running binary
#[cfg(windows)]
const BINDING_LOCATION: &str = ".";
#[cfg(unix)]
const BINDING_LOCATION: &str = if cfg!(target_os = "macos") {
	"../Frameworks/FFMpeg.framework/Libraries"
} else {
	"../lib/spacedrive"
};

static PDFIUM: Lazy<Option<Pdfium>> = Lazy::new(|| {
	let lib_name = Pdfium::pdfium_platform_library_name();
	let lib_path = current_exe()
		.ok()
		.and_then(|exe_path| {
			exe_path.parent().and_then(|parent_path| {
				match parent_path
					.join(BINDING_LOCATION)
					.join(&lib_name)
					.canonicalize()
				{
					Ok(lib_path) => lib_path.to_str().map(ToOwned::to_owned),
					Err(err) => {
						error!("{err:#?}");
						None
					}
				}
			})
		})
		.unwrap_or_else(|| {
			#[allow(clippy::expect_used)]
			PathBuf::from(BINDING_LOCATION)
				.join(&lib_name)
				.to_str()
				.expect("We are converting valid strs to PathBuf then back, it should not fail")
				.to_owned()
		});

	Pdfium::bind_to_library(lib_path)
		.or_else(|err| {
			error!("{err:#?}");
			Pdfium::bind_to_system_library()
		})
		.map(Pdfium::new)
		.map_err(|err| error!("{err:#?}"))
		.ok()
});

pub struct PdfHandler {}

impl ImageHandler for PdfHandler {
	fn handle_image(&self, path: &Path) -> Result<DynamicImage> {
		let pdfium = PDFIUM.as_ref().ok_or(PdfiumBinding)?;

		let render_config = PdfRenderConfig::new()
			.set_target_width(PDF_RENDER_SIZE.try_into()?)
			.set_maximum_height(PDF_RENDER_SIZE.try_into()?)
			.rotate_if_landscape(PdfPageRenderRotation::Degrees90, true);

		Ok(pdfium
			.load_pdf_from_file(path, None)?
			.pages()
			.first()?
			.render_with_config(&render_config)?
			.as_image())
	}
}
