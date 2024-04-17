use crate::{dict::FFmpegDict, error::Error, model::MediaMetadata, utils::check_error};

use ffmpeg_sys_next::{
	avformat_close_input, avformat_find_stream_info, avformat_open_input, AVFormatContext,
};

use std::{
	ffi::{CStr, CString},
	ptr,
};

#[derive(Debug)]
pub(crate) struct FFmpegFormatContext {
	data: *mut AVFormatContext,
}

impl FFmpegFormatContext {
	pub(crate) fn formats(&self) -> Option<Vec<String>> {
		if self.data.is_null() {
			return None;
		}

		let format: *const ffmpeg_sys_next::AVInputFormat = unsafe { (*self.data).iformat };
		if format.is_null() {
			return None;
		}

		let name = unsafe { (*format).name };
		if name.is_null() {
			return None;
		}

		let c_str = unsafe { CStr::from_ptr(name) };
		let string = c_str.to_string_lossy().into_owned();
		let entries: Vec<String> = string
			.split(',')
			.map(|entry| entry.trim().to_string())
			.filter(|entry| !entry.is_empty())
			.collect();

		Some(entries)
	}

	pub(crate) fn metadata(&self) -> Option<MediaMetadata> {
		if self.data.is_null() {
			return None;
		}

		let metadata_ptr = unsafe { (*self.data).metadata };
		if metadata_ptr.is_null() {
			return None;
		}

		let mut media_metadata = MediaMetadata::default();

		let metadata = FFmpegDict::new(Some(metadata_ptr));
		for (key, value) in metadata.into_iter() {
			match key.as_str() {
				"album" => media_metadata.album = Some(value.clone()),
				"album_artist" => media_metadata.album_artist = Some(value.clone()),
				"artist" => media_metadata.artist = Some(value.clone()),
				"comment" => media_metadata.comment = Some(value.clone()),
				"composer" => media_metadata.composer = Some(value.clone()),
				"copyright" => media_metadata.copyright = Some(value.clone()),
				"creation_time" => {
					if let Ok(creation_time) = chrono::DateTime::parse_from_rfc3339(&value) {
						media_metadata.creation_time = Some(creation_time.into());
					}
				}
				"date" => {
					if let Ok(date) = chrono::DateTime::parse_from_rfc3339(&value) {
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
					if let Ok(variant_bitrate) = value.parse() {
						media_metadata.variant_bitrate = Some(variant_bitrate);
					}
				}
				_ => {
					media_metadata.custom.insert(key.clone(), value.clone());
				}
			}
		}

		Some(media_metadata)
	}

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
