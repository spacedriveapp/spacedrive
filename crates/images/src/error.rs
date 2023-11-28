use std::{num::TryFromIntError, path::Path};

pub type Result<T> = std::result::Result<T, Error>;

#[derive(thiserror::Error, Debug)]
pub enum Error {
	#[error("there was an i/o at path '{}' error: {0}", .1.display())]
	Io(std::io::Error, Box<Path>),

	#[error("the image provided is unsupported")]
	Unsupported,
	#[error("the image provided is too large (over 20MiB)")]
	TooLarge,
	#[error("the provided bit depth is invalid")]
	InvalidBitDepth,
	#[error("invalid path provided (non UTF-8)")]
	InvalidPath,
	#[error("the length of an input stream was invalid")]
	InvalidLength,

	// these errors are either: reliant on external (C dependencies), or are extremely niche
	// this means they rely on a lot of specific functionality, and therefore have specific errors
	#[cfg(feature = "heif")]
	#[error("error with libheif: {0}")]
	LibHeif(#[from] libheif_rs::HeifError),
	#[error("there was an error while converting the image to an `RgbImage`")]
	RgbImageConversion,
	#[error("error with pdfium: {0}")]
	Pdfium(#[from] pdfium_render::prelude::PdfiumError),
	#[error("error with usvg: {0}")]
	USvg(#[from] resvg::usvg::Error),
	#[error("failed to allocate `Pixbuf` while converting an SVG")]
	Pixbuf,
	#[error("error while loading the image (via the `image` crate): {0}")]
	Image(#[from] image::ImageError),
	// #[error("error while converting from raw")] // not enough rust support for it to be feasible
	// RawConversion,
	#[error("error while parsing integers")]
	TryFromInt(#[from] TryFromIntError),
}

#[cfg(feature = "rspc")]
impl From<Error> for rspc::Error {
	fn from(value: Error) -> Self {
		Self::new(rspc::ErrorCode::InternalServerError, value.to_string())
	}
}
