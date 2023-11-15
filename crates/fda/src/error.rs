use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum Error {
	#[error("unable to access path: {0}")]
	PermissionDenied(PathBuf),

	#[cfg(target_os = "macos")]
	#[error("there was an error while prompting for full disk access")]
	FDAPromptError,
}

pub type Result<T> = std::result::Result<T, Error>;
