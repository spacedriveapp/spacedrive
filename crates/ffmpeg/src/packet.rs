use crate::error::FFmpegError;
use ffmpeg_sys_next::{av_packet_alloc, av_packet_free, av_packet_unref, AVPacket};

pub struct FFmpegPacket(*mut AVPacket);

impl FFmpegPacket {
	pub(crate) fn new() -> Result<Self, FFmpegError> {
		let ptr = unsafe { av_packet_alloc() };
		if ptr.is_null() {
			return Err(FFmpegError::NullError);
		}
		Ok(Self(ptr))
	}

	pub(crate) fn as_ptr(&self) -> *mut AVPacket {
		self.0
	}

	pub(crate) fn as_ref(&self) -> Option<&AVPacket> {
		unsafe { self.0.as_ref() }
	}

	pub(crate) fn unref(&mut self) {
		if !self.0.is_null() {
			unsafe { av_packet_unref(self.0) };
		}
	}
}

impl Drop for FFmpegPacket {
	fn drop(&mut self) {
		if !self.0.is_null() {
			unsafe { av_packet_free(&mut self.0) };
			self.0 = std::ptr::null_mut();
		}
	}
}
