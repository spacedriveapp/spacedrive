use std::num::ParseFloatError;

#[derive(Debug, thiserror::Error)]
pub enum Error {
	#[error("there was an i/o error: {0}")]
	Io(#[from] std::io::Error),
	#[error("error from the exif crate: {0}")]
	Exif(#[from] exif::Error),
	#[error("there was an error while parsing time with chrono: {0}")]
	Chrono(#[from] chrono::ParseError),
	#[error("there was an error while converting between types")]
	Conversion,
	#[error("there was an error while parsing the location of an image")]
	MediaLocationParse,
	#[error("there was an error while parsing a float")]
	FloatParse(#[from] ParseFloatError),
	#[error("there was an error while initializing the exif reader")]
	Init,

	#[error("serde error {0}")]
	Serde(#[from] serde_json::Error),
}

pub type Result<T> = std::result::Result<T, Error>;
