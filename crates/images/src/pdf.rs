use std::{
	borrow::ToOwned,
	env::current_exe,
	path::{Path, PathBuf},
};

use crate::{consts::PDF_RENDER_WIDTH, ImageHandler, Result};
use image::DynamicImage;
use once_cell::sync::Lazy;
use pdfium_render::prelude::{PdfPageRenderRotation, PdfRenderConfig, Pdfium};
use tracing::error;

// This path must be relative to the running binary
#[cfg(windows)]
const BINDING_LOCATION: &str = ".";
#[cfg(unix)]
const BINDING_LOCATION: &str = if cfg!(target_os = "macos") {
	"../Frameworks/Spacedrive.framework/Libraries"
} else {
	"../lib/spacedrive"
};

static PDFIUM_LIB: Lazy<String> = Lazy::new(|| {
	let lib_name = Pdfium::pdfium_platform_library_name();
	current_exe()
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
		})
});

static PDFIUM_RENDER_CONFIG: Lazy<PdfRenderConfig> = Lazy::new(|| {
	PdfRenderConfig::new()
		.set_target_width(PDF_RENDER_WIDTH)
		.rotate_if_landscape(PdfPageRenderRotation::Degrees90, true)
		.render_form_data(false)
		.render_annotations(false)
});

pub struct PdfHandler {}

impl ImageHandler for PdfHandler {
	fn handle_image(&self, path: &Path) -> Result<DynamicImage> {
		let pdfium = Pdfium::new(Pdfium::bind_to_library(PDFIUM_LIB.as_str()).or_else(|err| {
			error!("{err:#?}");
			Pdfium::bind_to_system_library()
		})?);

		let pdf = pdfium.load_pdf_from_file(path, None)?;
		let first_page = pdf.pages().first()?;
		let image = first_page
			.render_with_config(&PDFIUM_RENDER_CONFIG)?
			.as_image();

		Ok(image)
	}
}
