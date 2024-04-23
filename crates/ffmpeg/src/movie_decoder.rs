use crate::{
	codec_ctx::FFmpegCodecContext,
	dict::FFmpegDict,
	error::{Error, FFmpegError},
	format_ctx::FFmpegFormatContext,
	probe::probe,
	utils::{check_error, from_path, CSTRING_ERROR_MSG},
	video_frame::{FFmpegFrame, FrameSource, VideoFrame},
};

use std::{ffi::CString, path::Path, ptr};

use chrono::TimeDelta;
use ffmpeg_sys_next::{
	av_buffersink_get_frame, av_buffersrc_write_frame, av_display_rotation_get, av_frame_alloc,
	av_frame_free, av_guess_sample_aspect_ratio, av_packet_unref, av_read_frame, av_seek_frame,
	av_stream_get_side_data, avcodec_find_decoder, avfilter_get_by_name, avfilter_graph_alloc,
	avfilter_graph_config, avfilter_graph_create_filter, avfilter_graph_free, avfilter_link,
	AVFilterContext, AVFilterGraph, AVFrame, AVPacket, AVPacketSideDataType, AVRational, AVStream,
	AVERROR, AVPROBE_SCORE_MAX, AV_FRAME_FLAG_KEY, AV_TIME_BASE, EAGAIN,
};

#[derive(Debug, Clone, Copy)]
pub enum ThumbnailSize {
	Dimensions { width: u32, height: u32 },
	Size(u32),
}

pub struct MovieDecoder {
	video_stream_index: i32,
	format_context: FFmpegFormatContext,
	video_codec_context: FFmpegCodecContext,

	// TODO: Only used in one function remove from struct
	filter_graph: *mut AVFilterGraph,
	filter_source: *mut AVFilterContext,
	filter_sink: *mut AVFilterContext,

	video_stream: AVStream,
	frame: FFmpegFrame,
	packet: *mut AVPacket,
	allow_seek: bool,
	use_embedded_data: bool,
}

impl MovieDecoder {
	pub(crate) fn new(
		filename: impl AsRef<Path>,
		prefer_embedded_metadata: bool,
		allow_seek: bool,
	) -> Result<Self, Error> {
		let filename = filename.as_ref();

		// TODO: Remove this, just here to test and so clippy stops complaining about it being unused
		let _ = probe(filename);

		let mut format_context =
			FFmpegFormatContext::open_file(from_path(filename)?, &mut FFmpegDict::new(None))?;

		format_context.find_stream_info()?;

		// This needs to remain at 100 or the app will force crash if it comes
		// across a video with subtitles or any type of corruption.
		if format_context.as_ref().probe_score != AVPROBE_SCORE_MAX {
			return Err(Error::CorruptVideo);
		}

		let (use_embedded_data, video_stream_index) =
			format_context.find_preferred_video_stream(prefer_embedded_metadata)?;

		let video_stream = *unsafe { format_context.as_ref().streams.as_ref() }
			.and_then(|streams| unsafe { streams.offset(video_stream_index as isize).as_mut() })
			.ok_or(FFmpegError::NullError)?;

		let video_codec = unsafe { video_stream.codecpar.as_ref() }
			.and_then(|codecpar| unsafe { avcodec_find_decoder(codecpar.codec_id).as_ref() })
			.ok_or(FFmpegError::DecoderNotFound)?;

		let mut video_codec_context = FFmpegCodecContext::new()?;
		video_codec_context.parameters_to_context(
			unsafe { video_stream.codecpar.as_ref() }.ok_or(FFmpegError::NullError)?,
		)?;
		video_codec_context.as_mut().workaround_bugs = 1;
		video_codec_context.open2(video_codec)?;

		let frame = unsafe { av_frame_alloc() };
		if frame.is_null() {
			Err(FFmpegError::FrameAllocation)?;
		}

		Ok(Self {
			video_stream_index: -1,
			format_context,
			video_codec_context,
			filter_graph: std::ptr::null_mut(),
			filter_source: std::ptr::null_mut(),
			filter_sink: std::ptr::null_mut(),
			video_stream,
			frame: FFmpegFrame::new()?,
			packet: ptr::null_mut(),
			allow_seek,
			use_embedded_data,
		})
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
			unsafe { av_seek_frame(self.format_context.as_mut(), -1, timestamp, 0) },
			"Seeking video failed",
		)?;

		self.video_codec_context.flush()?;

		let mut got_frame = false;
		for _ in 0..200 {
			got_frame = false;
			let mut count = 0;
			while !got_frame && count < 20 {
				self.get_video_packet();
				got_frame = self.decode_video_packet().unwrap_or(false);
				count += 1;
			}

			if got_frame && self.frame.as_ref().flags & AV_FRAME_FLAG_KEY != 0 {
				break;
			}
		}

		if got_frame {
			Ok(())
		} else {
			Err(Error::SeekError)
		}
	}

	pub(crate) fn get_scaled_video_frame(
		&mut self,
		scaled_size: Option<ThumbnailSize>,
		maintain_aspect_ratio: bool,
		video_frame: &mut VideoFrame,
	) -> Result<(), Error> {
		let time_base = unsafe { self.format_context.as_ref().streams.as_ref() }
			.and_then(|streams| unsafe {
				streams.offset(self.video_stream_index as isize).as_ref()
			})
			.map(|stream| stream.time_base)
			.ok_or(FFmpegError::NullError)?;

		self.initialize_filter_graph(&time_base, scaled_size, maintain_aspect_ratio)?;

		check_error(
			unsafe { av_buffersrc_write_frame(self.filter_source, self.frame) },
			"Failed to write frame to filter graph",
		)?;

		let mut new_frame = FFmpegFrame::new()?;
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
			return Err(Error::FFmpegWithReason(
				FFmpegError::from(ret),
				"Failed to get buffer from filter".to_string(),
			));
		}

		video_frame.width = new_frame.as_ref().width.try_into()?;
		video_frame.height = new_frame.as_ref().height.try_into()?;
		video_frame.line_size = new_frame.as_ref().linesize[0].try_into()?;
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
			std::slice::from_raw_parts(new_frame.as_ref().data[0], frame_data_size)
		});

		if !self.filter_graph.is_null() {
			unsafe { avfilter_graph_free(&mut self.filter_graph) };
			self.filter_graph = std::ptr::null_mut();
		}

		Ok(())
	}

	pub fn get_video_duration(&self) -> Option<TimeDelta> {
		self.format_context.duration()
	}

	fn get_video_packet(&mut self) -> bool {
		let mut frames_available = true;
		let mut frame_decoded = false;

		if !self.packet.is_null() {
			unsafe {
				av_packet_unref(self.packet);
			}
		}

		while frames_available && !frame_decoded {
			if self.format_context.read_frame(self.packet).is_ok() {
				frame_decoded = unsafe { self.packet.as_ref() }
					.map(|packet| packet.stream_index == self.video_stream_index)
					.unwrap_or(false);
				if !frame_decoded {
					unsafe { av_packet_unref(self.packet) }
				}
			}
		}

		frame_decoded
	}

	fn decode_video_packet(&mut self) -> Result<bool, Error> {
		if unsafe { self.packet.as_ref() }
			.map(|packet| packet.stream_index != self.video_stream_index)
			.unwrap_or(false)
		{
			return Ok(false);
		}

		if (match self.video_codec_context.send_packet(self.packet) {
			Err(Error::Again) => Ok(true),
			e => e,
		})? {
			match self.video_codec_context.receive_frame(self.frame.as_mut()) {
				Err(Error::Again) => Ok(false),
				e => e,
			}
		} else {
			Ok(false)
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
			Err(FFmpegError::FilterGraphAllocation)?;
		}

		let codec_ctx = self.video_codec_context.as_ref();
		let args = format!(
			"video_size={}x{}:pix_fmt={}:time_base={}/{}:pixel_aspect={}/{}",
			codec_ctx.width,
			codec_ctx.height,
			codec_ctx.pix_fmt as i32,
			timebase.num,
			timebase.den,
			codec_ctx.sample_aspect_ratio.num,
			i32::max(codec_ctx.sample_aspect_ratio.den, 1)
		);

		setup_filter(
			&mut self.filter_source,
			CString::new("buffer").expect(CSTRING_ERROR_MSG),
			CString::new("thumb_buffer").expect(CSTRING_ERROR_MSG),
			CString::new(args)?,
			self.filter_graph,
			"Failed to create filter source",
		)?;

		setup_filter_without_args(
			&mut self.filter_sink,
			CString::new("buffersink").expect(CSTRING_ERROR_MSG),
			CString::new("thumb_buffersink").expect(CSTRING_ERROR_MSG),
			self.filter_graph,
			"Failed to create filter sink",
		)?;

		let mut yadif_filter = std::ptr::null_mut();
		if self.frame.as_mut().interlaced_frame != 0 {
			setup_filter(
				&mut yadif_filter,
				CString::new("yadif").expect(CSTRING_ERROR_MSG),
				CString::new("thumb_deint").expect(CSTRING_ERROR_MSG),
				CString::new("deint=1").expect(CSTRING_ERROR_MSG),
				self.filter_graph,
				"Failed to create de-interlace filter",
			)?;
		}

		let mut scale_filter = std::ptr::null_mut();
		setup_filter(
			&mut scale_filter,
			CString::new("scale").expect(CSTRING_ERROR_MSG),
			CString::new("thumb_scale").expect(CSTRING_ERROR_MSG),
			CString::new(self.create_scale_string(scaled_size, maintain_aspect_ratio)?)?,
			self.filter_graph,
			"Failed to create scale filter",
		)?;

		let mut format_filter = std::ptr::null_mut();
		setup_filter(
			&mut format_filter,
			CString::new("format").expect(CSTRING_ERROR_MSG),
			CString::new("thumb_format").expect(CSTRING_ERROR_MSG),
			CString::new("pix_fmts=rgb24").expect(CSTRING_ERROR_MSG),
			self.filter_graph,
			"Failed to create format filter",
		)?;

		let mut rotate_filter = std::ptr::null_mut();
		let rotation = self.get_stream_rotation();
		if rotation == 3 {
			setup_filter(
				&mut rotate_filter,
				CString::new("rotate").expect(CSTRING_ERROR_MSG),
				CString::new("thumb_rotate").expect(CSTRING_ERROR_MSG),
				CString::new("PI").expect(CSTRING_ERROR_MSG),
				self.filter_graph,
				"Failed to create rotate filter",
			)?;
		} else if rotation != -1 {
			setup_filter(
				&mut rotate_filter,
				CString::new("transpose").expect(CSTRING_ERROR_MSG),
				CString::new("thumb_transpose").expect(CSTRING_ERROR_MSG),
				CString::new(rotation.to_string())?,
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

	fn create_scale_string(
		&mut self,
		size: Option<ThumbnailSize>,
		maintain_aspect_ratio: bool,
	) -> Result<String, Error> {
		let (mut width, mut height) = match size {
			Some(ThumbnailSize::Dimensions { width, height }) => (width as i32, height as i32),
			Some(ThumbnailSize::Size(width)) => (width as i32, -1),
			None => return Ok("w=0:h=0".to_string()),
		};

		if width <= 0 {
			width = -1;
		}

		if height <= 0 {
			height = -1;
		}

		let mut scale = String::new();

		if width != -1 && height != -1 {
			scale.push_str(&format!("w={}:h={}", width, height));
			if maintain_aspect_ratio {
				scale.push_str(":force_original_aspect_ratio=decrease");
			}
		} else if !maintain_aspect_ratio {
			let size = if width == -1 { height } else { width };
			scale.push_str(&format!("w={}:h={}", size, size));
		} else {
			let size = if height == -1 { width } else { height };
			let codec_ctx = self.video_codec_context.as_ref();
			width = codec_ctx.width;
			height = codec_ctx.height;

			let par = unsafe {
				av_guess_sample_aspect_ratio(
					self.format_context.as_mut(),
					&mut self.video_stream,
					self.frame.as_mut(),
				)
			};

			// if the pixel aspect ratio is defined and is not 1, we have an anamorphic stream
			let anamorphic = par.num != 0 && par.num != par.den;
			if anamorphic {
				width = width * par.num / par.den;

				if size != 0 {
					if height > width {
						width = width * size / height;
						height = size;
					} else {
						height = height * size / width;
						width = size;
					}
				}

				scale.push_str(&format!("w={}:h={}", width, height));
			} else if height > width {
				scale.push_str(&format!("w=-1:h={}", if size == 0 { height } else { size }));
			} else {
				scale.push_str(&format!("h=-1:w={}", if size == 0 { width } else { size }));
			}
		}

		Ok(scale)
	}

	#[allow(clippy::cast_ptr_alignment)]
	fn get_stream_rotation(&self) -> i32 {
		let matrix = unsafe {
			av_stream_get_side_data(
				&self.video_stream,
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
		if !self.packet.is_null() {
			unsafe { av_packet_unref(self.packet) }
		}
		self.video_stream_index = -1;
	}
}

fn setup_filter(
	filter_ctx: *mut *mut AVFilterContext,
	filter_name: CString,
	filter_setup_name: CString,
	args: CString,
	graph_ctx: *mut AVFilterGraph,
	error_message: &str,
) -> Result<(), Error> {
	check_error(
		unsafe {
			avfilter_graph_create_filter(
				filter_ctx,
				avfilter_get_by_name(filter_name.as_ptr()),
				filter_setup_name.as_ptr(),
				args.as_ptr(),
				std::ptr::null_mut(),
				graph_ctx,
			)
		},
		error_message,
	)
}

fn setup_filter_without_args(
	filter_ctx: *mut *mut AVFilterContext,
	filter_name: CString,
	filter_setup_name: CString,
	graph_ctx: *mut AVFilterGraph,
	error_message: &str,
) -> Result<(), Error> {
	check_error(
		unsafe {
			avfilter_graph_create_filter(
				filter_ctx,
				avfilter_get_by_name(filter_name.as_ptr()),
				filter_setup_name.as_ptr(),
				std::ptr::null_mut(),
				std::ptr::null_mut(),
				graph_ctx,
			)
		},
		error_message,
	)
}
