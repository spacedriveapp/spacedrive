use std::{
	num::ParseFloatError,
	path::{Path, PathBuf},
};

#[derive(Debug, thiserror::Error)]
pub enum Error {
	#[error("there was an i/o error {0} at {}", .1.display())]
	Io(std::io::Error, Box<Path>),
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
	#[error("the file provided at ({0}) contains no exif data")]
	NoExifDataOnPath(PathBuf),
	#[error("the slice provided contains no exif data")]
	NoExifDataOnSlice,

	#[error("serde error {0}")]
	Serde(#[from] serde_json::Error),
}

pub type Result<T> = std::result::Result<T, Error>;
