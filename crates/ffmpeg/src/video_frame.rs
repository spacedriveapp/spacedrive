use crate::error::FFmpegError;
use ffmpeg_sys_next::{av_frame_alloc, av_frame_free, AVFrame};

#[derive(Debug)]
pub(crate) enum FrameSource {
	VideoStream,
	Metadata,
}

#[derive(Debug, Default)]
pub(crate) struct VideoFrame {
	pub width: u32,
	pub height: u32,
	pub line_size: u32,
	pub data: Vec<u8>,
	pub source: Option<FrameSource>,
}

pub(crate) struct FFmpegFrame {
	data: *mut AVFrame,
}

impl FFmpegFrame {
	pub(crate) fn new() -> Result<Self, FFmpegError> {
		let data = unsafe { av_frame_alloc() };
		if data.is_null() {
			return Err(FFmpegError::FrameAllocation);
		}
		Ok(Self { data })
	}

	pub(crate) fn as_mut_ptr(&mut self) -> *mut AVFrame {
		self.data
	}
}

impl Drop for FFmpegFrame {
	fn drop(&mut self) {
		if !self.data.is_null() {
			unsafe { av_frame_free(&mut self.data) };
			self.data = std::ptr::null_mut();
		}
	}
}
