use image::DynamicImage;
use lazy_static::lazy_static;
use pdfium_render::prelude::{PdfPageRenderRotation, PdfRenderConfig, Pdfium, PdfiumError};
use std::path::Path;
use thiserror::Error;
use tokio::{
	fs,
	task::{spawn_blocking, JoinError},
};
use tracing::error;

type PdfResult<T> = Result<T, PdfError>;

const THUMB_SIZE: i32 = 512;

// This path is relative to the running binary
#[cfg(windows)]
const BINDING_LOCATION: &str = "./";
#[cfg(unix)]
const BINDING_LOCATION: &str = if cfg!(target_os = "macos") {
	"../Frameworks/PDFium.framework"
} else {
	"../lib/"
};

lazy_static! {
	static ref PDFIUM: Option<Pdfium> =
		Pdfium::bind_to_library(Pdfium::pdfium_platform_library_name_at_path({
			BINDING_LOCATION
		}))
		.or_else(|_| Pdfium::bind_to_system_library())
		.map(Pdfium::new)
		.map_err(|err| error!("{err:#?}"))
		.ok();
}

/// The maximum file size that an image can be in order to have a thumbnail generated.
///
/// This value is in MiB.
const PDF_MAXIMUM_FILE_SIZE: u64 = 1048576 * 20;

#[derive(Error, Debug)]
pub enum PdfError {
	#[error("error with pdfium: {0}")]
	Pdfium(#[from] PdfiumError),
	#[error("error while loading the image (via the `image` crate): {0}")]
	Image(#[from] image::ImageError),
	#[error("io error: {0} at {}", .1.display())]
	Io(std::io::Error, Box<Path>),
	#[error("Blocking task failed to execute to completion.")]
	Join(#[from] JoinError),
	#[error("failed to allocate `Pixbuf`")]
	Pixbuf,
	#[error("there was an error while converting the image to an `RgbImage`")]
	RgbImageConversion,
	#[error("failed to calculate thumbnail size")]
	InvalidSize,
	#[error("the image provided is too large (over 20MiB)")]
	TooLarge,
	#[error("no available bindings")]
	Bindings,
}

pub async fn pdf_to_dynamic_image(path: &Path) -> PdfResult<DynamicImage> {
	let pdfium = PDFIUM.as_ref().ok_or(PdfError::Bindings)?;

	if fs::metadata(path)
		.await
		.map_err(|e| PdfError::Io(e, path.to_path_buf().into_boxed_path()))?
		.len() > PDF_MAXIMUM_FILE_SIZE
	{
		return Err(PdfError::TooLarge);
	}

	let data = fs::read(path)
		.await
		.map_err(|e| PdfError::Io(e, path.to_path_buf().into_boxed_path()))?;

	let render_config = PdfRenderConfig::new()
		.set_target_width(THUMB_SIZE)
		.set_maximum_height(THUMB_SIZE)
		.rotate_if_landscape(PdfPageRenderRotation::Degrees90, true);

	spawn_blocking(move || -> Result<_, PdfError> {
		Ok(pdfium
			.load_pdf_from_byte_vec(data, None)?
			.pages()
			.first()?
			.render_with_config(&render_config)?
			.as_image())
	})
	.await?
}
