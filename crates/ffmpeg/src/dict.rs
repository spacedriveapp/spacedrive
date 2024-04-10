use crate::{error::Error, utils::check_error};

use std::{ffi::CString, ptr};

use ffmpeg_sys_next::{av_dict_free, av_dict_set, AVDictionary};

#[derive(Debug)]
pub(crate) struct FFmpegDict {
	data: *mut AVDictionary,
}

impl FFmpegDict {
	pub(crate) fn new() -> Self {
		Self {
			data: ptr::null_mut(),
		}
	}

	pub(crate) fn as_mut_ptr(&mut self) -> *mut AVDictionary {
		self.data
	}

	pub(crate) fn set(&mut self, key: CString, value: CString) -> Result<(), Error> {
		check_error(
			unsafe { av_dict_set(&mut self.data, key.as_ptr(), value.as_ptr(), 0) },
			"Fail to set dictionary key-value pair",
		)?;

		Ok(())
	}

	pub(crate) fn reset(&mut self, key: CString) -> Result<(), Error> {
		check_error(
			unsafe { av_dict_set(&mut self.data, key.as_ptr(), ptr::null(), 0) },
			"Fail to set dictionary key-value pair",
		)?;

		Ok(())
	}
}

impl Drop for FFmpegDict {
	fn drop(&mut self) {
		if !self.data.is_null() {
			unsafe { av_dict_free(&mut self.data) };
			self.data = std::ptr::null_mut();
		}
	}
}
