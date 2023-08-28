pub type Result<T> = std::result::Result<T, Error>;

#[derive(thiserror::Error, Debug)]
pub enum Error {
	#[error("error with libheif: {0}")]
	LibHeif(#[from] libheif_rs::HeifError),
	#[error("error while loading the image (via the `image` crate): {0}")]
	Image(#[from] image::ImageError),
	#[error("io error: {0}")]
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
	#[error("invalid path provided (it had no file extension)")]
	NoExtension,
}
