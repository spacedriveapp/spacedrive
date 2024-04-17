use crate::{error::Error, utils::check_error};

use std::{
	ffi::{CStr, CString},
	ptr,
};

use ffmpeg_sys_next::{
	av_dict_free, av_dict_iterate, av_dict_set, AVDictionary, AVDictionaryEntry,
};

#[derive(Debug)]
pub(crate) struct FFmpegDict(*mut AVDictionary);

impl FFmpegDict {
	pub(crate) fn new(av_dict: Option<*mut AVDictionary>) -> Self {
		Self(av_dict.unwrap_or(ptr::null_mut()))
	}

	pub(crate) fn as_mut_ptr(&mut self) -> *mut AVDictionary {
		self.0
	}

	pub(crate) fn set(&mut self, key: CString, value: CString) -> Result<(), Error> {
		check_error(
			unsafe { av_dict_set(&mut self.0, key.as_ptr(), value.as_ptr(), 0) },
			"Fail to set dictionary key-value pair",
		)?;

		Ok(())
	}

	pub(crate) fn reset(&mut self, key: CString) -> Result<(), Error> {
		check_error(
			unsafe { av_dict_set(&mut self.0, key.as_ptr(), ptr::null(), 0) },
			"Fail to set dictionary key-value pair",
		)?;

		Ok(())
	}
}

impl Drop for FFmpegDict {
	fn drop(&mut self) {
		if !self.0.is_null() {
			unsafe { av_dict_free(&mut self.0) };
			self.0 = std::ptr::null_mut();
		}
	}
}

impl<'a> IntoIterator for &'a FFmpegDict {
	type Item = (String, String);
	type IntoIter = FFmpegDictIter<'a>;

	#[inline]
	fn into_iter(self) -> FFmpegDictIter<'a> {
		FFmpegDictIter {
			dict: self.0,
			prev: std::ptr::null(),
			_lifetime: std::marker::PhantomData,
		}
	}
}

pub(crate) struct FFmpegDictIter<'a> {
	dict: *mut AVDictionary,
	prev: *const AVDictionaryEntry,
	_lifetime: std::marker::PhantomData<&'a ()>,
}

impl<'a> Iterator for FFmpegDictIter<'a> {
	type Item = (String, String);

	fn next(&mut self) -> Option<(String, String)> {
		unsafe {
			self.prev = av_dict_iterate(self.dict, self.prev);
		}
		if self.prev.is_null() {
			return None;
		}

		let key = unsafe { CStr::from_ptr((*self.prev).key) };
		let value = unsafe { CStr::from_ptr((*self.prev).value) };
		return Some((
			key.to_string_lossy().into_owned(),
			value.to_string_lossy().into_owned(),
		));
	}
}
