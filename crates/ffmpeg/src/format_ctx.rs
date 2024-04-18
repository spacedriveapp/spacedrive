use crate::{
	dict::FFmpegDict,
	error::Error,
	model::{MediaChapter, MediaMetadata, MediaProgram, MediaStream},
	utils::check_error,
};

use ffmpeg_sys_next::{
	av_cmp_q, av_q2d, av_reduce, avformat_close_input, avformat_find_stream_info,
	avformat_open_input, AVFormatContext, AVRational, AV_DISPOSITION_ATTACHED_PIC,
	AV_DISPOSITION_CAPTIONS, AV_DISPOSITION_CLEAN_EFFECTS, AV_DISPOSITION_COMMENT,
	AV_DISPOSITION_DEFAULT, AV_DISPOSITION_DEPENDENT, AV_DISPOSITION_DESCRIPTIONS,
	AV_DISPOSITION_DUB, AV_DISPOSITION_FORCED, AV_DISPOSITION_HEARING_IMPAIRED,
	AV_DISPOSITION_KARAOKE, AV_DISPOSITION_LYRICS, AV_DISPOSITION_METADATA,
	AV_DISPOSITION_NON_DIEGETIC, AV_DISPOSITION_ORIGINAL, AV_DISPOSITION_STILL_IMAGE,
	AV_DISPOSITION_TIMED_THUMBNAILS, AV_DISPOSITION_VISUAL_IMPAIRED, AV_NOPTS_VALUE, AV_TIME_BASE,
};

use std::{
	collections::HashSet,
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

	pub(crate) unsafe fn as_mut<'a>(&mut self) -> Option<&'a mut AVFormatContext> {
		self.data.as_mut()
	}

	pub(crate) unsafe fn as_ref<'a>(&self) -> Option<&'a AVFormatContext> {
		self.data.as_ref()
	}

	pub(crate) fn find_stream_info(&self) -> Result<(), Error> {
		check_error(
			unsafe { avformat_find_stream_info(self.data, ptr::null_mut()) },
			"Fail to read packets of a media file to get stream information",
		)?;

		Ok(())
	}

	pub fn formats(&self) -> Vec<String> {
		unsafe { self.as_ref() }
			.and_then(|ctx| {
				unsafe { ctx.iformat.as_ref() }.and_then(|format| {
					let name = format.name;
					if name.is_null() {
						None
					} else {
						Some(
							String::from_utf8_lossy(unsafe { CStr::from_ptr(name) }.to_bytes())
								.split(',')
								.map(|entry| entry.trim().to_string())
								.filter(|entry| !entry.is_empty())
								.collect(),
						)
					}
				})
			})
			.unwrap_or(vec![])
	}

	pub fn duration(&self) -> Option<TimeDelta> {
		unsafe { self.as_ref() }.and_then(|ctx| {
			let duration = ctx.duration;
			if duration == AV_NOPTS_VALUE {
				return None;
			}

			let ms = (duration % (AV_TIME_BASE as i64)).abs();
			TimeDelta::new(duration / (AV_TIME_BASE as i64), (ms * 1000) as u32)
		})
	}

	pub fn start_time(&self) -> Option<TimeDelta> {
		unsafe { self.as_ref() }.and_then(|ctx| {
			let start_time = ctx.start_time;
			if start_time == AV_NOPTS_VALUE {
				return None;
			}

			let _secs = start_time / (AV_TIME_BASE as i64);
			let ms = (start_time % (AV_TIME_BASE as i64)).abs();

			TimeDelta::new(start_time / (AV_TIME_BASE as i64), (ms * 1000) as u32)
		})
	}

	pub fn bit_rate(&self) -> Option<i64> {
		unsafe { self.as_ref() }.map(|ctx| ctx.bit_rate)
	}

	pub fn chapters(&self) -> Vec<MediaChapter> {
		unsafe { self.as_ref() }
			.and_then(
				|ctx| match (ctx.nb_chapters, unsafe { ctx.chapters.as_ref() }) {
					(0, _) => None,
					(a, Some(b)) => Some((a, b)),
					_ => None,
				},
			)
			.map(|(nb_chapters, chapters)| {
				(0..nb_chapters)
					.filter_map(|id| {
						unsafe { chapters.offset(id as isize).as_ref() }.map(|chapter| {
							MediaChapter {
								id,
								start: chapter.start as f64 * unsafe { av_q2d(chapter.time_base) },
								end: chapter.end as f64 * unsafe { av_q2d(chapter.time_base) },
								metadata: FFmpegDict::new(Some(chapter.metadata)).into(),
							}
						})
					})
					.collect::<Vec<MediaChapter>>()
			})
			.unwrap_or(vec![])
	}

	pub fn stream(&self, id: u32) -> Option<MediaStream> {
		unsafe { self.as_ref() }
			.and_then(|ctx| unsafe { ctx.streams.as_ref() })
			.and_then(|streams| unsafe { streams.offset(id as isize).as_ref() })
			.map(|stream| {
				let mut metadata = FFmpegDict::new(Some(stream.metadata));
				let name = CString::new("name").map_or(None, |key| {
					let name = metadata.get(key.to_owned());
					if name.is_some() {
						let _ = metadata.remove(key);
					}
					name
				});

				let aspect_ratio = unsafe { stream.codecpar.as_ref() }
					.and_then(|codecpar| {
						if stream.sample_aspect_ratio.num != 0
							&& unsafe {
								av_cmp_q(stream.sample_aspect_ratio, codecpar.sample_aspect_ratio)
							} != 0
						{
							let mut display_aspect_ratio = AVRational { num: 0, den: 0 };
							let num = (codecpar.width * codecpar.sample_aspect_ratio.num) as i64;
							let den = (codecpar.height * codecpar.sample_aspect_ratio.den) as i64;
							let max = 1024 * 1024;
							unsafe {
								av_reduce(
									&mut display_aspect_ratio.num,
									&mut display_aspect_ratio.den,
									num,
									den,
									max,
								);
							}

							Some(display_aspect_ratio)
						} else {
							None
						}
					})
					.unwrap_or(stream.sample_aspect_ratio);

				let dispositions = [
					(AV_DISPOSITION_DEFAULT, "default"),
					(AV_DISPOSITION_DUB, "dub"),
					(AV_DISPOSITION_ORIGINAL, "original"),
					(AV_DISPOSITION_COMMENT, "comment"),
					(AV_DISPOSITION_LYRICS, "lyrics"),
					(AV_DISPOSITION_KARAOKE, "karaoke"),
					(AV_DISPOSITION_FORCED, "forced"),
					(AV_DISPOSITION_HEARING_IMPAIRED, "hearing impaired"),
					(AV_DISPOSITION_VISUAL_IMPAIRED, "visual impaired"),
					(AV_DISPOSITION_CLEAN_EFFECTS, "clean effects"),
					(AV_DISPOSITION_ATTACHED_PIC, "attached pic"),
					(AV_DISPOSITION_TIMED_THUMBNAILS, "timed thumbnails"),
					(AV_DISPOSITION_CAPTIONS, "captions"),
					(AV_DISPOSITION_DESCRIPTIONS, "descriptions"),
					(AV_DISPOSITION_METADATA, "metadata"),
					(AV_DISPOSITION_DEPENDENT, "dependent"),
					(AV_DISPOSITION_STILL_IMAGE, "still image"),
					(AV_DISPOSITION_NON_DIEGETIC, "non-diegetic"),
				]
				.iter()
				.filter_map(|&(flag, name)| {
					if stream.disposition & flag != 0 {
						Some(name.to_string())
					} else {
						None
					}
				})
				.collect::<Vec<String>>();

				MediaStream {
					id: stream.id as u32,
					name,
					codec: None,
					aspect_ratio_num: aspect_ratio.num,
					aspect_ratio_den: aspect_ratio.den,
					frames_per_second_num: stream.avg_frame_rate.num,
					frames_per_second_den: stream.avg_frame_rate.den,
					time_base_real_num: stream.time_base.num,
					time_base_real_den: stream.time_base.den,
					dispositions,
					metadata: metadata.into(),
				}
			})
	}

	pub fn programs(&self) -> Vec<MediaProgram> {
		unsafe { self.as_ref() }
			.map(|ctx| {
				let mut visited_streams: HashSet<u32> = HashSet::new();
				let mut programs = unsafe { ctx.programs.as_ref() }
					.map(|programs| {
						(0..ctx.nb_programs)
							.filter_map(|id| {
								unsafe { programs.offset(id as isize).as_ref() }.map(|program| {
									let mut metadata = FFmpegDict::new(Some(program.metadata));
									let name = CString::new("name").map_or(None, |key| {
										let name = metadata.get(key.to_owned());
										if name.is_some() {
											let _ = metadata.remove(key);
										}
										name
									});

									let streams = (0..program.nb_stream_indexes)
										.filter_map(|index| {
											unsafe {
												program.stream_index.offset(index as isize).as_ref()
											}
											.and_then(|stream_index| {
												self.stream(*stream_index).map(|stream| {
													visited_streams.insert(*stream_index);
													stream
												})
											})
										})
										.collect::<Vec<MediaStream>>();

									MediaProgram {
										id,
										name,
										streams,
										metadata: metadata.into(),
									}
								})
							})
							.collect::<Vec<MediaProgram>>()
					})
					.unwrap_or(vec![]);

				let unvisited_streams = (0..ctx.nb_streams)
					.filter(|i| !visited_streams.contains(i))
					.filter_map(|i| self.stream(i))
					.collect::<Vec<MediaStream>>();
				if !unvisited_streams.is_empty() {
					// Create an empty program to hold unvisited streams if there are any
					programs.push(MediaProgram {
						id: programs.len() as u32,
						name: Some("No Program".to_string()),
						streams: unvisited_streams,
						metadata: MediaMetadata::default(),
					});
				}

				programs
			})
			.unwrap_or(vec![])
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
