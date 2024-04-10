use crate::{dict::FFmpegDict, error::Error, utils::check_error};

use ffmpeg_sys_next::{
	avformat_close_input, avformat_find_stream_info, avformat_open_input, AVFormatContext,
};

use std::{ffi::CString, ptr};

#[derive(Debug)]
pub(crate) struct FFmpegFormatContext {
	data: *mut AVFormatContext,
}

impl FFmpegFormatContext {
	pub(crate) fn as_mut_ptr(&mut self) -> *mut AVFormatContext {
		self.data
	}

	pub(crate) fn open_file(filename: CString, options: &mut FFmpegDict) -> Result<Self, Error> {
		let mut ctx = Self {
			data: ptr::null_mut(),
		};

		check_error(
			unsafe {
				avformat_open_input(
					&mut ctx.data,
					filename.as_ptr(),
					ptr::null(),
					&mut options.as_mut_ptr(),
				)
			},
			"Fail to open an input stream and read the header",
		)?;

		Ok(ctx)
	}

	pub(crate) fn find_stream_info(&self) -> Result<(), Error> {
		check_error(
			unsafe { avformat_find_stream_info(self.data, ptr::null_mut()) },
			"Fail to read packets of a media file to get stream information",
		)?;

		Ok(())
	}
}

impl Drop for FFmpegFormatContext {
	fn drop(&mut self) {
		if !self.data.is_null() {
			unsafe { avformat_close_input(&mut self.data) };
			self.data = std::ptr::null_mut();
		}
	}
}
