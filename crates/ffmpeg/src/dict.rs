use crate::{error::Error, model::FFmpegMetadata, utils::check_error};

use std::{ffi::CStr, ptr};

use chrono::DateTime;
use ffmpeg_sys_next::{
	av_dict_free, av_dict_get, av_dict_iterate, av_dict_set, AVDictionary, AVDictionaryEntry,
	AV_DICT_MATCH_CASE,
};

#[derive(Debug)]
pub struct FFmpegDictionary {
	dict: *mut AVDictionary,
	managed: bool,
}

impl FFmpegDictionary {
	pub(crate) fn new(av_dict: Option<&mut AVDictionary>) -> Self {
		av_dict.map_or_else(
			|| Self {
				dict: ptr::null_mut(),
				managed: true,
			},
			|ptr| Self {
				dict: ptr,
				managed: false,
			},
		)
	}

	pub(crate) fn get(&self, key: &CStr) -> Option<String> {
		if self.dict.is_null() {
			return None;
		}

		unsafe { av_dict_get(self.dict, key.as_ptr(), ptr::null(), AV_DICT_MATCH_CASE).as_ref() }
			.and_then(|entry| unsafe { entry.value.as_ref() })
			.map(|value| {
				let cstr = unsafe { CStr::from_ptr(value) };
				String::from_utf8_lossy(cstr.to_bytes()).to_string()
			})
	}

	pub(crate) fn remove(&mut self, key: &CStr) -> Result<(), Error> {
		check_error(
			unsafe {
				av_dict_set(
					&mut self.dict,
					key.as_ptr(),
					ptr::null(),
					AV_DICT_MATCH_CASE,
				)
			},
			"Fail to set dictionary key-value pair",
		)?;

		Ok(())
	}
}

impl Drop for FFmpegDictionary {
	fn drop(&mut self) {
		if self.managed && !self.dict.is_null() {
			unsafe { av_dict_free(&mut self.dict) };
			self.dict = ptr::null_mut();
		}
	}
}

impl<'a> IntoIterator for &'a FFmpegDictionary {
	type Item = (String, Option<String>);
	type IntoIter = FFmpegDictIter<'a>;

	#[inline]
	fn into_iter(self) -> FFmpegDictIter<'a> {
		FFmpegDictIter {
			dict: self.dict,
			prev: ptr::null(),
			_lifetime: std::marker::PhantomData,
		}
	}
}

pub struct FFmpegDictIter<'a> {
	dict: *mut AVDictionary,
	prev: *const AVDictionaryEntry,
	_lifetime: std::marker::PhantomData<&'a ()>,
}

impl Iterator for FFmpegDictIter<'_> {
	type Item = (String, Option<String>);

	fn next(&mut self) -> Option<(String, Option<String>)> {
		unsafe { av_dict_iterate(self.dict, self.prev).as_ref() }.and_then(|prev| {
			self.prev = prev;
			let key = unsafe { prev.key.as_ref() }.map(|key| unsafe { CStr::from_ptr(key) });
			let value =
				unsafe { prev.value.as_ref() }.map(|value| unsafe { CStr::from_ptr(value) });

			match (key, value) {
				(None, _) => None,
				(Some(key), None) => {
					Some((String::from_utf8_lossy(key.to_bytes()).to_string(), None))
				}
				(Some(key), Some(value)) => Some((
					String::from_utf8_lossy(key.to_bytes()).to_string(),
					Some(String::from_utf8_lossy(value.to_bytes()).to_string()),
				)),
			}
		})
	}
}

impl From<&FFmpegDictionary> for FFmpegMetadata {
	fn from(dict: &FFmpegDictionary) -> Self {
		let mut media_metadata = Self::default();

		for (key, value) in dict {
			if let Some(value) = value {
				match key.as_str() {
					"album" => media_metadata.album = Some(value.clone()),
					"album_artist" => media_metadata.album_artist = Some(value.clone()),
					"artist" => media_metadata.artist = Some(value.clone()),
					"comment" => media_metadata.comment = Some(value.clone()),
					"composer" => media_metadata.composer = Some(value.clone()),
					"copyright" => media_metadata.copyright = Some(value.clone()),
					"creation_time" => {
						if let Ok(creation_time) = DateTime::parse_from_rfc2822(&value) {
							media_metadata.creation_time = Some(creation_time.into());
						} else if let Ok(creation_time) = DateTime::parse_from_rfc3339(&value) {
							media_metadata.creation_time = Some(creation_time.into());
						}
					}
					"date" => {
						if let Ok(date) = DateTime::parse_from_rfc2822(&value) {
							media_metadata.date = Some(date.into());
						} else if let Ok(date) = DateTime::parse_from_rfc3339(&value) {
							media_metadata.date = Some(date.into());
						}
					}
					"disc" => {
						if let Ok(disc) = value.parse() {
							media_metadata.disc = Some(disc);
						}
					}
					"encoder" => media_metadata.encoder = Some(value.clone()),
					"encoded_by" => media_metadata.encoded_by = Some(value.clone()),
					"filename" => media_metadata.filename = Some(value.clone()),
					"genre" => media_metadata.genre = Some(value.clone()),
					"language" => media_metadata.language = Some(value.clone()),
					"performer" => media_metadata.performer = Some(value.clone()),
					"publisher" => media_metadata.publisher = Some(value.clone()),
					"service_name" => media_metadata.service_name = Some(value.clone()),
					"service_provider" => media_metadata.service_provider = Some(value.clone()),
					"title" => media_metadata.title = Some(value.clone()),
					"track" => {
						if let Ok(track) = value.parse() {
							media_metadata.track = Some(track);
						}
					}
					"variant_bitrate" => {
						if let Ok(variant_bit_rate) = value.parse() {
							media_metadata.variant_bit_rate = Some(variant_bit_rate);
						}
					}
					_ => {
						media_metadata.custom.insert(key.clone(), value.clone());
					}
				}
			}
		}

		media_metadata
	}
}

impl From<FFmpegDictionary> for FFmpegMetadata {
	fn from(dict: FFmpegDictionary) -> Self {
		(&dict).into()
	}
}
