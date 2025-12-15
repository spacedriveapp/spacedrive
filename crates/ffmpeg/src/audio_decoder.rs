//! Audio decoding module for extracting PCM samples from media files

use crate::{
	codec_ctx::FFmpegCodecContext,
	error::{Error, FFmpegError},
	format_ctx::FFmpegFormatContext,
	packet::FFmpegPacket,
	utils::from_path,
	video_frame::FFmpegFrame,
};

use std::{path::Path, slice};

use ffmpeg_sys_next::{av_read_frame, avcodec_find_decoder, AVFrame, AVMediaType, AVSampleFormat};

/// Extract audio samples from a media file as 16kHz mono f32 PCM
pub fn extract_audio_samples(filename: impl AsRef<Path>) -> Result<Vec<f32>, Error> {
	let filename = filename.as_ref();

	unsafe {
		let mut format_ctx = FFmpegFormatContext::open_file(from_path(filename)?.as_c_str())?;
		format_ctx.find_stream_info()?;

		// Find the best audio stream
		let audio_stream_index =
			find_best_audio_stream(format_ctx.as_ref()).ok_or(FFmpegError::StreamNotFound)?;

		let audio_stream = format_ctx
			.stream(audio_stream_index as u32)
			.ok_or(FFmpegError::StreamNotFound)?;

		// Get codec parameters
		let codecpar = audio_stream
			.codecpar
			.as_ref()
			.ok_or(FFmpegError::NullError)?;

		// Find decoder
		let decoder = avcodec_find_decoder(codecpar.codec_id)
			.as_ref()
			.ok_or(FFmpegError::DecoderNotFound)?;

		// Create codec context
		let mut codec_ctx = FFmpegCodecContext::new()?;
		codec_ctx.parameters_to_context(codecpar)?;
		codec_ctx.open2(decoder)?;

		// Allocate packet and frame using RAII wrappers for automatic cleanup
		let mut packet = FFmpegPacket::new()?;
		let mut frame = FFmpegFrame::new()?;

		let mut samples = Vec::new();

		// Read and decode packets
		while av_read_frame(format_ctx.as_mut(), packet.as_ptr()) >= 0 {
			let pkt = packet.as_ref().ok_or(FFmpegError::NullError)?;

			if pkt.stream_index == audio_stream_index {
				// Send packet to decoder
				if codec_ctx.send_packet(packet.as_ptr()).is_err() {
					packet.unref();
					continue;
				}

				// Receive decoded frames
				loop {
					match codec_ctx.receive_frame(frame.as_mut()) {
						Ok(true) => {
							// Extract samples from this frame
							let frame_samples = extract_and_convert_frame(frame.as_ref())?;
							samples.extend_from_slice(&frame_samples);
						}
						Ok(false) | Err(FFmpegError::Again) => break,
						Err(e) => {
							// RAII wrappers handle cleanup automatically via Drop
							return Err(e.into());
						}
					}
				}
			}

			packet.unref();
		}

		// RAII wrappers handle cleanup automatically when they go out of scope

		// Now resample to 16kHz mono if needed
		let codec_ref = codec_ctx.as_ref();
		let in_sample_rate = codec_ref.sample_rate;
		let in_channels = codec_ref.ch_layout.nb_channels;

		let final_samples = if in_sample_rate != 16000 || in_channels != 1 {
			resample_audio(&samples, in_sample_rate, in_channels, 16000, 1)?
		} else {
			samples
		};

		Ok(final_samples)
	}
}

/// Find the best audio stream in a format context
unsafe fn find_best_audio_stream(format_ctx: &ffmpeg_sys_next::AVFormatContext) -> Option<i32> {
	let streams = format_ctx.streams;
	if streams.is_null() {
		return None;
	}

	for i in 0..format_ctx.nb_streams {
		let stream = (*streams.add(i as usize)).as_ref()?;
		let codecpar = stream.codecpar.as_ref()?;

		if codecpar.codec_type == AVMediaType::AVMEDIA_TYPE_AUDIO {
			return Some(i as i32);
		}
	}
	None
}

/// Extract and convert audio frame to f32 samples
unsafe fn extract_and_convert_frame(frame: &AVFrame) -> Result<Vec<f32>, Error> {
	let nb_samples = frame.nb_samples as usize;
	let channels = frame.ch_layout.nb_channels as usize;
	let format = frame.format;

	match format {
		f if f == AVSampleFormat::AV_SAMPLE_FMT_FLT as i32 => {
			// Interleaved f32 - perfect, just copy
			let data = slice::from_raw_parts(frame.data[0] as *const f32, nb_samples * channels);
			Ok(data.to_vec())
		}
		f if f == AVSampleFormat::AV_SAMPLE_FMT_FLTP as i32 => {
			// Planar f32 - interleave it
			let mut output = Vec::with_capacity(nb_samples * channels);
			for i in 0..nb_samples {
				for ch in 0..channels {
					let channel_data =
						slice::from_raw_parts(frame.data[ch] as *const f32, nb_samples);
					output.push(channel_data[i]);
				}
			}
			Ok(output)
		}
		f if f == AVSampleFormat::AV_SAMPLE_FMT_S16 as i32 => {
			// Interleaved s16 - convert to f32
			let data = slice::from_raw_parts(frame.data[0] as *const i16, nb_samples * channels);
			Ok(data.iter().map(|&s| s as f32 / 32768.0).collect())
		}
		f if f == AVSampleFormat::AV_SAMPLE_FMT_S16P as i32 => {
			// Planar s16 - interleave and convert
			let mut output = Vec::with_capacity(nb_samples * channels);
			for i in 0..nb_samples {
				for ch in 0..channels {
					let channel_data =
						slice::from_raw_parts(frame.data[ch] as *const i16, nb_samples);
					output.push(channel_data[i] as f32 / 32768.0);
				}
			}
			Ok(output)
		}
		f if f == AVSampleFormat::AV_SAMPLE_FMT_S32 as i32 => {
			// Interleaved s32 - convert to f32
			let data = slice::from_raw_parts(frame.data[0] as *const i32, nb_samples * channels);
			Ok(data.iter().map(|&s| s as f32 / 2147483648.0).collect())
		}
		f if f == AVSampleFormat::AV_SAMPLE_FMT_S32P as i32 => {
			// Planar s32 - interleave and convert
			let mut output = Vec::with_capacity(nb_samples * channels);
			for i in 0..nb_samples {
				for ch in 0..channels {
					let channel_data =
						slice::from_raw_parts(frame.data[ch] as *const i32, nb_samples);
					output.push(channel_data[i] as f32 / 2147483648.0);
				}
			}
			Ok(output)
		}
		_ => Err(FFmpegError::UnsupportedFormat.into()),
	}
}

/// Simple resampling using linear interpolation
/// For production, this should use a proper resampling library
fn resample_audio(
	samples: &[f32],
	in_rate: i32,
	in_channels: i32,
	out_rate: i32,
	out_channels: i32,
) -> Result<Vec<f32>, Error> {
	if samples.is_empty() {
		return Ok(Vec::new());
	}

	let in_rate = in_rate as usize;
	let out_rate = out_rate as usize;
	let in_channels = in_channels as usize;
	let out_channels = out_channels as usize;

	let in_frames = samples.len() / in_channels;
	let out_frames = (in_frames * out_rate + in_rate - 1) / in_rate;

	let mut output = Vec::with_capacity(out_frames * out_channels);

	for out_frame_idx in 0..out_frames {
		// Calculate corresponding input frame (with fractional part)
		let in_frame_pos = (out_frame_idx * in_rate) as f32 / out_rate as f32;
		let in_frame_idx = in_frame_pos as usize;
		let frac = in_frame_pos - in_frame_idx as f32;

		// For each output channel
		for out_ch in 0..out_channels {
			let mut sample = 0.0f32;

			if in_channels == out_channels {
				// Same channel count - just resample
				let in_ch = out_ch;

				if in_frame_idx + 1 < in_frames {
					let s1 = samples[in_frame_idx * in_channels + in_ch];
					let s2 = samples[(in_frame_idx + 1) * in_channels + in_ch];
					sample = s1 * (1.0 - frac) + s2 * frac;
				} else if in_frame_idx < in_frames {
					sample = samples[in_frame_idx * in_channels + in_ch];
				}
			} else if in_channels > out_channels {
				// Downmix (e.g., stereo to mono) - average channels
				let mut sum = 0.0f32;
				let mut count = 0;

				for in_ch in 0..in_channels {
					if in_frame_idx + 1 < in_frames {
						let s1 = samples[in_frame_idx * in_channels + in_ch];
						let s2 = samples[(in_frame_idx + 1) * in_channels + in_ch];
						sum += s1 * (1.0 - frac) + s2 * frac;
						count += 1;
					} else if in_frame_idx < in_frames {
						sum += samples[in_frame_idx * in_channels + in_ch];
						count += 1;
					}
				}

				sample = if count > 0 { sum / count as f32 } else { 0.0 };
			} else {
				// Upmix (e.g., mono to stereo) - duplicate channel
				let in_ch = 0;

				if in_frame_idx + 1 < in_frames {
					let s1 = samples[in_frame_idx * in_channels + in_ch];
					let s2 = samples[(in_frame_idx + 1) * in_channels + in_ch];
					sample = s1 * (1.0 - frac) + s2 * frac;
				} else if in_frame_idx < in_frames {
					sample = samples[in_frame_idx * in_channels + in_ch];
				}
			}

			output.push(sample);
		}
	}

	Ok(output)
}
