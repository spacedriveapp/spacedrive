//! Speech-to-Text system
//!
//! Transcribes audio/video to text using whisper.rs.
//! Generates .srt subtitle files as sidecars.

pub mod action;

#[cfg(feature = "ffmpeg")]
pub mod job;
#[cfg(feature = "ffmpeg")]
pub mod processor;

pub use action::{TranscribeAudioAction, TranscribeAudioInput, TranscribeAudioOutput};

#[cfg(feature = "ffmpeg")]
pub use job::{SpeechToTextJob, SpeechToTextJobConfig};
#[cfg(feature = "ffmpeg")]
pub use processor::SpeechToTextProcessor;

use anyhow::{Context, Result};
use std::path::Path;
use std::sync::Arc;

/// Transcribe audio/video to text using whisper
#[cfg(feature = "ffmpeg")]
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
/// Uses FFmpeg libraries directly (no subprocess)
#[cfg(feature = "ffmpeg")]
fn load_audio_samples(path: &Path) -> Result<Vec<f32>> {
	// Use sd-ffmpeg to extract audio samples directly
	// This returns 16kHz mono f32 PCM samples, exactly what Whisper needs
	Ok(sd_ffmpeg::extract_audio_samples(path)?)
}

/// Format a single SRT subtitle segment
#[cfg(feature = "ffmpeg")]
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
#[cfg(feature = "ffmpeg")]
fn format_srt_timestamp(seconds: f64) -> String {
	let hours = (seconds / 3600.0).floor() as u32;
	let minutes = ((seconds % 3600.0) / 60.0).floor() as u32;
	let secs = (seconds % 60.0).floor() as u32;
	let millis = ((seconds % 1.0) * 1000.0).floor() as u32;

	format!("{:02}:{:02}:{:02},{:03}", hours, minutes, secs, millis)
}

/// Check if a file type supports speech-to-text based on content kind
pub fn is_speech_supported(mime_type: &str, registry: &crate::filetype::FileTypeRegistry) -> bool {
	use crate::domain::ContentKind;

	if let Some(file_type) = registry.get_by_mime(mime_type) {
		// Speech-to-text supported for audio and video
		matches!(file_type.category, ContentKind::Audio | ContentKind::Video)
	} else {
		// Fallback to direct MIME check
		mime_type.starts_with("audio/") || mime_type.starts_with("video/")
	}
}

/// Get audio duration in seconds using ffprobe (public for job progress estimation)
#[cfg(feature = "ffmpeg")]
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
