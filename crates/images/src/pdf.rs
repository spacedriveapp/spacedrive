use std::path::Path;

use crate::{consts::PDF_RENDER_SIZE, Error::PdfiumBinding, ImageHandler, Result};
use image::DynamicImage;
use once_cell::sync::Lazy;
use pdfium_render::prelude::{PdfPageRenderRotation, PdfRenderConfig, Pdfium};
use tracing::error;

// This path is relative to the running binary
#[cfg(windows)]
const BINDING_LOCATION: &str = "./";
#[cfg(unix)]
const BINDING_LOCATION: &str = if cfg!(target_os = "macos") {
	"../Frameworks/FFMpeg.framework/Libraries/"
} else {
	"../lib/spacedrive"
};

static PDFIUM: Lazy<Option<Pdfium>> = Lazy::new(|| {
	Pdfium::bind_to_library(Pdfium::pdfium_platform_library_name_at_path({
		BINDING_LOCATION
	}))
	.or_else(|_| Pdfium::bind_to_system_library())
	.map(Pdfium::new)
	.map_err(|err| error!("{err:#?}"))
	.ok()
});

pub struct PdfHandler {}

impl ImageHandler for PdfHandler {
	fn maximum_size(&self) -> u64 {
		// Pdfium will only load the portions of the document it actually needs into memory.
		u64::MAX
	}

	fn validate_image(&self, _bits_per_pixel: u8, _length: usize) -> Result<()> {
		Ok(())
	}

	fn handle_image(&self, path: &Path) -> Result<DynamicImage> {
		let pdfium = PDFIUM.as_ref().ok_or(PdfiumBinding)?;

		let render_config = PdfRenderConfig::new()
			.set_target_width(PDF_RENDER_SIZE)
			.set_maximum_height(PDF_RENDER_SIZE)
			.rotate_if_landscape(PdfPageRenderRotation::Degrees90, true);

		Ok(pdfium
			.load_pdf_from_file(path, None)?
			.pages()
			.first()?
			.render_with_config(&render_config)?
			.as_image())
	}
}
