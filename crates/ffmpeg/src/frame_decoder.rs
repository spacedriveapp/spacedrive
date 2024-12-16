use crate::{
	codec_ctx::FFmpegCodecContext,
	error::{Error, FFmpegError},
	filter_graph::FFmpegFilterGraph,
	format_ctx::FFmpegFormatContext,
	utils::{check_error, from_path},
	video_frame::FFmpegFrame,
};

use std::{path::Path, ptr};

use ffmpeg_sys_next::{
	av_buffersink_get_frame, av_buffersrc_write_frame, av_frame_alloc,
	av_guess_sample_aspect_ratio, av_packet_alloc, av_packet_free, av_packet_unref, av_seek_frame,
	avcodec_find_decoder, AVPacket, AVRational, AVStream, AVERROR, AVPROBE_SCORE_MAX,
	AV_FRAME_FLAG_INTERLACED, AV_FRAME_FLAG_KEY, AV_TIME_BASE, EAGAIN,
};

#[derive(Debug, Clone, Copy)]
pub enum ThumbnailSize {
	Scale(u32),
	Dimensions { width: u32, height: u32 },
}

#[derive(Debug)]
pub struct VideoFrame {
	pub data: Vec<u8>,
	pub width: u32,
	pub height: u32,
	pub rotation: f64,
}

pub struct FrameDecoder {
	format_ctx: FFmpegFormatContext,
	preferred_stream_id: u32,
	codec_ctx: FFmpegCodecContext,
	frame: FFmpegFrame,
	packet: *mut AVPacket,
	embedded: bool,
	allow_seek: bool,
}

impl FrameDecoder {
	pub(crate) fn new(
		filename: impl AsRef<Path>,
		allow_seek: bool,
		prefer_embedded: bool,
	) -> Result<Self, Error> {
		let filename = filename.as_ref();

		let mut format_context = FFmpegFormatContext::open_file(from_path(filename)?.as_c_str())?;

		format_context.find_stream_info()?;

		// This needs to remain at 100 or the app will force crash if it comes
		// across a video with subtitles or any type of corruption.
		if format_context.as_ref().probe_score != AVPROBE_SCORE_MAX {
			return Err(Error::CorruptVideo(
				filename.to_path_buf().into_boxed_path(),
			));
		}

		let (embedded, video_stream) =
			format_context.find_preferred_video_stream(prefer_embedded)?;

		let preferred_stream_id = u32::try_from(video_stream.index)?;

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
			format_ctx: format_context,
			preferred_stream_id,
			codec_ctx: video_codec_context,
			frame: FFmpegFrame::new()?,
			packet: ptr::null_mut(),
			allow_seek,
			embedded,
		})
	}

	pub(crate) const fn use_embedded(&self) -> bool {
		self.embedded
	}

	pub(crate) fn decode_video_frame(&mut self) -> Result<(), Error> {
		let mut frame_finished = false;

		while !frame_finished && self.find_packet_for_stream() {
			frame_finished = self.decode_packet()?;
		}

		if !frame_finished {
			return Err(Error::FrameDecodeError);
		}

		Ok(())
	}

	pub(crate) fn seek(&mut self, seconds: i64) -> Result<(), Error> {
		if !self.allow_seek {
			return Ok(());
		}

		let timestamp = i64::from(AV_TIME_BASE).checked_mul(seconds).unwrap_or(0);

		check_error(
			unsafe { av_seek_frame(self.format_ctx.as_mut(), -1, timestamp, 0) },
			"Seeking video failed",
		)?;

		self.codec_ctx.flush();

		let mut got_frame = false;
		for _ in 0..200 {
			got_frame = false;
			let mut count = 0;
			while !got_frame && count < 20 {
				self.find_packet_for_stream();
				got_frame = self.decode_packet().unwrap_or(false);
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
		size: Option<ThumbnailSize>,
		maintain_aspect_ratio: bool,
	) -> Result<VideoFrame, Error> {
		let (time_base, stream_ptr) = self
			.format_ctx
			.stream(self.preferred_stream_id)
			.map(|stream| -> (AVRational, *mut AVStream) { (stream.time_base, stream) })
			.ok_or(FFmpegError::NullError)?;

		let pixel_aspect_ratio = unsafe {
			av_guess_sample_aspect_ratio(self.format_ctx.as_mut(), stream_ptr, self.frame.as_mut())
		};

		let (_guard, filter_source, filter_sink) = FFmpegFilterGraph::thumbnail_graph(
			size,
			&time_base,
			&self.codec_ctx,
			(self.frame.as_mut().flags & AV_FRAME_FLAG_INTERLACED) != 0,
			pixel_aspect_ratio,
			maintain_aspect_ratio,
		)?;

		let mut new_frame = FFmpegFrame::new()?;
		let mut get_frame_errno = 0;
		for _ in 0..10 {
			check_error(
				unsafe { av_buffersrc_write_frame(filter_source, self.frame.as_ref()) },
				"Failed to write frame to filter graph",
			)?;

			get_frame_errno = unsafe { av_buffersink_get_frame(filter_sink, new_frame.as_mut()) };
			if get_frame_errno != AVERROR(EAGAIN) {
				break;
			}

			self.decode_video_frame()?;
		}
		check_error(get_frame_errno, "Failed to get buffer from filter")?;

		let width = new_frame.as_ref().width.unsigned_abs();
		let height = new_frame.as_ref().height.unsigned_abs();
		let line_size = usize::try_from(new_frame.as_ref().linesize[0])?;

		let mut data = Vec::with_capacity(line_size * usize::try_from(height)?);
		data.extend_from_slice(unsafe {
			std::slice::from_raw_parts(new_frame.as_ref().data[0], data.capacity())
		});

		Ok(VideoFrame {
			data,
			width,
			height,
			rotation: self
				.format_ctx
				.get_stream_rotation_angle(self.preferred_stream_id)
				.round(),
		})
	}

	pub fn get_duration_secs(&self) -> Option<f64> {
		self.format_ctx.duration().map(|duration| {
			let av_time_base = i64::from(AV_TIME_BASE);
			#[allow(clippy::cast_precision_loss)]
			{
				// SAFETY: the duration would need to be humongous for this cast to f64 to cause problems
				(duration / av_time_base) as f64
					+ ((duration % av_time_base) as f64 / f64::from(AV_TIME_BASE))
			}
		})
	}

	fn reset_packet(&mut self) {
		if self.packet.is_null() {
			self.packet = unsafe { av_packet_alloc() };
		} else {
			unsafe { av_packet_unref(self.packet) }
		}
	}

	fn is_packet_for_stream(&self) -> Option<&mut AVPacket> {
		let packet = (unsafe { self.packet.as_mut() })?;

		let packet_stream_id = u32::try_from(packet.stream_index).ok()?;

		if packet_stream_id == self.preferred_stream_id {
			Some(packet)
		} else {
			None
		}
	}

	fn find_packet_for_stream(&mut self) -> bool {
		self.reset_packet();
		while self.format_ctx.read_frame(self.packet).is_ok() {
			if self.is_packet_for_stream().is_some() {
				return true;
			}

			self.reset_packet();
		}

		false
	}

	fn decode_packet(&mut self) -> Result<bool, Error> {
		let Some(packet) = self.is_packet_for_stream() else {
			return Ok(false);
		};

		if match self.codec_ctx.send_packet(packet) {
			Ok(b) => b,
			Err(FFmpegError::Again) => true,
			Err(e) => {
				return Err(Error::FFmpegWithReason(
					e,
					"Failed to send packet to decoder".to_string(),
				))
			}
		} {
			match self.codec_ctx.receive_frame(self.frame.as_mut()) {
				Ok(ok) => Ok(ok),
				Err(FFmpegError::Again) => Ok(false),
				Err(e) => Err(Error::FFmpegWithReason(
					e,
					"Failed to receive frame from decoder".to_string(),
				)),
			}
		} else {
			Ok(false)
		}
	}
}

impl Drop for FrameDecoder {
	fn drop(&mut self) {
		unsafe {
			av_packet_free(&mut self.packet);
		}
	}
}
