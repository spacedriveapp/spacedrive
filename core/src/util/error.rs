use std::{io, path::Path};

use thiserror::Error;

#[derive(Debug, Error)]
#[error("error accessing path: '{}'", .path.display())]
pub struct FileIOError {
	pub path: Box<Path>,
	#[source]
	pub source: io::Error,
}

impl<P: AsRef<Path>> From<(P, io::Error)> for FileIOError {
	fn from((path, source): (P, io::Error)) -> Self {
		Self {
			path: path.as_ref().into(),
			source,
		}
	}
}

#[derive(Debug, Error)]
#[error("received a non UTF-8 path: <lossy_path='{}'>", .0.to_string_lossy())]
pub struct NonUtf8PathError(pub Box<Path>);
