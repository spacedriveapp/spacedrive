//! Whisper over a stream of speech audio.

use std::{
	path::Path,
	sync::{
		atomic::{AtomicBool, Ordering},
		Arc, Once,
	},
};

use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

use super::{
	audio::{quietest_cut, speech_regions, voiced_frames, SpeechAudio, FRAME, SAMPLE_RATE},
	transcript::{Transcript, TranscriptSegment, TRANSCRIPT_VERSION},
};

/// Audio handed to the model per call. Whisper reads 30 s windows inside a
/// call and carries context between them, so a longer chunk keeps more
/// context; this bounds the memory a call holds to about 8 MB.
const CHUNK: usize = 120 * SAMPLE_RATE;

/// How far back from a chunk's end to look for a quiet place to cut it.
const CUT_WINDOW: usize = 10 * SAMPLE_RATE;

/// Voiced frames a stretch needs before the model sees it. Below this it is
/// room tone or a click, and the model would invent a sentence for it.
const MIN_VOICED_FRAMES: usize = 3;

/// Why a transcription stopped.
#[derive(Debug)]
pub enum TranscribeError {
	Cancelled,
	Failed(String),
}

impl From<String> for TranscribeError {
	fn from(message: String) -> Self {
		Self::Failed(message)
	}
}

/// A loaded model.
pub struct Engine {
	context: WhisperContext,
	model_id: String,
}

impl Engine {
	/// Load a ggml model file. Uses the GPU when whisper-rs was built with
	/// one (Metal on Apple platforms) and the CPU otherwise.
	pub fn load(path: &Path, model_id: &str) -> Result<Self, String> {
		static LOGS: Once = Once::new();
		LOGS.call_once(whisper_rs::install_logging_hooks);

		let path = path
			.to_str()
			.ok_or_else(|| format!("model path is not UTF-8: {}", path.display()))?;
		let context = WhisperContext::new_with_params(path, WhisperContextParameters::default())
			.map_err(|e| format!("load {path}: {e}"))?;
		Ok(Self {
			context,
			model_id: model_id.to_string(),
		})
	}

	/// Transcribe everything `audio` yields.
	///
	/// `language` pins the language; None detects it from the first chunk
	/// with speech and keeps it for the rest, so a recording is never split
	/// between languages by a misheard chunk. `progress` is called with the
	/// milliseconds read so far after every chunk.
	pub fn transcribe(
		&self,
		audio: &mut dyn SpeechAudio,
		language: Option<&str>,
		progress: &mut dyn FnMut(u64),
		cancel: &Arc<AtomicBool>,
	) -> Result<Transcript, TranscribeError> {
		let mut language = language.map(str::to_string);
		let mut segments = Vec::new();
		let mut buffer: Vec<f32> = Vec::with_capacity(CHUNK);
		let mut offset = 0usize;
		let mut ended = false;

		loop {
			if cancel.load(Ordering::Relaxed) {
				return Err(TranscribeError::Cancelled);
			}
			while !ended && buffer.len() < CHUNK {
				let read = audio.read(CHUNK - buffer.len())?;
				if read.is_empty() {
					ended = true;
				}
				buffer.extend_from_slice(&read);
			}
			if buffer.is_empty() {
				break;
			}
			let cut = if ended {
				buffer.len()
			} else {
				quietest_cut(&buffer, CUT_WINDOW)
			};
			let chunk: Vec<f32> = buffer.drain(..cut).collect();
			for region in speech_regions(&chunk, MIN_VOICED_FRAMES) {
				if cancel.load(Ordering::Relaxed) {
					return Err(TranscribeError::Cancelled);
				}
				let start = offset + region.start;
				let (found, detected) = self.run(&chunk[region], language.as_deref(), cancel)?;
				if language.is_none() && !found.is_empty() {
					language = detected;
				}
				segments.extend(found.into_iter().map(|segment| TranscriptSegment {
					id: String::new(),
					start_ms: segment.start_ms + ms(start),
					end_ms: segment.end_ms + ms(start),
					text: segment.text,
				}));
			}
			offset += cut;
			progress(ms(offset));
		}

		let mut transcript = Transcript {
			version: TRANSCRIPT_VERSION,
			model: self.model_id.clone(),
			language,
			duration_ms: ms(offset),
			segments,
		};
		transcript.assign_ids();
		Ok(transcript)
	}

	/// One model call over one chunk. Times are relative to the chunk.
	fn run(
		&self,
		chunk: &[f32],
		language: Option<&str>,
		cancel: &Arc<AtomicBool>,
	) -> Result<(Vec<TranscriptSegment>, Option<String>), TranscribeError> {
		let mut state = self
			.context
			.create_state()
			.map_err(|e| format!("whisper state: {e}"))?;
		let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
		params.set_n_threads(threads());
		params.set_language(Some(language.unwrap_or("auto")));
		params.set_print_special(false);
		params.set_print_progress(false);
		params.set_print_realtime(false);
		params.set_print_timestamps(false);
		params.set_suppress_blank(true);
		// Each chunk starts fresh: carried text is how one hallucinated line
		// repeats for minutes.
		params.set_no_context(true);
		// whisper-rs 0.15's set_abort_callback_safe hands the C side a boxed
		// trait object but reads it back as the bare closure, so every call
		// reads garbage and usually aborts. The raw callback reads the flag
		// directly; `cancel` outlives the `full` call that uses it.
		unsafe extern "C" fn aborted(flag: *mut std::ffi::c_void) -> bool {
			(*flag.cast::<AtomicBool>()).load(Ordering::Relaxed)
		}
		unsafe {
			params.set_abort_callback(Some(aborted));
			params.set_abort_callback_user_data(Arc::as_ptr(cancel).cast_mut().cast());
		}

		if let Err(error) = state.full(params, chunk) {
			if cancel.load(Ordering::Relaxed) {
				return Err(TranscribeError::Cancelled);
			}
			return Err(format!("whisper: {error}").into());
		}
		let detected =
			whisper_rs::get_lang_str(state.full_lang_id_from_state()).map(str::to_string);

		let mut segments = Vec::new();
		for segment in state.as_iter() {
			let text = segment
				.to_str_lossy()
				.map_err(|e| format!("whisper segment: {e}"))?
				.trim()
				.to_string();
			if text.is_empty() || is_annotation(&text) {
				continue;
			}
			// Whisper's timestamps are in hundredths of a second.
			let start =
				(segment.start_timestamp().max(0) as usize * SAMPLE_RATE / 100).min(chunk.len());
			let end =
				(segment.end_timestamp().max(0) as usize * SAMPLE_RATE / 100).min(chunk.len());
			// Text over a stretch with no voice in it is the model filling
			// silence, not something anyone said.
			let heard = &chunk[start.saturating_sub(FRAME)..(end + FRAME).min(chunk.len())];
			if voiced_frames(heard) == 0 {
				continue;
			}
			segments.push(TranscriptSegment {
				id: String::new(),
				start_ms: ms(start),
				end_ms: ms(end.max(start)),
				text,
			});
		}
		Ok((segments, detected))
	}
}

/// Bracketed or parenthesised text alone, like `[BLANK_AUDIO]` or
/// `(music)`: the model describing sound rather than words.
fn is_annotation(text: &str) -> bool {
	let wrapped = |open: char, close: char| text.starts_with(open) && text.ends_with(close);
	wrapped('[', ']') || wrapped('(', ')') || wrapped('*', '*')
}

fn ms(samples: usize) -> u64 {
	(samples as u64 * 1000) / SAMPLE_RATE as u64
}

/// Leave cores for the rest of the daemon; past eight, Whisper gains little.
fn threads() -> i32 {
	let available = std::thread::available_parallelism().map_or(4, |n| n.get());
	available.saturating_sub(1).clamp(1, 8) as i32
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn sound_descriptions_are_not_speech() {
		assert!(is_annotation("[BLANK_AUDIO]"));
		assert!(is_annotation("(upbeat music)"));
		assert!(!is_annotation("Hello (there)"));
	}

	#[test]
	fn sample_counts_become_milliseconds() {
		assert_eq!(ms(SAMPLE_RATE), 1000);
		assert_eq!(ms(SAMPLE_RATE * 90 + 8), 90_000);
	}
}
