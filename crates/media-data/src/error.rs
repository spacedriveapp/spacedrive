use std::{io, num::ParseFloatError, path::Path};

#[derive(Debug, thiserror::Error)]
pub enum Error {
	#[error("there was an i/o error: {0} at file {}", .1.display())]
	FileIo(io::Error, Box<Path>),
	#[error("error from the exif crate: {0}")]
	Exif(#[from] exif::Error),
	#[error("error from exif crate: {0} on file {}", .1.display())]
	ExifOnFile(exif::Error, Box<Path>),
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
	#[error("error processing on a tokio thread")]
	TokioJoin(#[from] tokio::task::JoinError),
}

pub type Result<T> = std::result::Result<T, Error>;

impl From<(io::Error, Box<Path>)> for Error {
	fn from((e, p): (io::Error, Box<Path>)) -> Self {
		Self::FileIo(e, p)
	}
}

impl From<(exif::Error, Box<Path>)> for Error {
	fn from((e, p): (exif::Error, Box<Path>)) -> Self {
		Self::ExifOnFile(e, p)
	}
}
