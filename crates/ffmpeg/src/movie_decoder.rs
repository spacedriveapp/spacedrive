use crate::{
	error::{Error, FfmpegError},
	utils::from_path,
	video_frame::{FfmpegFrame, FrameSource, VideoFrame},
};

use ffmpeg_sys_next::{
	av_buffersink_get_frame, av_buffersrc_write_frame, av_dict_get, av_display_rotation_get,
	av_frame_alloc, av_frame_free, av_packet_alloc, av_packet_free, av_packet_unref, av_read_frame,
	av_seek_frame, av_stream_get_side_data, avcodec_alloc_context3, avcodec_find_decoder,
	avcodec_flush_buffers, avcodec_free_context, avcodec_open2, avcodec_parameters_to_context,
	avcodec_receive_frame, avcodec_send_packet, avfilter_get_by_name, avfilter_graph_alloc,
	avfilter_graph_config, avfilter_graph_create_filter, avfilter_graph_free, avfilter_link,
	avformat_close_input, avformat_find_stream_info, avformat_open_input, AVCodec, AVCodecContext,
	AVCodecID, AVFilterContext, AVFilterGraph, AVFormatContext, AVFrame, AVMediaType, AVPacket,
	AVPacketSideDataType, AVRational, AVStream, AVERROR, AVERROR_EOF, AVPROBE_SCORE_MAX,
	AV_DICT_IGNORE_SUFFIX, AV_TIME_BASE, EAGAIN,
};
use std::{
	ffi::{CStr, CString},
	fmt::Write,
	path::Path,
	time::Duration,
};

#[derive(Debug, Clone, Copy)]
pub enum ThumbnailSize {
	Dimensions { width: u32, height: u32 },
	Size(u32),
}

pub struct MovieDecoder {
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
	) -> Result<Self, Error> {
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
					return Err(Error::FfmpegWithReason(
						FfmpegError::from(e),
						"Failed to open input".to_string(),
					))
				}
			}
		}

		unsafe {
			// This needs to remain at 100 or the app will force crash if it comes
			// across a video with subtitles or any type of corruption.
			if (*decoder.format_context).probe_score != AVPROBE_SCORE_MAX {
				return Err(Error::CorruptVideo);
			}
		}

		decoder.initialize_video(prefer_embedded_metadata)?;

		decoder.frame = unsafe { av_frame_alloc() };
		if decoder.frame.is_null() {
			return Err(FfmpegError::FrameAllocation.into());
		}

		Ok(decoder)
	}

	pub(crate) fn decode_video_frame(&mut self) -> Result<(), Error> {
		let mut frame_finished = false;

		while !frame_finished && self.get_video_packet() {
			frame_finished = self.decode_video_packet()?;
		}

		if !frame_finished {
			return Err(Error::FrameDecodeError);
		}

		Ok(())
	}

	pub(crate) const fn embedded_metadata_is_available(&self) -> bool {
		self.use_embedded_data
	}

	pub(crate) fn seek(&mut self, seconds: i64) -> Result<(), Error> {
		if !self.allow_seek {
			return Ok(());
		}

		let timestamp = i64::from(AV_TIME_BASE).checked_mul(seconds).unwrap_or(0);

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
			return Err(Error::SeekError);
		}

		Ok(())
	}

	pub(crate) fn get_scaled_video_frame(
		&mut self,
		scaled_size: Option<ThumbnailSize>,
		maintain_aspect_ratio: bool,
		video_frame: &mut VideoFrame,
	) -> Result<(), Error> {
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
		while ret == AVERROR(EAGAIN) && attempts < 10 {
			self.decode_video_frame()?;
			check_error(
				unsafe { av_buffersrc_write_frame(self.filter_source, self.frame) },
				"Failed to write frame to filter graph",
			)?;
			ret = unsafe { av_buffersink_get_frame(self.filter_sink, new_frame.as_mut_ptr()) };
			attempts += 1;
		}
		if ret < 0 {
			return Err(Error::FfmpegWithReason(
				FfmpegError::from(ret),
				"Failed to get buffer from filter".to_string(),
			));
		}

		// SAFETY: these should always be positive, so clippy doesn't need to alert on them
		#[allow(clippy::cast_sign_loss)]
		{
			video_frame.width = unsafe { (*new_frame.as_mut_ptr()).width as u32 };
			video_frame.height = unsafe { (*new_frame.as_mut_ptr()).height as u32 };
			video_frame.line_size = unsafe { (*new_frame.as_mut_ptr()).linesize[0] as u32 };
		}
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

		if !self.filter_graph.is_null() {
			unsafe { avfilter_graph_free(&mut self.filter_graph) };
			self.filter_graph = std::ptr::null_mut();
		}

		Ok(())
	}

	// SAFETY: this should always be positive, so clippy doesn't need to alert on them
	#[allow(clippy::cast_sign_loss)]
	pub fn get_video_duration(&self) -> Duration {
		Duration::from_secs(unsafe { (*self.format_context).duration as u64 / AV_TIME_BASE as u64 })
	}

	fn initialize_video(&mut self, prefer_embedded_metadata: bool) -> Result<(), Error> {
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

	fn find_preferred_video_stream(&mut self, prefer_embedded_metadata: bool) -> Result<(), Error> {
		let mut video_streams = vec![];
		let mut embedded_data_streams = vec![];
		let empty_cstring = CString::new("").unwrap();

		for stream_idx in 0..(unsafe { (*self.format_context).nb_streams.try_into()? }) {
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
								empty_cstring.as_ptr(),
								tag,
								AV_DICT_IGNORE_SUFFIX,
							)
						};

						if tag.is_null() {
							break;
						}

						// WARNING: NEVER use CString with foreign raw pointer (causes double-free)
						let key = unsafe { CStr::from_ptr((*tag).key) }.to_str();
						if let Ok(key) = key {
							let value = unsafe { CStr::from_ptr((*tag).value) }.to_str();
							if let Ok(value) = value {
								if key == "filename" && value == "cover." {
									embedded_data_streams.insert(0, stream_idx);
									continue;
								}
							}
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
			frames_available = unsafe { av_read_frame(self.format_context, self.packet) >= 0 };
			if frames_available {
				frame_decoded = unsafe { (*self.packet).stream_index } == self.video_stream_index;
				if !frame_decoded {
					unsafe { av_packet_unref(self.packet) };
				}
			}
		}

		frame_decoded
	}

	fn decode_video_packet(&self) -> Result<bool, Error> {
		if unsafe { (*self.packet).stream_index } != self.video_stream_index {
			return Ok(false);
		}

		let ret = unsafe { avcodec_send_packet(self.video_codec_context, self.packet) };
		if ret != AVERROR(EAGAIN) {
			if ret == AVERROR_EOF {
				return Ok(false);
			} else if ret < 0 {
				return Err(Error::FfmpegWithReason(
					FfmpegError::from(ret),
					"Failed to send packet to decoder".to_string(),
				));
			}
		}

		match unsafe { avcodec_receive_frame(self.video_codec_context, self.frame) } {
			0 => Ok(true),
			e if e != AVERROR(EAGAIN) => Err(Error::FfmpegWithReason(
				FfmpegError::from(e),
				"Failed to receive frame from decoder".to_string(),
			)),
			_ => Ok(false),
		}
	}

	#[allow(clippy::too_many_lines)]
	fn initialize_filter_graph(
		&mut self,
		timebase: &AVRational,
		scaled_size: Option<ThumbnailSize>,
		maintain_aspect_ratio: bool,
	) -> Result<(), Error> {
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
				"Failed to create de-interlace filter",
			)?;
		}

		let mut scale_filter = std::ptr::null_mut();
		setup_filter(
			&mut scale_filter,
			"scale",
			"thumb_scale",
			&Self::create_scale_string(scaled_size, maintain_aspect_ratio),
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
					if rotate_filter.is_null() {
						format_filter
					} else {
						rotate_filter
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
					if yadif_filter.is_null() {
						scale_filter
					} else {
						yadif_filter
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

	fn create_scale_string(size: Option<ThumbnailSize>, maintain_aspect_ratio: bool) -> String {
		let mut scaled_width;
		let mut scaled_height = -1;
		if size.is_none() {
			return "w=0:h=0".to_string();
		}
		let size = size.expect("Size should have been checked for None");

		#[allow(clippy::cast_possible_wrap)]
		match size {
			ThumbnailSize::Dimensions { width, height } => {
				scaled_width = width as i32;
				scaled_height = height as i32;
			}
			ThumbnailSize::Size(width) => {
				scaled_width = width as i32;
			}
		}

		if scaled_width <= 0 {
			scaled_width = -1;
		}

		if scaled_height <= 0 {
			scaled_height = -1;
		}

		let mut scale = String::new();

		write!(scale, "w={scaled_width}:h={scaled_height}")
			.expect("Write of const string should work");

		if maintain_aspect_ratio {
			write!(scale, ":force_original_aspect_ratio=decrease")
				.expect("Write of const string should work");
		}

		// TODO: Handle anamorphic videos

		scale
	}

	#[allow(clippy::cast_ptr_alignment)]
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
			}
			self.packet = std::ptr::null_mut();
		}

		if !self.frame.is_null() {
			unsafe {
				av_frame_free(&mut self.frame);
				self.frame = std::ptr::null_mut();
			}
			self.frame = std::ptr::null_mut();
		}

		self.video_stream_index = -1;
	}
}

fn check_error(return_code: i32, error_message: &str) -> Result<(), Error> {
	if return_code < 0 {
		Err(Error::FfmpegWithReason(
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
) -> Result<(), Error> {
	let filter_name_cstr = CString::new(filter_name).expect("CString from str");
	let filter_setup_name_cstr = CString::new(filter_setup_name).expect("CString from str");
	let args_cstr = CString::new(args).expect("CString from str");

	check_error(
		unsafe {
			avfilter_graph_create_filter(
				filter_ctx,
				avfilter_get_by_name(filter_name_cstr.as_ptr()),
				filter_setup_name_cstr.as_ptr(),
				args_cstr.as_ptr(),
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
) -> Result<(), Error> {
	let filter_name_cstr = CString::new(filter_name).unwrap();
	let filter_setup_name_cstr = CString::new(filter_setup_name).unwrap();

	check_error(
		unsafe {
			avfilter_graph_create_filter(
				filter_ctx,
				avfilter_get_by_name(filter_name_cstr.as_ptr()),
				filter_setup_name_cstr.as_ptr(),
				std::ptr::null_mut(),
				std::ptr::null_mut(),
				graph_ctx,
			)
		},
		error_message,
	)
}
