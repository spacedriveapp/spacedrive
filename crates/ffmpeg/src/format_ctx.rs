use crate::{
	dict::FFmpegDict,
	error::Error,
	model::{MediaChapter, MediaMetadata, MediaProgram},
	utils::check_error,
};

use ffmpeg_sys_next::{
	av_q2d, avformat_close_input, avformat_find_stream_info, avformat_open_input, AVFormatContext,
	AV_NOPTS_VALUE, AV_TIME_BASE,
};

use std::{
	ffi::{CStr, CString},
	ptr,
};

use chrono::TimeDelta;

#[derive(Debug)]
pub(crate) struct FFmpegFormatContext {
	data: *mut AVFormatContext,
}

impl FFmpegFormatContext {
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

	pub(crate) fn as_mut_ptr(&mut self) -> *mut AVFormatContext {
		self.data
	}

	pub(crate) fn find_stream_info(&self) -> Result<(), Error> {
		check_error(
			unsafe { avformat_find_stream_info(self.data, ptr::null_mut()) },
			"Fail to read packets of a media file to get stream information",
		)?;

		Ok(())
	}

	pub fn formats(&self) -> Vec<String> {
		if self.data.is_null() {
			return vec![];
		}

		let format: *const ffmpeg_sys_next::AVInputFormat = unsafe { (*self.data).iformat };
		if format.is_null() {
			return vec![];
		}

		let name = unsafe { (*format).name };
		if name.is_null() {
			return vec![];
		}

		let cstr = unsafe { CStr::from_ptr(name) };
		let _string = cstr.to_string_lossy().into_owned();
		let entries: Vec<String> = String::from_utf8_lossy(cstr.to_bytes())
			.split(',')
			.map(|entry| entry.trim().to_string())
			.filter(|entry| !entry.is_empty())
			.collect();

		entries
	}

	pub fn duration(&self) -> Option<TimeDelta> {
		if self.data.is_null() {
			return None;
		}

		let duration = unsafe { *self.data }.duration;
		if duration == AV_NOPTS_VALUE {
			return None;
		}

		let ms = (duration % (AV_TIME_BASE as i64)).abs();
		TimeDelta::new(duration / (AV_TIME_BASE as i64), (ms * 1000) as u32)
	}

	pub fn start_time(&self) -> Option<TimeDelta> {
		if self.data.is_null() {
			return None;
		}

		let start_time = unsafe { *self.data }.start_time;
		if start_time == AV_NOPTS_VALUE {
			return None;
		}

		let _secs = start_time / (AV_TIME_BASE as i64);
		let ms = (start_time % (AV_TIME_BASE as i64)).abs();

		TimeDelta::new(start_time / (AV_TIME_BASE as i64), (ms * 1000) as u32)
	}

	pub fn bit_rate(&self) -> Option<i64> {
		if self.data.is_null() {
			return None;
		}

		Some(unsafe { *self.data }.bit_rate)
	}

	pub fn chapters(&self) -> Vec<MediaChapter> {
		if self.data.is_null() {
			return vec![];
		}

		let nb_chapters = unsafe { *self.data }.nb_chapters;
		if nb_chapters == 0 {
			return vec![];
		}

		let mut chapters = Vec::new();
		for id in 0..nb_chapters {
			let chapter = unsafe { **(*self.data).chapters.offset(id as isize) };
			chapters.push(MediaChapter {
				id,
				start: chapter.start as f64 * unsafe { av_q2d(chapter.time_base) },
				end: chapter.end as f64 * unsafe { av_q2d(chapter.time_base) },
				metadata: FFmpegDict::new(Some(chapter.metadata)).into(),
			});
		}

		chapters
	}

	pub fn programs(&self) -> Vec<MediaProgram> {
		let mut programs = Vec::new();

		if !self.data.is_null() && unsafe { (*self.data).nb_programs } > 0 {
			for id in 0..unsafe { (*self.data).nb_programs } {
				let program = unsafe { **(*self.data).programs.offset(id as isize) };
				let mut metadata = FFmpegDict::new(Some(program.metadata));
				let name = CString::new("name").map_or(None, |key| {
					let name = metadata.get(key.to_owned());
					if name.is_some() {
						let _ = metadata.remove(key);
					}
					name
				});

				programs.push(MediaProgram {
					id,
					name,
					streams: Vec::new(),
					metadata: metadata.into(),
				})
			}
		}

		programs
	}

	pub fn metadata(&self) -> Option<MediaMetadata> {
		if self.data.is_null() {
			return None;
		}

		let metadata_ptr = unsafe { *self.data }.metadata;
		if metadata_ptr.is_null() {
			return None;
		}

		Some(FFmpegDict::new(Some(metadata_ptr)).into())
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
