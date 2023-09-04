use std::num::TryFromIntError;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(thiserror::Error, Debug)]
pub enum Error {
	#[cfg(all(feature = "heif", not(target_os = "linux")))]
	#[error("error with libheif: {0}")]
	LibHeif(#[from] libheif_rs::HeifError),

	#[error("error with usvg: {0}")]
	USvg(#[from] resvg::usvg::Error),
	#[error("failed to allocate `Pixbuf` while converting an SVG")]
	Pixbuf,

	#[error("there was an error while converting a raw image: {0}")]
	RawLoader(#[from] rawloader::RawLoaderError),

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
	// #[error("there was an error with asynchronous i/o: {0}")]
	// AsyncIo(#[from] tokio::io::Error),
	// #[error("a blocking task failed to execute to completion")]
	// Join(#[from] JoinError),
}
