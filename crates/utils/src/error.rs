use std::{fmt::Display, path::Path};

use thiserror::Error;
use tracing::error;

/// Report an error with tracing
pub fn report_error(res: &Result<(), impl Display>) {
	if let Err(e) = res {
		error!("{e:#}");
	}
}

/// File I/O error that includes the path that caused the error
#[derive(Error, Debug)]
pub struct FileIOError {
	pub path: Box<Path>,
	#[source]
	pub source: std::io::Error,
	pub maybe_context: Option<String>,
}

impl Display for FileIOError {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		write!(
			f,
			"file I/O error{}: {}; path: '{}'",
			self.maybe_context
				.as_ref()
				.map(|ctx| format!(" ({ctx})"))
				.unwrap_or_default(),
			self.source,
			self.path.display()
		)
	}
}

impl FileIOError {
	pub fn from_std_io_err(path: impl AsRef<Path>, source: std::io::Error) -> Self {
		Self {
			path: path.as_ref().into(),
			source,
			maybe_context: None,
		}
	}

	pub fn from_std_io_err_with_msg(
		path: impl AsRef<Path>,
		source: std::io::Error,
		msg: impl Into<String>,
	) -> Self {
		Self {
			path: path.as_ref().into(),
			source,
			maybe_context: Some(msg.into()),
		}
	}
}

/// Error for paths that contain non-UTF8 characters
#[derive(Error, Debug)]
#[error("Received a non UTF-8 path: <path='{0:?}'>")]
pub struct NonUtf8PathError(pub Box<Path>);
