// use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum Error {
	// #[error("unable to access path: {0}")]
	// PermissionDenied(PathBuf),

	// #[error("the path provided is invalid and likely doesn't exist: \"{0}\"")]
	// PathInvalid(PathBuf),
	#[cfg(target_os = "macos")]
	#[error("there was an error while prompting for full disk access")]
	FDAPromptError,
}

pub type Result<T> = std::result::Result<T, Error>;
