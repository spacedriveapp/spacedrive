use sd_utils::error::FileIOError;

#[derive(Debug, thiserror::Error)]
pub enum Error {
	#[error("error from the exif crate: {0}")]
	Exif(#[from] exif::Error),
	#[cfg(feature = "ffmpeg")]
	#[error("error from the ffmpeg crate: {0}")]
	FFmpeg(#[from] sd_ffmpeg::Error),
	#[cfg(not(feature = "ffmpeg"))]
	#[error("ffmpeg not available")]
	NoFFmpeg,
	#[error("there was an error while parsing time with chrono: {0}")]
	Chrono(#[from] chrono::ParseError),
	#[error("there was an error while converting between types")]
	Conversion,
	#[error("there was an error while parsing the location of an image")]
	MediaLocationParse,

	#[error("serde error {0}")]
	Serde(#[from] serde_json::Error),
	#[error("failed to join tokio task: {0}")]
	TokioJoinHandle(#[from] tokio::task::JoinError),

	#[error(transparent)]
	FileIO(#[from] FileIOError),
}

pub type Result<T> = std::result::Result<T, Error>;
