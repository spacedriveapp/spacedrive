//! Proxy generation error types

use thiserror::Error;

pub type ProxyResult<T> = Result<T, ProxyError>;

#[derive(Error, Debug)]
pub enum ProxyError {
	#[error("File not found: {0}")]
	FileNotFound(String),

	#[error("Unsupported format: {0}")]
	UnsupportedFormat(String),

	#[error("Video encoding failed: {0}")]
	EncodingFailed(String),

	#[error("FFmpeg not found in PATH")]
	FFmpegNotFound,

	#[error("FFmpeg process failed with status: {0}")]
	FFmpegProcessFailed(i32),

	#[error("Invalid preset: {0}")]
	InvalidPreset(String),

	#[error("No video duration")]
	NoVideoDuration,

	#[error("IO error: {0}")]
	Io(#[from] std::io::Error),

	#[error("Other error: {0}")]
	Other(String),
}

impl ProxyError {
	pub fn other(msg: impl Into<String>) -> Self {
		Self::Other(msg.into())
	}

	pub fn encoding_failed(msg: impl Into<String>) -> Self {
		Self::EncodingFailed(msg.into())
	}
}
