use std::num::TryFromIntError;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(thiserror::Error, Debug)]
pub enum Error {
	#[cfg(all(
		feature = "heif",
		any(not(any(target_os = "linux", target_os = "windows")), heif_images)
	))]
	#[error("error with libheif: {0}")]
	LibHeif(#[from] libheif_rs::HeifError),

	#[error("error with usvg: {0}")]
	USvg(#[from] resvg::usvg::Error),
	#[error("failed to allocate `Pixbuf` while converting an SVG")]
	Pixbuf,
	#[error("error while loading the image (via the `image` crate): {0}")]
	Image(#[from] image::ImageError),
	#[error("there was an i/o error: {0}")]
	Io(#[from] std::io::Error),
	#[error("there was an error while converting the image to an `RgbImage`")]
	RgbImageConversion,
	#[error("the image provided is unsupported")]
	Unsupported,
	#[error("the image provided is too large (over 20MiB)")]
	TooLarge,
	#[error("the provided bit depth is invalid")]
	InvalidBitDepth,
	#[error("invalid path provided (non UTF-8)")]
	InvalidPath,
	#[error("the image has an invalid length to be RGB")]
	InvalidLength,
	#[error("invalid path provided (it had no file extension)")]
	NoExtension,
	#[error("error while converting from raw")]
	RawConversion,
	#[error("error while parsing integers")]
	TryFromInt(#[from] TryFromIntError),
}

#[cfg(feature = "rspc")]
impl From<Error> for rspc::Error {
	fn from(value: Error) -> Self {
		Self::new(rspc::ErrorCode::InternalServerError, value.to_string())
	}
}
