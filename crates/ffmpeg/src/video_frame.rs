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
	ref_: AVFrame,
	ptr: *mut AVFrame,
}

impl FFmpegFrame {
	pub(crate) fn new() -> Result<Self, FFmpegError> {
		let ptr = unsafe { av_frame_alloc() };
		if ptr.is_null() {
			return Err(FFmpegError::FrameAllocation);
		}
		Ok(Self {
			ref_: *unsafe { ptr.as_mut() }.ok_or(FFmpegError::NullError)?,
			ptr,
		})
	}

	pub(crate) fn as_ref(&self) -> &AVFrame {
		&self.ref_
	}

	pub(crate) fn as_mut(&mut self) -> &mut AVFrame {
		&mut self.ref_
	}
}

impl Drop for FFmpegFrame {
	fn drop(&mut self) {
		if !self.ptr.is_null() {
			unsafe { av_frame_free(&mut self.ptr) };
			self.ptr = std::ptr::null_mut();
		}
	}
}
