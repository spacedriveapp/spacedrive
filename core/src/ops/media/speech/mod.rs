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

use anyhow::Result;
use std::path::Path;

/// Transcribe audio/video to text using whisper
pub async fn transcribe_audio_file(
	source_path: &Path,
	model: &str,
	language: Option<&str>,
) -> Result<String> {
	use tokio::task::spawn_blocking;

	let source = source_path.to_path_buf();
	let model_name = model.to_string();
	let lang = language.map(|s| s.to_string());

	// Run whisper in blocking task (CPU/GPU intensive)
	spawn_blocking(move || {
		// TODO: Integrate whisper.rs
		// Add to Cargo.toml:
		// whisper-rs = { version = "0.11", optional = true }
		//
		// Then implement:
		// #[cfg(feature = "whisper")]
		// {
		//     use whisper_rs::{WhisperContext, FullParams, SamplingStrategy};
		//
		//     // Load model (cache this globally!)
		//     let ctx = WhisperContext::new(&format!("models/{}.bin", model_name))?;
		//
		//     // Load audio file
		//     let audio_data = load_audio_samples(&source)?;
		//
		//     // Set up params
		//     let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
		//     if let Some(l) = lang {
		//         params.set_language(Some(&l));
		//     }
		//     params.set_print_progress(false);
		//     params.set_print_timestamps(false);
		//
		//     // Run transcription
		//     ctx.full(params, &audio_data)?;
		//
		//     // Extract text as SRT format
		//     let segment_count = ctx.full_n_segments();
		//     let mut srt = String::new();
		//     for i in 0..segment_count {
		//         let start_ts = ctx.full_get_segment_t0(i);
		//         let end_ts = ctx.full_get_segment_t1(i);
		//         let text = ctx.full_get_segment_text(i)?;
		//         srt.push_str(&format_srt_segment(i + 1, start_ts, end_ts, &text));
		//     }
		//     Ok(srt)
		// }
		// #[cfg(not(feature = "whisper"))]
		// {
		//     Err(anyhow::anyhow!("Whisper feature not enabled"))
		// }

		// Placeholder implementation
		let filename = source
			.file_name()
			.map(|n| n.to_string_lossy().to_string())
			.unwrap_or_else(|| "unknown".to_string());

		Ok(format!(
			"[Speech-to-text placeholder - whisper.rs integration needed for {}]",
			filename
		))
	})
	.await?
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
