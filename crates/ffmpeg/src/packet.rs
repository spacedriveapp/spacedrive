use crate::error::FFmpegError;
use ffmpeg_sys_next::{av_packet_alloc, av_packet_free, av_packet_unref, AVPacket};

pub(crate) struct FFmpegPacket {
	ref_: AVPacket,
	ptr: *mut AVPacket,
}

impl FFmpegPacket {
	pub(crate) fn new() -> Result<Self, FFmpegError> {
		let ptr = unsafe { av_packet_alloc() };
		if ptr.is_null() {
			return Err(FFmpegError::FrameAllocation);
		}
		Ok(Self {
			ref_: *unsafe { ptr.as_mut() }.ok_or(FFmpegError::NullError)?,
			ptr,
		})
	}

	pub(crate) fn as_ref(&self) -> &AVPacket {
		&self.ref_
	}

	pub(crate) fn as_mut(&mut self) -> &mut AVPacket {
		&mut self.ref_
	}

	pub(crate) fn reset(&mut self) -> &mut FFmpegPacket {
		unsafe { av_packet_unref(self.ptr) };
		return self;
	}
}

impl Drop for FFmpegPacket {
	fn drop(&mut self) {
		if !self.ptr.is_null() {
			unsafe { av_packet_free(&mut self.ptr) };
			self.ptr = std::ptr::null_mut();
		}
	}
}
