//! Audio decoding: a file's first audio track as 16 kHz mono f32 PCM.
//!
//! `AudioStream` decodes on demand, so a two-hour recording costs the
//! memory of the chunk a caller asks for rather than the whole track. Channels
//! are averaged and the rate converted as each frame arrives, with the
//! resampler's position carried across frames so chunk edges add no clicks.

use crate::{
	codec_ctx::FFmpegCodecContext,
	error::{Error, FFmpegError},
	format_ctx::FFmpegFormatContext,
	packet::FFmpegPacket,
	utils::from_path,
	video_frame::FFmpegFrame,
};

use std::{collections::VecDeque, path::Path, ptr};

use ffmpeg_sys_next::{
	av_read_frame, avcodec_find_decoder, AVFrame, AVMediaType, AVSampleFormat, AV_NOPTS_VALUE,
	AV_TIME_BASE,
};

/// The rate every sample this module returns is at.
pub const SPEECH_SAMPLE_RATE: u32 = 16_000;

/// Extract audio samples from a media file as 16kHz mono f32 PCM.
///
/// Holds the whole track in memory. Use [`AudioStream`] for anything long.
pub fn extract_audio_samples(filename: impl AsRef<Path>) -> Result<Vec<f32>, Error> {
	let mut stream = AudioStream::open(filename)?.ok_or(FFmpegError::StreamNotFound)?;
	let mut samples = Vec::new();
	loop {
		let chunk = stream.read(1 << 20)?;
		if chunk.is_empty() {
			return Ok(samples);
		}
		samples.extend_from_slice(&chunk);
	}
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum State {
	Reading,
	Done,
}

/// The first audio track of a file, decoded as it is read.
pub struct AudioStream {
	format_ctx: FFmpegFormatContext,
	codec_ctx: FFmpegCodecContext,
	packet: FFmpegPacket,
	frame: FFmpegFrame,
	stream_index: i32,
	resampler: Option<Resampler>,
	buffered: VecDeque<f32>,
	duration_secs: Option<f64>,
	state: State,
}

impl AudioStream {
	/// Open a file and its first audio track, or None when it has no audio
	/// track, such as a silent screen recording.
	pub fn open(filename: impl AsRef<Path>) -> Result<Option<Self>, Error> {
		let filename = filename.as_ref();
		let mut format_ctx = FFmpegFormatContext::open_file(from_path(filename)?.as_c_str())?;
		format_ctx.find_stream_info()?;

		let Some(stream_index) = (unsafe { find_first_audio_stream(format_ctx.as_ref()) }) else {
			return Ok(None);
		};
		let stream = format_ctx
			.stream(u32::try_from(stream_index).map_err(|_| FFmpegError::StreamNotFound)?)
			.ok_or(FFmpegError::StreamNotFound)?;
		let codecpar = unsafe { stream.codecpar.as_ref() }.ok_or(FFmpegError::NullError)?;
		let decoder = unsafe { avcodec_find_decoder(codecpar.codec_id).as_ref() }
			.ok_or(FFmpegError::DecoderNotFound)?;

		let mut codec_ctx = FFmpegCodecContext::new()?;
		codec_ctx.parameters_to_context(codecpar)?;
		codec_ctx.open2(decoder)?;

		let duration = format_ctx.as_ref().duration;
		#[allow(clippy::cast_precision_loss)]
		let duration_secs = (duration != AV_NOPTS_VALUE && duration > 0)
			.then(|| duration as f64 / f64::from(AV_TIME_BASE));

		Ok(Some(Self {
			format_ctx,
			codec_ctx,
			packet: FFmpegPacket::new()?,
			frame: FFmpegFrame::new()?,
			stream_index,
			resampler: None,
			buffered: VecDeque::new(),
			duration_secs,
			state: State::Reading,
		}))
	}

	/// The container's duration, when it states one.
	#[must_use]
	pub const fn duration_secs(&self) -> Option<f64> {
		self.duration_secs
	}

	/// Up to `max` samples at 16 kHz mono. Empty once the track is done.
	pub fn read(&mut self, max: usize) -> Result<Vec<f32>, Error> {
		while self.buffered.len() < max && self.state != State::Done {
			self.decode_packet()?;
		}
		let take = max.min(self.buffered.len());
		Ok(self.buffered.drain(..take).collect())
	}

	/// Feed the decoder one packet of this track, or the end of stream, and
	/// convert every frame it gives back.
	fn decode_packet(&mut self) -> Result<(), Error> {
		loop {
			if unsafe { av_read_frame(self.format_ctx.as_mut(), self.packet.as_ptr()) } < 0 {
				// A null packet asks the decoder for the frames it still holds.
				let _ = self.codec_ctx.send_packet(ptr::null_mut());
				self.receive_frames()?;
				self.state = State::Done;
				return Ok(());
			}
			let ours = self
				.packet
				.as_ref()
				.is_some_and(|packet| packet.stream_index == self.stream_index);
			if !ours {
				self.packet.unref();
				continue;
			}
			// A packet the decoder rejects is corrupt; skipping it loses a
			// few milliseconds rather than the whole track.
			let sent = self.codec_ctx.send_packet(self.packet.as_ptr());
			self.packet.unref();
			if sent.is_ok() {
				self.receive_frames()?;
			}
			return Ok(());
		}
	}

	fn receive_frames(&mut self) -> Result<(), Error> {
		loop {
			match self.codec_ctx.receive_frame(self.frame.as_mut()) {
				Ok(true) => {
					let frame = self.frame.as_ref();
					let rate = u32::try_from(frame.sample_rate).unwrap_or(0);
					if rate == 0 {
						return Err(FFmpegError::InvalidData.into());
					}
					let mono = unsafe { frame_to_mono(frame) }?;
					let resampler = match &mut self.resampler {
						Some(resampler) if resampler.in_rate == rate => resampler,
						slot => slot.insert(Resampler::new(rate)),
					};
					resampler.push(&mono, &mut self.buffered);
				}
				Ok(false) | Err(FFmpegError::Again) => return Ok(()),
				Err(e) => return Err(e.into()),
			}
		}
	}
}

unsafe fn find_first_audio_stream(format_ctx: &ffmpeg_sys_next::AVFormatContext) -> Option<i32> {
	let streams = format_ctx.streams;
	if streams.is_null() {
		return None;
	}
	for i in 0..usize::try_from(format_ctx.nb_streams).ok()? {
		let stream = (*streams.add(i)).as_ref()?;
		let codecpar = stream.codecpar.as_ref()?;
		if codecpar.codec_type == AVMediaType::AVMEDIA_TYPE_AUDIO {
			return i32::try_from(i).ok();
		}
	}
	None
}

type SampleReader = unsafe fn(*const u8, usize) -> f32;

// The readers view FFmpeg's byte planes as their sample type, which is sound
// because FFmpeg allocates planes with at least 32-byte alignment. They are
// called through a function pointer, so being const buys nothing.

#[allow(clippy::cast_ptr_alignment, clippy::missing_const_for_fn)]
unsafe fn read_f32(plane: *const u8, i: usize) -> f32 {
	*plane.cast::<f32>().add(i)
}

#[allow(
	clippy::cast_ptr_alignment,
	clippy::cast_possible_truncation,
	clippy::missing_const_for_fn
)]
unsafe fn read_f64(plane: *const u8, i: usize) -> f32 {
	*plane.cast::<f64>().add(i) as f32
}

#[allow(clippy::cast_ptr_alignment)]
unsafe fn read_s16(plane: *const u8, i: usize) -> f32 {
	f32::from(*plane.cast::<i16>().add(i)) / 32_768.0
}

#[allow(clippy::cast_ptr_alignment, clippy::cast_precision_loss)]
unsafe fn read_s32(plane: *const u8, i: usize) -> f32 {
	*plane.cast::<i32>().add(i) as f32 / 2_147_483_648.0
}

unsafe fn read_u8(plane: *const u8, i: usize) -> f32 {
	(f32::from(*plane.add(i)) - 128.0) / 128.0
}

/// One frame as mono f32, its channels averaged.
#[allow(clippy::cast_precision_loss)]
unsafe fn frame_to_mono(frame: &AVFrame) -> Result<Vec<f32>, Error> {
	use AVSampleFormat::{
		AV_SAMPLE_FMT_DBL, AV_SAMPLE_FMT_DBLP, AV_SAMPLE_FMT_FLT, AV_SAMPLE_FMT_FLTP,
		AV_SAMPLE_FMT_S16, AV_SAMPLE_FMT_S16P, AV_SAMPLE_FMT_S32, AV_SAMPLE_FMT_S32P,
		AV_SAMPLE_FMT_U8, AV_SAMPLE_FMT_U8P,
	};

	let samples = usize::try_from(frame.nb_samples).map_err(|_| FFmpegError::InvalidData)?;
	let channels =
		usize::try_from(frame.ch_layout.nb_channels).map_err(|_| FFmpegError::InvalidData)?;
	if channels == 0 || frame.extended_data.is_null() {
		return Err(FFmpegError::InvalidData.into());
	}

	let formats: [(AVSampleFormat, bool, SampleReader); 10] = [
		(AV_SAMPLE_FMT_FLT, false, read_f32),
		(AV_SAMPLE_FMT_FLTP, true, read_f32),
		(AV_SAMPLE_FMT_DBL, false, read_f64),
		(AV_SAMPLE_FMT_DBLP, true, read_f64),
		(AV_SAMPLE_FMT_S16, false, read_s16),
		(AV_SAMPLE_FMT_S16P, true, read_s16),
		(AV_SAMPLE_FMT_S32, false, read_s32),
		(AV_SAMPLE_FMT_S32P, true, read_s32),
		(AV_SAMPLE_FMT_U8, false, read_u8),
		(AV_SAMPLE_FMT_U8P, true, read_u8),
	];
	let (_, planar, read) = formats
		.into_iter()
		.find(|(format, _, _)| *format as i32 == frame.format)
		.ok_or(FFmpegError::UnsupportedFormat)?;

	let mut mono = Vec::with_capacity(samples);
	for i in 0..samples {
		let mut sum = 0.0f32;
		for channel in 0..channels {
			sum += if planar {
				read((*frame.extended_data.add(channel)).cast_const(), i)
			} else {
				read((*frame.extended_data).cast_const(), i * channels + channel)
			};
		}
		mono.push(sum / channels as f32);
	}
	Ok(mono)
}

/// Linear-interpolation rate converter that keeps its place between calls.
///
/// Speech models are robust to the mild aliasing linear interpolation leaves,
/// and this keeps the decoder free of another native dependency.
#[derive(Debug, Clone)]
struct Resampler {
	in_rate: u32,
	/// Input samples per output sample.
	step: f64,
	/// Position of the next output sample, in input samples from the start
	/// of the buffer the next push sees (the carried sample is index 0).
	pos: f64,
	/// The last input sample of the previous push.
	carry: Option<f32>,
}

impl Resampler {
	fn new(in_rate: u32) -> Self {
		Self {
			in_rate,
			step: f64::from(in_rate) / f64::from(SPEECH_SAMPLE_RATE),
			pos: 0.0,
			carry: None,
		}
	}

	#[allow(
		clippy::cast_possible_truncation,
		clippy::cast_sign_loss,
		clippy::cast_precision_loss,
		clippy::while_float
	)]
	fn push(&mut self, input: &[f32], out: &mut VecDeque<f32>) {
		if input.is_empty() {
			return;
		}
		let carry = self.carry;
		let offset = usize::from(carry.is_some());
		let at = |i: usize| match carry {
			Some(sample) if i == 0 => sample,
			_ => input[i - offset],
		};
		let len = input.len() + offset;
		while self.pos + 1.0 < len as f64 {
			let i = self.pos as usize;
			let frac = (self.pos - i as f64) as f32;
			out.push_back(at(i).mul_add(1.0 - frac, at(i + 1) * frac));
			self.pos += self.step;
		}
		self.pos -= (len - 1) as f64;
		self.carry = input.last().copied();
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	fn run(resampler: &mut Resampler, input: &[f32], chunk: usize) -> Vec<f32> {
		let mut out = VecDeque::new();
		for part in input.chunks(chunk) {
			resampler.push(part, &mut out);
		}
		out.into_iter().collect()
	}

	#[test]
	fn chunking_does_not_change_the_output() {
		#[allow(clippy::cast_precision_loss)]
		let input: Vec<f32> = (0..48_000).map(|i| (i as f32 * 0.01).sin()).collect();
		let whole = run(&mut Resampler::new(48_000), &input, input.len());
		let pieces = run(&mut Resampler::new(48_000), &input, 1_023);
		assert_eq!(whole.len(), pieces.len());
		for (a, b) in whole.iter().zip(&pieces) {
			assert!((a - b).abs() < 1e-5);
		}
		assert!((15_990..=16_000).contains(&whole.len()), "{}", whole.len());
	}

	#[test]
	fn the_speech_rate_passes_through() {
		let input: Vec<f32> = vec![0.25; 1_000];
		let out = run(&mut Resampler::new(SPEECH_SAMPLE_RATE), &input, 7);
		assert!(out.len() >= 999);
		assert!(out.iter().all(|s| (s - 0.25).abs() < 1e-6));
	}
}
