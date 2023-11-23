#[derive(Debug, thiserror::Error)]
pub enum Error {
	#[cfg(target_os = "macos")]
	#[error("there was an error while prompting for full disk access")]
	FDAPromptError,
}

pub type Result<T> = std::result::Result<T, Error>;
