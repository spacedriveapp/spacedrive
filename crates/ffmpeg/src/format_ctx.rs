use crate::{
	codec_ctx::FFmpegCodecContext,
	dict::FFmpegDict,
	error::{Error, FFmpegError},
	model::{MediaChapter, MediaInfo, MediaMetadata, MediaProgram, MediaStream},
	utils::check_error,
};

use ffmpeg_sys_next::{
	av_cmp_q, av_display_rotation_get, av_read_frame, av_reduce, av_stream_get_side_data,
	avformat_close_input, avformat_find_stream_info, avformat_open_input, AVCodecID, AVDictionary,
	AVFormatContext, AVMediaType, AVPacket, AVPacketSideDataType, AVRational, AVStream,
	AV_DISPOSITION_ATTACHED_PIC, AV_DISPOSITION_CAPTIONS, AV_DISPOSITION_CLEAN_EFFECTS,
	AV_DISPOSITION_COMMENT, AV_DISPOSITION_DEFAULT, AV_DISPOSITION_DEPENDENT,
	AV_DISPOSITION_DESCRIPTIONS, AV_DISPOSITION_DUB, AV_DISPOSITION_FORCED,
	AV_DISPOSITION_HEARING_IMPAIRED, AV_DISPOSITION_KARAOKE, AV_DISPOSITION_LYRICS,
	AV_DISPOSITION_METADATA, AV_DISPOSITION_NON_DIEGETIC, AV_DISPOSITION_ORIGINAL,
	AV_DISPOSITION_STILL_IMAGE, AV_DISPOSITION_TIMED_THUMBNAILS, AV_DISPOSITION_VISUAL_IMPAIRED,
	AV_NOPTS_VALUE, AV_TIME_BASE,
};

use std::{collections::HashSet, ffi::CStr, ptr};

use chrono::TimeDelta;

fn extract_name_and_convert_metadata(
	metadata: *mut AVDictionary,
) -> (MediaMetadata, Option<String>) {
	let mut metadata = FFmpegDict::new(unsafe { metadata.as_mut() });
	let name = metadata.get(c"name");
	if name.is_some() {
		let _ = metadata.remove(c"name");
	}

	(metadata.into(), name)
}

#[derive(Debug)]
pub(crate) struct FFmpegFormatContext(*mut AVFormatContext);

impl FFmpegFormatContext {
	pub(crate) fn open_file(filename: &CStr) -> Result<Self, Error> {
		let mut ptr = ptr::null_mut();

		check_error(
			unsafe {
				avformat_open_input(&mut ptr, filename.as_ptr(), ptr::null(), ptr::null_mut())
			},
			"Fail to open an input stream and read the header",
		)
		.map(|()| Self(ptr))
	}

	pub(crate) fn as_ref(&self) -> &AVFormatContext {
		unsafe { self.0.as_ref() }.expect("initialized on struct creation")
	}

	pub(crate) fn as_mut(&mut self) -> &mut AVFormatContext {
		unsafe { self.0.as_mut() }.expect("initialized on struct creation")
	}

	pub(crate) fn duration(&self) -> Option<TimeDelta> {
		let duration = self.as_ref().duration;
		if duration == AV_NOPTS_VALUE {
			return None;
		}

		let ms = (duration % (AV_TIME_BASE as i64)).abs();
		TimeDelta::new(duration / (AV_TIME_BASE as i64), (ms * 1000) as u32)
	}

	pub(crate) fn stream(&self, index: u32) -> Option<&mut AVStream> {
		let Ok(index) = isize::try_from(index) else {
			return None;
		};

		unsafe { self.as_ref().streams.as_ref() }
			.and_then(|streams| unsafe { streams.offset(index).as_mut() })
	}

	pub(crate) fn get_stream_rotation_angle(&self, index: u32) -> f64 {
		let Some(stream) = self.stream(index) else {
			return 0.0;
		};

		#[allow(clippy::cast_ptr_alignment)]
		let matrix = (unsafe {
			av_stream_get_side_data(
				stream,
				AVPacketSideDataType::AV_PKT_DATA_DISPLAYMATRIX,
				std::ptr::null_mut(),
			)
		} as *const i32);

		if matrix.is_null() {
			0.0
		} else {
			unsafe { av_display_rotation_get(matrix) }
		}
	}

	pub(crate) fn read_frame(&mut self, packet: *mut AVPacket) -> Result<(), Error> {
		check_error(
			unsafe { av_read_frame(self.as_mut(), packet) },
			"Fail to read the next frame of a media file",
		)?;

		Ok(())
	}

	pub(crate) fn find_stream_info(&mut self) -> Result<(), Error> {
		check_error(
			unsafe { avformat_find_stream_info(self.as_mut(), ptr::null_mut()) },
			"Fail to read packets of a media file to get stream information",
		)?;

		Ok(())
	}

	pub(crate) fn find_preferred_video_stream(
		&self,
		prefer_embedded_metadata: bool,
	) -> Result<(bool, &mut AVStream), Error> {
		let mut video_streams = vec![];
		let mut embedded_data_streams = vec![];

		'outer: for stream_idx in 0..self.as_ref().nb_streams {
			let Some(stream) = unsafe { self.as_ref().streams.as_ref() }
				.and_then(|streams| unsafe { streams.offset(stream_idx as isize).as_ref() })
			else {
				continue;
			};

			let Some((codec_type, codec_id)) = unsafe { stream.codecpar.as_ref() }
				.map(|codec_params| (codec_params.codec_type, codec_params.codec_id))
			else {
				continue;
			};

			if codec_type != AVMediaType::AVMEDIA_TYPE_VIDEO {
				continue;
			}

			if !prefer_embedded_metadata
				|| !(codec_id == AVCodecID::AV_CODEC_ID_MJPEG
					|| codec_id == AVCodecID::AV_CODEC_ID_PNG)
			{
				video_streams.push(stream_idx);
				continue;
			}

			if let Some(metadata) =
				unsafe { stream.metadata.as_mut() }.map(|metadata| FFmpegDict::new(Some(metadata)))
			{
				for (key, value) in metadata.into_iter() {
					if let Some(value) = value {
						if key == "filename" && value.starts_with("cover.") {
							embedded_data_streams.insert(0, stream_idx);
							continue 'outer;
						}
					}
				}
			}

			embedded_data_streams.push(stream_idx);
		}

		if prefer_embedded_metadata && !embedded_data_streams.is_empty() {
			for stream_index in embedded_data_streams {
				if let Some(stream) = self.stream(stream_index) {
					return Ok((true, stream));
				}
			}
		}

		for stream_index in video_streams {
			if let Some(stream) = self.stream(stream_index) {
				return Ok((false, stream));
			}
		}

		Err(FFmpegError::StreamNotFound)?
	}

	fn formats(&self) -> Vec<String> {
		unsafe { self.as_ref().iformat.as_ref() }
			.and_then(|format| unsafe { format.name.as_ref() })
			.map(|name| {
				let cstr = unsafe { CStr::from_ptr(name) };
				String::from_utf8_lossy(cstr.to_bytes())
					.split(',')
					.map(|entry| entry.trim().to_string())
					.filter(|entry| !entry.is_empty())
					.collect()
			})
			.unwrap_or(vec![])
	}

	fn start_time(&self) -> Option<TimeDelta> {
		let start_time = self.as_ref().start_time;
		if start_time == AV_NOPTS_VALUE {
			return None;
		}

		let _secs = start_time / (AV_TIME_BASE as i64);
		let ms = (start_time % (AV_TIME_BASE as i64)).abs();

		TimeDelta::new(start_time / (AV_TIME_BASE as i64), (ms * 1000) as u32)
	}

	fn bit_rate(&self) -> i64 {
		self.as_ref().bit_rate
	}

	fn chapters(&self) -> Vec<MediaChapter> {
		unsafe { self.as_ref().chapters.as_ref() }
			.map(|chapters| {
				(0..self.as_ref().nb_chapters)
					.filter_map(|id| {
						unsafe { chapters.offset(id as isize).as_ref() }.map(|chapter| {
							MediaChapter {
								id,
								start: chapter.start,
								end: chapter.end,
								time_base_num: chapter.time_base.num,
								time_base_den: chapter.time_base.den,
								metadata: unsafe { chapter.metadata.as_mut() }
									.map(|metadata| FFmpegDict::new(Some(metadata)).into())
									.unwrap_or_else(MediaMetadata::default),
							}
						})
					})
					.collect::<Vec<MediaChapter>>()
			})
			.unwrap_or(vec![])
	}

	fn programs(&self) -> Vec<MediaProgram> {
		let mut visited_streams: HashSet<u32> = HashSet::new();
		let mut programs = unsafe { self.as_ref().programs.as_ref() }
			.map(|programs| {
				(0..self.as_ref().nb_programs)
					.filter_map(|id| {
						unsafe { programs.offset(id as isize).as_ref() }.map(|program| {
							let (metadata, name) =
								extract_name_and_convert_metadata(program.metadata);

							let streams = (0..program.nb_stream_indexes)
								.filter_map(|index| {
									unsafe { program.stream_index.offset(index as isize).as_ref() }
										.and_then(|stream_index| {
											self.stream(*stream_index).map(|stream| {
												visited_streams.insert(*stream_index);
												(&*stream).into()
											})
										})
								})
								.collect::<Vec<MediaStream>>();

							MediaProgram {
								id,
								name,
								streams,
								metadata,
							}
						})
					})
					.collect::<Vec<MediaProgram>>()
			})
			.unwrap_or(vec![]);

		let unvisited_streams = (0..self.as_ref().nb_streams)
			.filter(|i| !visited_streams.contains(i))
			.filter_map(|i| self.stream(i).map(|stream| (&*stream).into()))
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
	}

	fn metadata(&self) -> Option<MediaMetadata> {
		unsafe { self.as_ref().metadata.as_mut() }
			.map(|metadata| FFmpegDict::new(Some(metadata)).into())
	}
}

impl Drop for FFmpegFormatContext {
	fn drop(&mut self) {
		if !self.0.is_null() {
			unsafe { avformat_close_input(&mut self.0) };
			self.0 = std::ptr::null_mut();
		}
	}
}

impl From<&FFmpegFormatContext> for MediaInfo {
	fn from(ctx: &FFmpegFormatContext) -> Self {
		MediaInfo {
			formats: ctx.formats(),
			duration: ctx.duration(),
			start_time: ctx.start_time(),
			bitrate: ctx.bit_rate(),
			chapters: ctx.chapters(),
			programs: ctx.programs(),
			metadata: ctx.metadata(),
		}
	}
}

impl From<&AVStream> for MediaStream {
	fn from(stream: &AVStream) -> Self {
		let (metadata, name) = extract_name_and_convert_metadata(stream.metadata);

		let aspect_ratio = unsafe { stream.codecpar.as_ref() }
			.and_then(|codecpar| {
				if stream.sample_aspect_ratio.num != 0
					&& unsafe { av_cmp_q(stream.sample_aspect_ratio, codecpar.sample_aspect_ratio) }
						!= 0
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

		let codec = unsafe { stream.codecpar.as_ref() }.and_then(|codec_params| {
			FFmpegCodecContext::new()
				.and_then(|mut codec| {
					codec.parameters_to_context(codec_params)?;
					Ok(codec)
				})
				.map(|codec| (&codec).into())
				.ok()
		});

		MediaStream {
			id: stream.id as u32,
			name,
			codec,
			aspect_ratio_num: aspect_ratio.num,
			aspect_ratio_den: aspect_ratio.den,
			frames_per_second_num: stream.avg_frame_rate.num,
			frames_per_second_den: stream.avg_frame_rate.den,
			time_base_real_num: stream.time_base.num,
			time_base_real_den: stream.time_base.den,
			dispositions,
			metadata,
		}
	}
}
