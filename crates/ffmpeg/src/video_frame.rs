use crate::error::FfmpegError;
use ffmpeg_sys_next::{av_frame_alloc, av_frame_free, AVFrame};

#[derive(Debug)]
pub enum FrameSource {
	VideoStream,
	Metadata,
}

#[derive(Debug, Default)]
pub struct VideoFrame {
	pub width: u32,
	pub height: u32,
	pub line_size: u32,
	pub data: Vec<u8>,
	pub source: Option<FrameSource>,
}

pub struct FfmpegFrame {
	data: *mut AVFrame,
}

impl FfmpegFrame {
	pub fn new() -> Result<Self, FfmpegError> {
		let data = unsafe { av_frame_alloc() };
		if data.is_null() {
			return Err(FfmpegError::FrameAllocation);
		}
		Ok(Self { data })
	}

	pub fn as_mut_ptr(&mut self) -> *mut AVFrame {
		self.data
	}
}

impl Drop for FfmpegFrame {
	fn drop(&mut self) {
		if !self.data.is_null() {
			unsafe { av_frame_free(&mut self.data) };
			self.data = std::ptr::null_mut();
		}
	}
}
