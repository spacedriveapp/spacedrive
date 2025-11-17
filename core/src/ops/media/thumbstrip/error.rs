//! Thumbstrip generation error types

use thiserror::Error;

pub type ThumbstripResult<T> = Result<T, ThumbstripError>;

#[derive(Error, Debug)]
pub enum ThumbstripError {
	#[error("File not found: {0}")]
	FileNotFound(String),

	#[error("Unsupported format: {0}")]
	UnsupportedFormat(String),

	#[error("Video processing failed: {0}")]
	VideoProcessing(String),

	#[error("Invalid quality: {0} (must be 0-100)")]
	InvalidQuality(u8),

	#[error("Invalid grid dimensions: {0}x{1}")]
	InvalidGridDimensions(u32, u32),

	#[error("No frames extracted from video")]
	NoFrames,

	#[error("FFmpeg error: {0}")]
	FFmpeg(String),

	#[error("IO error: {0}")]
	Io(#[from] std::io::Error),

	#[error("Image processing error: {0}")]
	ImageProcessing(String),

	#[error("Other error: {0}")]
	Other(String),
}

impl ThumbstripError {
	pub fn other(msg: impl Into<String>) -> Self {
		Self::Other(msg.into())
	}

	pub fn video_processing(msg: impl Into<String>) -> Self {
		Self::VideoProcessing(msg.into())
	}

	pub fn unsupported_format(format: impl Into<String>) -> Self {
		Self::UnsupportedFormat(format.into())
	}
}

// Conversion from sd-ffmpeg errors
#[cfg(feature = "ffmpeg")]
impl From<sd_ffmpeg::Error> for ThumbstripError {
	fn from(err: sd_ffmpeg::Error) -> Self {
		Self::FFmpeg(err.to_string())
	}
}
