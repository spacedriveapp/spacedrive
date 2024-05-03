use crate::error::FFmpegError;
use ffmpeg_sys_next::{av_frame_alloc, av_frame_free, AVFrame};

pub struct FFmpegFrame(*mut AVFrame);

impl FFmpegFrame {
	pub(crate) fn new() -> Result<Self, FFmpegError> {
		let ptr = unsafe { av_frame_alloc() };
		if ptr.is_null() {
			return Err(FFmpegError::FrameAllocation);
		}
		Ok(Self(ptr))
	}

	pub(crate) fn as_ref(&self) -> &AVFrame {
		unsafe { self.0.as_ref() }.expect("initialized on struct creation")
	}

	pub(crate) fn as_mut(&mut self) -> &mut AVFrame {
		unsafe { self.0.as_mut() }.expect("initialized on struct creation")
	}
}

impl Drop for FFmpegFrame {
	fn drop(&mut self) {
		if !self.0.is_null() {
			unsafe { av_frame_free(&mut self.0) };
			self.0 = std::ptr::null_mut();
		}
	}
}
