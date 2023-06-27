use std::ffi::c_int;
use std::path::PathBuf;
use thiserror::Error;
use tokio::task::JoinError;

use ffmpeg_sys_next::{
	AVERROR_BSF_NOT_FOUND, AVERROR_BUFFER_TOO_SMALL, AVERROR_BUG, AVERROR_BUG2,
	AVERROR_DECODER_NOT_FOUND, AVERROR_DEMUXER_NOT_FOUND, AVERROR_ENCODER_NOT_FOUND, AVERROR_EOF,
	AVERROR_EXIT, AVERROR_EXTERNAL, AVERROR_FILTER_NOT_FOUND, AVERROR_HTTP_BAD_REQUEST,
	AVERROR_HTTP_FORBIDDEN, AVERROR_HTTP_NOT_FOUND, AVERROR_HTTP_OTHER_4XX,
	AVERROR_HTTP_SERVER_ERROR, AVERROR_HTTP_UNAUTHORIZED, AVERROR_INVALIDDATA,
	AVERROR_MUXER_NOT_FOUND, AVERROR_OPTION_NOT_FOUND, AVERROR_PATCHWELCOME,
	AVERROR_PROTOCOL_NOT_FOUND, AVERROR_STREAM_NOT_FOUND, AVERROR_UNKNOWN, AVUNERROR,
};

/// Error type for the library.
#[derive(Error, Debug)]
pub enum ThumbnailerError {
	#[error("I/O Error: {0}")]
	Io(#[from] std::io::Error),
	#[error("Path conversion error: Path: {0:#?}")]
	PathConversion(PathBuf),
	#[error("FFmpeg internal error: {0}")]
	Ffmpeg(#[from] FfmpegError),
	#[error("FFmpeg internal error: {0}; Reason: {1}")]
	FfmpegWithReason(FfmpegError, String),
	#[error("Failed to decode video frame")]
	FrameDecodeError,
	#[error("Failed to seek video")]
	SeekError,
	#[error("Seek not allowed")]
	SeekNotAllowed,
	#[error("Received an invalid seek percentage: {0}")]
	InvalidSeekPercentage(f32),
	#[error("Received an invalid quality, expected range [0.0, 100.0], received: {0}")]
	InvalidQuality(f32),
	#[error("Background task failed: {0}")]
	BackgroundTaskFailed(#[from] JoinError),
}

/// Enum to represent possible errors from FFmpeg library
///
/// Extracted from https://ffmpeg.org/doxygen/trunk/group__lavu__error.html
#[derive(Error, Debug)]
pub enum FfmpegError {
	#[error("Bitstream filter not found")]
	BitstreamFilterNotFound,
	#[error("Internal bug, also see AVERROR_BUG2")]
	InternalBug,
	#[error("Buffer too small")]
	BufferTooSmall,
	#[error("Decoder not found")]
	DecoderNotFound,
	#[error("Demuxer not found")]
	DemuxerNotFound,
	#[error("Encoder not found")]
	EncoderNotFound,
	#[error("End of file")]
	Eof,
	#[error("Immediate exit was requested; the called function should not be restarted")]
	Exit,
	#[error("Generic error in an external library")]
	External,
	#[error("Filter not found")]
	FilterNotFound,
	#[error("Invalid data found when processing input")]
	InvalidData,
	#[error("Muxer not found")]
	MuxerNotFound,
	#[error("Option not found")]
	OptionNotFound,
	#[error("Not yet implemented in FFmpeg, patches welcome")]
	NotImplemented,
	#[error("Protocol not found")]
	ProtocolNotFound,
	#[error("Stream not found")]
	StreamNotFound,
	#[error("This is semantically identical to AVERROR_BUG it has been introduced in Libav after our AVERROR_BUG and with a modified value")]
	InternalBug2,
	#[error("Unknown error, typically from an external library")]
	Unknown,
	#[error("Requested feature is flagged experimental. Set strict_std_compliance if you really want to use it")]
	Experimental,
	#[error("Input changed between calls. Reconfiguration is required. (can be OR-ed with AVERROR_OUTPUT_CHANGED)")]
	InputChanged,
	#[error("Output changed between calls. Reconfiguration is required. (can be OR-ed with AVERROR_INPUT_CHANGED)")]
	OutputChanged,
	#[error("HTTP Bad Request: 400")]
	HttpBadRequest,
	#[error("HTTP Unauthorized: 401")]
	HttpUnauthorized,
	#[error("HTTP Forbidden: 403")]
	HttpForbidden,
	#[error("HTTP Not Found: 404")]
	HttpNotFound,
	#[error("Other HTTP error: 4xx")]
	HttpOther4xx,
	#[error("HTTP Internal Server Error: 500")]
	HttpServerError,
	#[error("Other OS error, errno = {0}")]
	OtherOSError(c_int),
	#[error("Frame allocation error")]
	FrameAllocation,
	#[error("Video Codec allocation error")]
	VideoCodecAllocation,
	#[error("Filter Graph allocation error")]
	FilterGraphAllocation,
	#[error("Codec Open Error")]
	CodecOpen,
}

impl From<c_int> for FfmpegError {
	fn from(code: c_int) -> Self {
		match code {
			AVERROR_BSF_NOT_FOUND => FfmpegError::BitstreamFilterNotFound,
			AVERROR_BUG => FfmpegError::InternalBug,
			AVERROR_BUFFER_TOO_SMALL => FfmpegError::BufferTooSmall,
			AVERROR_DECODER_NOT_FOUND => FfmpegError::DecoderNotFound,
			AVERROR_DEMUXER_NOT_FOUND => FfmpegError::DemuxerNotFound,
			AVERROR_ENCODER_NOT_FOUND => FfmpegError::EncoderNotFound,
			AVERROR_EOF => FfmpegError::Eof,
			AVERROR_EXIT => FfmpegError::Exit,
			AVERROR_EXTERNAL => FfmpegError::External,
			AVERROR_FILTER_NOT_FOUND => FfmpegError::FilterNotFound,
			AVERROR_INVALIDDATA => FfmpegError::InvalidData,
			AVERROR_MUXER_NOT_FOUND => FfmpegError::MuxerNotFound,
			AVERROR_OPTION_NOT_FOUND => FfmpegError::OptionNotFound,
			AVERROR_PATCHWELCOME => FfmpegError::NotImplemented,
			AVERROR_PROTOCOL_NOT_FOUND => FfmpegError::ProtocolNotFound,
			AVERROR_STREAM_NOT_FOUND => FfmpegError::StreamNotFound,
			AVERROR_BUG2 => FfmpegError::InternalBug2,
			AVERROR_UNKNOWN => FfmpegError::Unknown,
			AVERROR_HTTP_BAD_REQUEST => FfmpegError::HttpBadRequest,
			AVERROR_HTTP_UNAUTHORIZED => FfmpegError::HttpUnauthorized,
			AVERROR_HTTP_FORBIDDEN => FfmpegError::HttpForbidden,
			AVERROR_HTTP_NOT_FOUND => FfmpegError::HttpNotFound,
			AVERROR_HTTP_OTHER_4XX => FfmpegError::HttpOther4xx,
			AVERROR_HTTP_SERVER_ERROR => FfmpegError::HttpServerError,
			other => FfmpegError::OtherOSError(AVUNERROR(other)),
		}
	}
}
