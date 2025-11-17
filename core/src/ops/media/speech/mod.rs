//! Speech-to-Text system
//!
//! Transcribes audio/video to text using whisper.rs.
//! Generates .srt subtitle files as sidecars.

pub mod action;
pub mod job;
pub mod processor;

pub use action::{TranscribeAudioAction, TranscribeAudioInput, TranscribeAudioOutput};
pub use job::{SpeechToTextJob, SpeechToTextJobConfig};
pub use processor::SpeechToTextProcessor;

use anyhow::{Context, Result};
use std::path::Path;
use std::sync::Arc;

/// Transcribe audio/video to text using whisper
pub async fn transcribe_audio_file(
	source_path: &Path,
	model: &str,
	language: Option<&str>,
	data_dir: &Path,
) -> Result<String> {
	use tokio::task::spawn_blocking;

	let source = source_path.to_path_buf();
	let model_name = model.to_string();
	let lang = language.map(|s| s.to_string());

	// Get model path from data directory
	let model_path = crate::ops::models::get_whisper_models_dir(data_dir)
		.join(format!("ggml-{}.bin", model_name));

	if !model_path.exists() {
		anyhow::bail!(
			"Whisper model not found: {}. Please download it first.",
			model_path.display()
		);
	}

	// Run whisper in blocking task (CPU/GPU intensive)
	spawn_blocking(move || {
		use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

		// Load model
		let ctx = WhisperContext::new_with_params(
			model_path.to_str().context("Invalid model path")?,
			WhisperContextParameters::default(),
		)
		.context("Failed to load Whisper model")?;

		// Load and convert audio to 16kHz mono f32 samples
		let audio_data = load_audio_samples(&source)?;

		// Set up transcription params
		let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });

		if let Some(ref l) = lang {
			params.set_language(Some(l));
		}
		params.set_print_progress(false);
		params.set_print_special(false);
		params.set_print_realtime(false);
		params.set_print_timestamps(true);

		// Run transcription
		let mut state = ctx
			.create_state()
			.context("Failed to create whisper state")?;
		state
			.full(params, &audio_data)
			.context("Transcription failed")?;

		// Extract text as SRT format using iterator
		let mut srt = String::new();

		for (index, segment) in state.as_iter().enumerate() {
			let start_ts = segment.start_timestamp() as f64 / 100.0; // centiseconds to seconds
			let end_ts = segment.end_timestamp() as f64 / 100.0;
			let text = segment.to_str().context("Failed to get segment text")?;

			srt.push_str(&format_srt_segment(index + 1, start_ts, end_ts, text));
		}

		Ok(srt)
	})
	.await?
}

/// Load audio file and convert to 16kHz mono f32 samples required by Whisper
fn load_audio_samples(path: &Path) -> Result<Vec<f32>> {
	use hound::WavReader;
	use rubato::{
		Resampler, SincFixedIn, SincInterpolationParameters, SincInterpolationType, WindowFunction,
	};

	// For non-WAV files (like MP4 videos), extract audio using FFmpeg first
	let (wav_path, is_temp) = if path.extension().and_then(|e| e.to_str()) != Some("wav") {
		// Create temporary WAV file
		let temp_wav =
			std::env::temp_dir().join(format!("whisper_audio_{}.wav", uuid::Uuid::new_v4()));

		// Use FFmpeg to extract audio as 16kHz mono WAV
		extract_audio_to_wav(path, &temp_wav)?;

		(temp_wav, true)
	} else {
		(path.to_path_buf(), false)
	};

	// Try to read as WAV
	if let Ok(mut reader) = WavReader::open(&wav_path) {
		let spec = reader.spec();
		let sample_rate = spec.sample_rate;
		let channels = spec.channels as usize;

		// Read samples based on bit depth
		let samples: Vec<f32> = match spec.bits_per_sample {
			16 => reader
				.samples::<i16>()
				.map(|s| s.map(|v| v as f32 / i16::MAX as f32))
				.collect::<Result<Vec<_>, _>>()?,
			32 => {
				if spec.sample_format == hound::SampleFormat::Float {
					reader.samples::<f32>().collect::<Result<Vec<_>, _>>()?
				} else {
					reader
						.samples::<i32>()
						.map(|s| s.map(|v| v as f32 / i32::MAX as f32))
						.collect::<Result<Vec<_>, _>>()?
				}
			}
			_ => anyhow::bail!("Unsupported bit depth: {}", spec.bits_per_sample),
		};

		// Convert to mono if needed
		let mono_samples: Vec<f32> = if channels == 1 {
			samples
		} else {
			samples
				.chunks(channels)
				.map(|chunk| chunk.iter().sum::<f32>() / channels as f32)
				.collect()
		};

		// Resample to 16kHz if needed
		let final_samples = if sample_rate != 16000 {
			let params = SincInterpolationParameters {
				sinc_len: 256,
				f_cutoff: 0.95,
				interpolation: SincInterpolationType::Linear,
				oversampling_factor: 256,
				window: WindowFunction::BlackmanHarris2,
			};

			let mut resampler = SincFixedIn::<f32>::new(
				16000 as f64 / sample_rate as f64,
				2.0,
				params,
				mono_samples.len(),
				1,
			)?;

			let waves_in = vec![mono_samples];
			let waves_out = resampler.process(&waves_in, None)?;
			waves_out[0].clone()
		} else {
			mono_samples
		};

		// Clean up temporary WAV file if we created one
		if is_temp {
			let _ = std::fs::remove_file(&wav_path);
		}

		Ok(final_samples)
	} else {
		// Clean up temporary WAV file if we created one
		if is_temp {
			let _ = std::fs::remove_file(&wav_path);
		}

		anyhow::bail!("Failed to read WAV file: {}", wav_path.display())
	}
}

/// Extract audio from video/audio file to WAV using FFmpeg
fn extract_audio_to_wav(input_path: &Path, output_path: &Path) -> Result<()> {
	use std::process::Command;

	// Use FFmpeg to extract audio as 16kHz mono WAV
	// Suppress FFmpeg output to avoid cluttering logs
	let output = Command::new("ffmpeg")
		.args([
			"-nostdin", // Don't expect stdin
			"-loglevel",
			"error", // Only show errors
			"-i",
			input_path.to_str().context("Invalid input path")?,
			"-vn", // No video
			"-acodec",
			"pcm_s16le", // PCM 16-bit
			"-ar",
			"16000", // 16kHz sample rate
			"-ac",
			"1",  // Mono
			"-y", // Overwrite output file
			output_path.to_str().context("Invalid output path")?,
		])
		.output()
		.context("Failed to run ffmpeg. Is ffmpeg installed?")?;

	if !output.status.success() {
		let stderr = String::from_utf8_lossy(&output.stderr);
		anyhow::bail!("FFmpeg audio extraction failed: {}", stderr);
	}

	Ok(())
}

/// Format a single SRT subtitle segment
fn format_srt_segment(index: usize, start: f64, end: f64, text: &str) -> String {
	let start_time = format_srt_timestamp(start);
	let end_time = format_srt_timestamp(end);

	format!(
		"{}\n{} --> {}\n{}\n\n",
		index,
		start_time,
		end_time,
		text.trim()
	)
}

/// Format timestamp in SRT format (HH:MM:SS,mmm)
fn format_srt_timestamp(seconds: f64) -> String {
	let hours = (seconds / 3600.0).floor() as u32;
	let minutes = ((seconds % 3600.0) / 60.0).floor() as u32;
	let secs = (seconds % 60.0).floor() as u32;
	let millis = ((seconds % 1.0) * 1000.0).floor() as u32;

	format!("{:02}:{:02}:{:02},{:03}", hours, minutes, secs, millis)
}

/// Check if a file type supports speech-to-text based on content kind
pub fn is_speech_supported(mime_type: &str) -> bool {
	use crate::domain::ContentKind;
	use crate::filetype::FileTypeRegistry;

	let registry = FileTypeRegistry::new();

	if let Some(file_type) = registry.get_by_mime(mime_type) {
		// Speech-to-text supported for audio and video
		matches!(file_type.category, ContentKind::Audio | ContentKind::Video)
	} else {
		// Fallback to direct MIME check
		mime_type.starts_with("audio/") || mime_type.starts_with("video/")
	}
}

/// Get audio duration in seconds using ffprobe (public for job progress estimation)
pub async fn get_audio_duration_public(path: &Path) -> Result<f32> {
	use std::process::Command;

	let output = tokio::task::spawn_blocking({
		let path = path.to_path_buf();
		move || {
			Command::new("ffprobe")
				.args([
					"-v",
					"error",
					"-show_entries",
					"format=duration",
					"-of",
					"default=noprint_wrappers=1:nokey=1",
					path.to_str().context("Invalid path")?,
				])
				.output()
				.context("Failed to run ffprobe")
		}
	})
	.await??;

	if !output.status.success() {
		anyhow::bail!("ffprobe failed");
	}

	let duration_str = String::from_utf8_lossy(&output.stdout);
	let duration: f32 = duration_str.trim().parse().context("Invalid duration")?;

	Ok(duration)
}
