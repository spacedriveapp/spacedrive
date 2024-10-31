use std::{io, path::Path};

use thiserror::Error;
use tracing::error;

pub fn report_error<E: std::error::Error + std::fmt::Debug>(
	message: &'static str,
) -> impl Fn(E) -> E {
	move |e| {
		error!(?e, "{message}");
		e
	}
}

#[derive(Debug, Error)]
#[error("error accessing path: '{}'", .path.display())]
pub struct FileIOError {
	pub path: Box<Path>,
	#[source]
	pub source: io::Error,
	pub maybe_context: Option<&'static str>,
}

impl<P: AsRef<Path>> From<(P, io::Error)> for FileIOError {
	fn from((path, source): (P, io::Error)) -> Self {
		Self {
			path: path.as_ref().into(),
			source,
			maybe_context: None,
		}
	}
}

impl<P: AsRef<Path>> From<(P, io::Error, &'static str)> for FileIOError {
	fn from((path, source, context): (P, io::Error, &'static str)) -> Self {
		Self {
			path: path.as_ref().into(),
			source,
			maybe_context: Some(context),
		}
	}
}

impl From<FileIOError> for rspc::Error {
	fn from(value: FileIOError) -> Self {
		Self::with_cause(
			rspc::ErrorCode::InternalServerError,
			value
				.maybe_context
				.unwrap_or("Error accessing file system")
				.to_string(),
			value,
		)
	}
}

#[derive(Debug, Error)]
#[error("received a non UTF-8 path: <lossy_path='{}'>", .0.to_string_lossy())]
pub struct NonUtf8PathError(pub Box<Path>);
