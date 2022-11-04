use crate::{
	error::{FfmpegError, ThumbnailerError},
	utils::from_path,
	video_frame::{FfmpegFrame, FrameSource, VideoFrame},
};

use ffmpeg_sys_next::{
	av_buffersink_get_frame, av_buffersrc_write_frame, av_dict_get, av_display_rotation_get,
	av_frame_alloc, av_frame_free, av_guess_sample_aspect_ratio, av_packet_alloc, av_packet_free,
	av_packet_unref, av_read_frame, av_seek_frame, av_stream_get_side_data, avcodec_alloc_context3,
	avcodec_find_decoder, avcodec_flush_buffers, avcodec_free_context, avcodec_open2,
	avcodec_parameters_to_context, avcodec_receive_frame, avcodec_send_packet,
	avfilter_get_by_name, avfilter_graph_alloc, avfilter_graph_config,
	avfilter_graph_create_filter, avfilter_graph_free, avfilter_link, avformat_close_input,
	avformat_find_stream_info, avformat_open_input, AVCodec, AVCodecContext, AVCodecID,
	AVFilterContext, AVFilterGraph, AVFormatContext, AVFrame, AVMediaType, AVPacket,
	AVPacketSideDataType, AVRational, AVStream, AVERROR, AVERROR_EOF, AV_DICT_IGNORE_SUFFIX,
	AV_TIME_BASE, EAGAIN,
};
use std::{
	ffi::{c_int, CString},
	fmt::Write,
	path::Path,
	time::Duration,
};

const AVERROR_EAGAIN: c_int = AVERROR(EAGAIN);

#[derive(Debug, Clone, Copy)]
pub(crate) enum ThumbnailSize {
	Dimensions { width: u32, height: u32 },
	Size(u32),
}

pub(crate) struct MovieDecoder {
	video_stream_index: i32,
	format_context: *mut AVFormatContext,
	video_codec_context: *mut AVCodecContext,
	video_codec: *const AVCodec,
	filter_graph: *mut AVFilterGraph,
	filter_source: *mut AVFilterContext,
	filter_sink: *mut AVFilterContext,
	video_stream: *mut AVStream,
	frame: *mut AVFrame,
	packet: *mut AVPacket,
	allow_seek: bool,
	use_embedded_data: bool,
}

impl MovieDecoder {
	pub(crate) fn new(
		filename: impl AsRef<Path>,
		prefer_embedded_metadata: bool,
	) -> Result<Self, ThumbnailerError> {
		let filename = filename.as_ref();

		let input_file = if filename == Path::new("-") {
			Path::new("pipe:")
		} else {
			filename
		};
		let allow_seek = filename != Path::new("-")
			&& !filename.starts_with("rsts://")
			&& !filename.starts_with("udp://");

		let mut decoder = Self {
			video_stream_index: -1,
			format_context: std::ptr::null_mut(),
			video_codec_context: std::ptr::null_mut(),
			video_codec: std::ptr::null_mut(),
			filter_graph: std::ptr::null_mut(),
			filter_source: std::ptr::null_mut(),
			filter_sink: std::ptr::null_mut(),
			video_stream: std::ptr::null_mut(),
			frame: std::ptr::null_mut(),
			packet: std::ptr::null_mut(),
			allow_seek,
			use_embedded_data: false,
		};

		unsafe {
			let input_file_cstring = from_path(input_file)?;
			match avformat_open_input(
				&mut decoder.format_context,
				input_file_cstring.as_ptr(),
				std::ptr::null_mut(),
				std::ptr::null_mut(),
			) {
				0 => {
					check_error(
						avformat_find_stream_info(decoder.format_context, std::ptr::null_mut()),
						"Failed to get stream info",
					)?;
				}
				e => {
					return Err(ThumbnailerError::FfmpegWithReason(
						FfmpegError::from(e),
						"Failed to open input".to_string(),
					))
				}
			}
		}

		decoder.initialize_video(prefer_embedded_metadata)?;

		decoder.frame = unsafe { av_frame_alloc() };
		if decoder.frame.is_null() {
			return Err(FfmpegError::FrameAllocation.into());
		}

		Ok(decoder)
	}

	pub(crate) fn decode_video_frame(&mut self) -> Result<(), ThumbnailerError> {
		let mut frame_finished = false;

		while !frame_finished && self.get_video_packet() {
			frame_finished = self.decode_video_packet()?;
		}

		if !frame_finished {
			return Err(ThumbnailerError::FrameDecodeError);
		}

		Ok(())
	}

	pub(crate) fn embedded_metadata_is_available(&self) -> bool {
		self.use_embedded_data
	}

	pub(crate) fn seek(&mut self, seconds: i64) -> Result<(), ThumbnailerError> {
		if !self.allow_seek {
			return Err(ThumbnailerError::SeekNotAllowed);
		}

		let timestamp = (AV_TIME_BASE as i64)
			.checked_mul(seconds as i64)
			.unwrap_or(0);

		check_error(
			unsafe { av_seek_frame(self.format_context, -1, timestamp, 0) },
			"Seeking video failed",
		)?;
		unsafe { avcodec_flush_buffers(self.video_codec_context) };

		let mut key_frame_attempts = 0;
		let mut got_frame;

		loop {
			let mut count = 0;
			got_frame = false;

			while !got_frame && count < 20 {
				self.get_video_packet();
				got_frame = self.decode_video_packet().unwrap_or(false);
				count += 1;
			}

			key_frame_attempts += 1;

			if !((!got_frame || unsafe { (*self.frame).key_frame } == 0)
				&& key_frame_attempts < 200)
			{
				break;
			}
		}

		if !got_frame {
			return Err(ThumbnailerError::SeekError);
		}

		Ok(())
	}

	pub(crate) fn get_scaled_video_frame(
		&mut self,
		scaled_size: Option<ThumbnailSize>,
		maintain_aspect_ratio: bool,
		video_frame: &mut VideoFrame,
	) -> Result<(), ThumbnailerError> {
		self.initialize_filter_graph(
			unsafe {
				&(*(*(*self.format_context)
					.streams
					.offset(self.video_stream_index as isize)))
				.time_base
			},
			scaled_size,
			maintain_aspect_ratio,
		)?;

		check_error(
			unsafe { av_buffersrc_write_frame(self.filter_source, self.frame) },
			"Failed to write frame to filter graph",
		)?;

		let mut new_frame = FfmpegFrame::new()?;
		let mut attempts = 0;
		let mut ret = unsafe { av_buffersink_get_frame(self.filter_sink, new_frame.as_mut_ptr()) };
		while ret == AVERROR_EAGAIN && attempts < 10 {
			self.decode_video_frame()?;
			check_error(
				unsafe { av_buffersrc_write_frame(self.filter_source, self.frame) },
				"Failed to write frame to filter graph",
			)?;
			ret = unsafe { av_buffersink_get_frame(self.filter_sink, new_frame.as_mut_ptr()) };
			attempts += 1;
		}
		if ret < 0 {
			return Err(ThumbnailerError::FfmpegWithReason(
				FfmpegError::from(ret),
				"Failed to get buffer from filter".to_string(),
			));
		}

		video_frame.width = unsafe { (*new_frame.as_mut_ptr()).width as u32 };
		video_frame.height = unsafe { (*new_frame.as_mut_ptr()).height as u32 };
		video_frame.line_size = unsafe { (*new_frame.as_mut_ptr()).linesize[0] as u32 };
		video_frame.source = if self.use_embedded_data {
			Some(FrameSource::Metadata)
		} else {
			Some(FrameSource::VideoStream)
		};

		let frame_data_size = video_frame.line_size as usize * video_frame.height as usize;
		match video_frame.data.capacity() {
			0 => {
				video_frame.data = Vec::with_capacity(frame_data_size);
			}
			c if c < frame_data_size => {
				video_frame.data.reserve_exact(frame_data_size - c);
				video_frame.data.clear();
			}
			c if c > frame_data_size => {
				video_frame.data.shrink_to(frame_data_size);
				video_frame.data.clear();
			}
			_ => {
				video_frame.data.clear();
			}
		}

		video_frame.data.extend_from_slice(unsafe {
			std::slice::from_raw_parts((*new_frame.as_mut_ptr()).data[0], frame_data_size)
		});

		unsafe { avfilter_graph_free(&mut self.filter_graph) };

		Ok(())
	}

	pub(crate) fn get_video_duration(&self) -> Duration {
		Duration::from_secs(unsafe { (*self.format_context).duration as u64 / AV_TIME_BASE as u64 })
	}

	fn initialize_video(&mut self, prefer_embedded_metadata: bool) -> Result<(), ThumbnailerError> {
		self.find_preferred_video_stream(prefer_embedded_metadata)?;

		self.video_stream = unsafe {
			*(*self.format_context)
				.streams
				.offset(self.video_stream_index as isize)
		};
		self.video_codec =
			unsafe { avcodec_find_decoder((*(*self.video_stream).codecpar).codec_id) };
		if self.video_codec.is_null() {
			return Err(FfmpegError::DecoderNotFound.into());
		}

		self.video_codec_context = unsafe { avcodec_alloc_context3(self.video_codec) };
		if self.video_codec_context.is_null() {
			return Err(FfmpegError::VideoCodecAllocation.into());
		}

		check_error(
			unsafe {
				avcodec_parameters_to_context(
					self.video_codec_context,
					(*self.video_stream).codecpar,
				)
			},
			"Failed to get parameters from context",
		)?;

		unsafe { (*self.video_codec_context).workaround_bugs = 1 };

		check_error(
			unsafe {
				avcodec_open2(
					self.video_codec_context,
					self.video_codec,
					std::ptr::null_mut(),
				)
			},
			"Failed to open video codec",
		)
	}

	fn find_preferred_video_stream(
		&mut self,
		prefer_embedded_metadata: bool,
	) -> Result<(), ThumbnailerError> {
		let mut video_streams = vec![];
		let mut embedded_data_streams = vec![];
		let empty_cstring = CString::new("").unwrap();

		for stream_idx in 0..(unsafe { (*self.format_context).nb_streams as i32 }) {
			let stream = unsafe { *(*self.format_context).streams.offset(stream_idx as isize) };
			let codec_params = unsafe { (*stream).codecpar };

			if unsafe { (*codec_params).codec_type } == AVMediaType::AVMEDIA_TYPE_VIDEO {
				let codec_id = unsafe { (*codec_params).codec_id };
				if !prefer_embedded_metadata
					|| !(codec_id == AVCodecID::AV_CODEC_ID_MJPEG
						|| codec_id == AVCodecID::AV_CODEC_ID_PNG)
				{
					video_streams.push(stream_idx);
					continue;
				}

				if unsafe { !(*stream).metadata.is_null() } {
					let mut tag = std::ptr::null_mut();
					loop {
						tag = unsafe {
							av_dict_get(
								(*stream).metadata,
								empty_cstring.as_ptr() as *const i8,
								tag,
								AV_DICT_IGNORE_SUFFIX,
							)
						};
						if tag.is_null() {
							break;
						}
						if unsafe {
							CString::from_raw((*tag).key).to_string_lossy() == "filename"
								&& CString::from_raw((*tag).value)
									.to_string_lossy()
									.starts_with("cover.")
						} {
							if embedded_data_streams.is_empty() {
								embedded_data_streams.push(stream_idx);
							} else {
								embedded_data_streams[0] = stream_idx;
							}
							continue;
						}
					}
				}

				embedded_data_streams.push(stream_idx);
			}
		}

		self.use_embedded_data = false;
		if prefer_embedded_metadata && !embedded_data_streams.is_empty() {
			self.use_embedded_data = true;
			self.video_stream_index = embedded_data_streams[0];
			Ok(())
		} else if !video_streams.is_empty() {
			self.video_stream_index = video_streams[0];
			Ok(())
		} else {
			Err(FfmpegError::StreamNotFound.into())
		}
	}

	fn get_video_packet(&mut self) -> bool {
		let mut frames_available = true;
		let mut frame_decoded = false;

		if !self.packet.is_null() {
			unsafe {
				av_packet_unref(self.packet);
				av_packet_free(&mut self.packet);
			}
		}

		self.packet = unsafe { av_packet_alloc() };

		while frames_available && !frame_decoded {
			frames_available = unsafe { av_read_frame(self.format_context, self.packet) == 0 };
			if frames_available {
				frame_decoded = unsafe { (*self.packet).stream_index } == self.video_stream_index;
				if !frame_decoded {
					unsafe { av_packet_unref(self.packet) };
				}
			}
		}

		frame_decoded
	}

	fn decode_video_packet(&self) -> Result<bool, ThumbnailerError> {
		if unsafe { (*self.packet).stream_index } != self.video_stream_index {
			return Ok(false);
		}

		let ret = unsafe { avcodec_send_packet(self.video_codec_context, self.packet) };
		if ret != AVERROR(EAGAIN) {
			if ret == AVERROR_EOF {
				return Ok(false);
			} else if ret < 0 {
				return Err(ThumbnailerError::FfmpegWithReason(
					FfmpegError::from(ret),
					"Failed to send packet to decoder".to_string(),
				));
			}
		}

		match unsafe { avcodec_receive_frame(self.video_codec_context, self.frame) } {
			0 => Ok(true),
			AVERROR_EAGAIN => Ok(false),
			e => Err(ThumbnailerError::FfmpegWithReason(
				FfmpegError::from(e),
				"Failed to receive frame from decoder".to_string(),
			)),
		}
	}

	fn initialize_filter_graph(
		&mut self,
		timebase: &AVRational,
		scaled_size: Option<ThumbnailSize>,
		maintain_aspect_ratio: bool,
	) -> Result<(), ThumbnailerError> {
		unsafe { self.filter_graph = avfilter_graph_alloc() };
		if self.filter_graph.is_null() {
			return Err(FfmpegError::FilterGraphAllocation.into());
		}

		let args = unsafe {
			format!(
				"video_size={}x{}:pix_fmt={}:time_base={}/{}:pixel_aspect={}/{}",
				(*self.video_codec_context).width,
				(*self.video_codec_context).height,
				(*self.video_codec_context).pix_fmt as i32,
				timebase.num,
				timebase.den,
				(*self.video_codec_context).sample_aspect_ratio.num,
				i32::max((*self.video_codec_context).sample_aspect_ratio.den, 1)
			)
		};

		setup_filter(
			&mut self.filter_source,
			"buffer",
			"thumb_buffer",
			&args,
			self.filter_graph,
			"Failed to create filter source",
		)?;

		setup_filter_without_args(
			&mut self.filter_sink,
			"buffersink",
			"thumb_buffersink",
			self.filter_graph,
			"Failed to create filter sink",
		)?;

		let mut yadif_filter = std::ptr::null_mut();
		if unsafe { (*self.frame).interlaced_frame } != 0 {
			setup_filter(
				&mut yadif_filter,
				"yadif",
				"thumb_deint",
				"deint=1",
				self.filter_graph,
				"Failed to create deinterlace filter",
			)?;
		}

		let mut scale_filter = std::ptr::null_mut();
		setup_filter(
			&mut scale_filter,
			"scale",
			"thumb_scale",
			&self.create_scale_string(scaled_size, maintain_aspect_ratio)?,
			self.filter_graph,
			"Failed to create scale filter",
		)?;

		let mut format_filter = std::ptr::null_mut();
		setup_filter(
			&mut format_filter,
			"format",
			"thumb_format",
			"pix_fmts=rgb24",
			self.filter_graph,
			"Failed to create format filter",
		)?;

		let mut rotate_filter = std::ptr::null_mut();
		let rotation = self.get_stream_rotation();
		if rotation == 3 {
			setup_filter(
				&mut rotate_filter,
				"rotate",
				"thumb_rotate",
				"PI",
				self.filter_graph,
				"Failed to create rotate filter",
			)?;
		} else if rotation != -1 {
			setup_filter(
				&mut rotate_filter,
				"transpose",
				"thumb_transpose",
				&rotation.to_string(),
				self.filter_graph,
				"Failed to create transpose filter",
			)?;
		}

		check_error(
			unsafe {
				avfilter_link(
					if !rotate_filter.is_null() {
						rotate_filter
					} else {
						format_filter
					},
					0,
					self.filter_sink,
					0,
				)
			},
			"Failed to link final filter",
		)?;

		if !rotate_filter.is_null() {
			check_error(
				unsafe { avfilter_link(format_filter, 0, rotate_filter, 0) },
				"Failed to link format filter",
			)?;
		}

		check_error(
			unsafe { avfilter_link(scale_filter, 0, format_filter, 0) },
			"Failed to link scale filter",
		)?;

		if !yadif_filter.is_null() {
			check_error(
				unsafe { avfilter_link(yadif_filter, 0, scale_filter, 0) },
				"Failed to link yadif filter",
			)?;
		}

		check_error(
			unsafe {
				avfilter_link(
					self.filter_source,
					0,
					if !yadif_filter.is_null() {
						yadif_filter
					} else {
						scale_filter
					},
					0,
				)
			},
			"Failed to link source filter",
		)?;

		check_error(
			unsafe { avfilter_graph_config(self.filter_graph, std::ptr::null_mut()) },
			"Failed to configure filter graph",
		)?;

		Ok(())
	}

	fn create_scale_string(
		&self,
		size: Option<ThumbnailSize>,
		maintain_aspect_ratio: bool,
	) -> Result<String, ThumbnailerError> {
		let mut scaled_width;
		let mut scaled_height = -1;
		if size.is_none() {
			return Ok("w=0:h=0".to_string());
		}

		let size = size.unwrap();

		match size {
			ThumbnailSize::Dimensions { width, height } => {
				scaled_width = width as i32;
				scaled_height = height as i32;
			}
			ThumbnailSize::Size(width) => {
				scaled_width = width as i32;
			}
		}

		let mut scale = String::new();

		if scaled_width != -1 && scaled_height != -1 {
			let _ = write!(scale, "w={scaled_width}:h={scaled_height}");
			if maintain_aspect_ratio {
				let _ = write!(scale, ":force_original_aspect_ratio=decrease");
			}
		} else if !maintain_aspect_ratio {
			if scaled_width == -1 {
				let _ = write!(scale, "w={scaled_height}:h={scaled_height}");
			} else {
				let _ = write!(scale, "w={scaled_width}:h={scaled_width}");
			}
		} else {
			let size_int = if scaled_height == -1 {
				scaled_width
			} else {
				scaled_height
			};

			let anamorphic;
			let aspect_ratio;
			unsafe {
				scaled_width = (*self.video_codec_context).width;
				scaled_height = (*self.video_codec_context).height;

				aspect_ratio = av_guess_sample_aspect_ratio(
					self.format_context,
					self.video_stream,
					self.frame,
				);
				anamorphic = aspect_ratio.num != 0 && aspect_ratio.num != aspect_ratio.den;
			}

			if anamorphic {
				scaled_width = scaled_width * aspect_ratio.num / aspect_ratio.den;

				if size_int != 0 {
					if scaled_height > scaled_width {
						scaled_width = scaled_width * size_int / scaled_height;
						scaled_height = size_int;
					} else {
						scaled_height = scaled_height * size_int / scaled_width;
						scaled_width = size_int;
					}
				}

				let _ = write!(scale, "w={scaled_width}:h={scaled_height}");
			} else if scaled_height > scaled_width {
				let _ = write!(
					scale,
					"w=-1:h={}",
					if size_int == 0 {
						scaled_height
					} else {
						size_int
					}
				);
			} else {
				let _ = write!(
					scale,
					"w={}:h=-1",
					if size_int == 0 {
						scaled_width
					} else {
						size_int
					}
				);
			}
		}

		Ok(scale)
	}

	fn get_stream_rotation(&self) -> i32 {
		let matrix = unsafe {
			av_stream_get_side_data(
				self.video_stream,
				AVPacketSideDataType::AV_PKT_DATA_DISPLAYMATRIX,
				std::ptr::null_mut(),
			)
		} as *const i32;

		if !matrix.is_null() {
			let angle = (unsafe { av_display_rotation_get(matrix) }).round();
			if angle < -135.0 {
				return 3;
			} else if angle > 45.0 && angle < 135.0 {
				return 2;
			} else if angle < -45.0 && angle > -135.0 {
				return 1;
			}
		}

		-1
	}
}

impl Drop for MovieDecoder {
	fn drop(&mut self) {
		if !self.video_codec_context.is_null() {
			unsafe {
				avcodec_free_context(&mut self.video_codec_context);
			}
			self.video_codec = std::ptr::null_mut();
		}

		if !self.format_context.is_null() {
			unsafe {
				avformat_close_input(&mut self.format_context);
			}
			self.format_context = std::ptr::null_mut();
		}

		if !self.packet.is_null() {
			unsafe {
				av_packet_unref(self.packet);
				av_packet_free(&mut self.packet);
				self.packet = std::ptr::null_mut();
			}
		}

		if !self.frame.is_null() {
			unsafe {
				av_frame_free(&mut self.frame);
				self.frame = std::ptr::null_mut();
			}
		}

		self.video_stream_index = -1;
	}
}

fn check_error(return_code: i32, error_message: &str) -> Result<(), ThumbnailerError> {
	if return_code < 0 {
		Err(ThumbnailerError::FfmpegWithReason(
			FfmpegError::from(return_code),
			error_message.to_string(),
		))
	} else {
		Ok(())
	}
}

fn setup_filter(
	filter_ctx: *mut *mut AVFilterContext,
	filter_name: &str,
	filter_setup_name: &str,
	args: &str,
	graph_ctx: *mut AVFilterGraph,
	error_message: &str,
) -> Result<(), ThumbnailerError> {
	let filter_name_cstr = CString::new(filter_name).unwrap();
	let filter_setup_name_cstr = CString::new(filter_setup_name).unwrap();
	let args_cstr = CString::new(args).unwrap();

	check_error(
		unsafe {
			avfilter_graph_create_filter(
				filter_ctx,
				avfilter_get_by_name(filter_name_cstr.as_ptr() as *const i8),
				filter_setup_name_cstr.as_ptr() as *const i8,
				args_cstr.as_ptr() as *const i8,
				std::ptr::null_mut(),
				graph_ctx,
			)
		},
		error_message,
	)
}

fn setup_filter_without_args(
	filter_ctx: *mut *mut AVFilterContext,
	filter_name: &str,
	filter_setup_name: &str,
	graph_ctx: *mut AVFilterGraph,
	error_message: &str,
) -> Result<(), ThumbnailerError> {
	let filter_name_cstr = CString::new(filter_name).unwrap();
	let filter_setup_name_cstr = CString::new(filter_setup_name).unwrap();

	check_error(
		unsafe {
			avfilter_graph_create_filter(
				filter_ctx,
				avfilter_get_by_name(filter_name_cstr.as_ptr() as *const i8),
				filter_setup_name_cstr.as_ptr() as *const i8,
				std::ptr::null_mut(),
				std::ptr::null_mut(),
				graph_ctx,
			)
		},
		error_message,
	)
}
