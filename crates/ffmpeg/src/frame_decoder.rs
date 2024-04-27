use crate::{
	codec_ctx::FFmpegCodecContext,
	error::{Error, FFmpegError},
	filter_graph::FFmpegFilterGraph,
	format_ctx::FFmpegFormatContext,
	probe::probe,
	utils::{check_error, from_path},
	video_frame::FFmpegFrame,
};

use std::{path::Path, ptr};

use chrono::TimeDelta;
use ffmpeg_sys_next::{
	av_buffersink_get_frame, av_buffersrc_write_frame, av_frame_alloc,
	av_guess_sample_aspect_ratio, av_packet_alloc, av_packet_free, av_packet_unref, av_seek_frame,
	avcodec_find_decoder, AVPacket, AVStream, AVERROR, AVPROBE_SCORE_MAX, AV_FRAME_FLAG_INTERLACED,
	AV_FRAME_FLAG_KEY, AV_TIME_BASE, EAGAIN,
};

#[derive(Debug, Clone, Copy)]
pub enum ThumbnailSize {
	Dimensions { width: u32, height: u32 },
	Size(u32),
}

#[derive(Debug)]
pub(crate) struct VideoFrame {
	pub data: Vec<u8>,
	pub width: u32,
	pub height: u32,
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

		// TODO: Remove this, just here to test and so clippy stops complaining about it being unused
		let _ = probe(filename);

		let mut format_context = FFmpegFormatContext::open_file(from_path(filename)?.as_c_str())?;

		format_context.find_stream_info()?;

		// This needs to remain at 100 or the app will force crash if it comes
		// across a video with subtitles or any type of corruption.
		if format_context.as_ref().probe_score != AVPROBE_SCORE_MAX {
			return Err(Error::CorruptVideo);
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

	pub(crate) fn use_embedded(&mut self) -> bool {
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

		self.codec_ctx.flush()?;

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
			.map(|stream| (stream.time_base, stream as *mut AVStream))
			.ok_or(FFmpegError::NullError)?;

		let pixel_aspect_ratio = unsafe {
			av_guess_sample_aspect_ratio(self.format_ctx.as_mut(), stream_ptr, self.frame.as_mut())
		};

		let rotation_angle = self
			.format_ctx
			.get_stream_rotation_angle(self.preferred_stream_id)
			.round();

		let (_guard, filter_source, filter_sink) = FFmpegFilterGraph::thumbnail_graph(
			size,
			&time_base,
			&self.codec_ctx,
			rotation_angle,
			(self.frame.as_mut().flags & AV_FRAME_FLAG_INTERLACED) != 0,
			&pixel_aspect_ratio,
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

		let height = new_frame.as_ref().height;
		let line_size = new_frame.as_ref().linesize[0];
		let mut data = Vec::with_capacity(usize::try_from(line_size * height)?);
		data.extend_from_slice(unsafe {
			std::slice::from_raw_parts(new_frame.as_ref().data[0], data.capacity())
		});

		Ok(VideoFrame {
			height: u32::try_from(height)?,
			width: new_frame.as_ref().width.try_into()?,
			data,
		})
	}

	pub fn get_duration(&self) -> Option<TimeDelta> {
		self.format_ctx.duration()
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
			} else {
				self.reset_packet();
			}
		}

		false
	}

	fn decode_packet(&mut self) -> Result<bool, Error> {
		let Some(packet) = self.is_packet_for_stream() else {
			return Ok(false);
		};

		if (match self.codec_ctx.send_packet(packet) {
			Err(Error::Again) => Ok(true),
			e => e,
		})? {
			match self.codec_ctx.receive_frame(self.frame.as_mut()) {
				Err(Error::Again) => Ok(false),
				e => e,
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
