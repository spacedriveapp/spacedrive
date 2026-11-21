//! # Speech audio
//!
//! 16 kHz mono f32 PCM for a file, read a chunk at a time.
//!
//! The speech audio sidecar is the preferred input, since it is already at the
//! rate the model takes and is shared with every other step that listens to
//! a recording. Until one exists for a file, the original is decoded with
//! FFmpeg. Both paths stream, so memory follows the chunk, not the file.

use std::{fs::File, io::BufReader, path::Path};

use uuid::Uuid;

use crate::domain::sidecar::ContentSidecar;

/// The rate every reader here returns.
pub const SAMPLE_RATE: usize = 16_000;

/// 100 ms, the frame silence is measured over.
pub const FRAME: usize = SAMPLE_RATE / 10;

/// RMS above which a frame counts as voiced, about -46 dBFS. Room tone and
/// hiss sit below it; quiet speech sits above.
const VOICE_RMS: f32 = 0.005;

/// A stream of 16 kHz mono samples.
pub trait SpeechAudio {
	/// Up to `max` samples. Empty once the audio is done.
	fn read(&mut self, max: usize) -> Result<Vec<f32>, String>;
	/// How long the audio is, when the container says.
	fn duration_ms(&self) -> Option<u64>;
}

/// The speech audio of one file.
pub enum Opened {
	Audio(Box<dyn SpeechAudio>),
	/// The file has no audio track, so there is nothing to transcribe.
	NoAudio,
}

/// Open the speech audio of `original`: its sidecar under `library_path`
/// when one exists, otherwise the original decoded.
///
/// Runs on a blocking thread; the decoder is not `Send`.
pub fn open_speech_audio(
	library_path: &Path,
	content_uuid: Uuid,
	original: &Path,
) -> Result<Opened, String> {
	let sidecar = ContentSidecar::speech_audio(content_uuid).path_in(library_path);
	if sidecar.is_file() {
		match WavAudio::open(&sidecar) {
			Ok(audio) => return Ok(Opened::Audio(Box::new(audio))),
			// A sidecar this reader cannot take is not fatal; the original
			// says the same thing.
			Err(error) => {
				tracing::warn!(path = %sidecar.display(), %error, "ignoring speech audio sidecar")
			}
		}
	}
	// The job refused placeholders before it got here; this guards the open
	// itself, which would download the file.
	crate::ops::indexing::metadata::refuse_dataless_blocking(original)
		.map_err(|e| format!("{}: {e}", original.display()))?;
	match sd_ffmpeg::AudioStream::open(original) {
		Ok(Some(stream)) => Ok(Opened::Audio(Box::new(DecodedAudio(stream)))),
		Ok(None) => Ok(Opened::NoAudio),
		Err(error) => Err(format!("decode {}: {error}", original.display())),
	}
}

/// A `16k_mono.wav` speech audio sidecar.
struct WavAudio {
	reader: hound::WavReader<BufReader<File>>,
	duration_ms: u64,
}

impl WavAudio {
	fn open(path: &Path) -> Result<Self, String> {
		let reader = hound::WavReader::open(path).map_err(|e| e.to_string())?;
		let spec = reader.spec();
		if spec.sample_rate as usize != SAMPLE_RATE
			|| spec.channels != 1
			|| spec.bits_per_sample != 16
			|| spec.sample_format != hound::SampleFormat::Int
		{
			return Err(format!("expected 16 kHz mono 16-bit PCM, found {spec:?}"));
		}
		let duration_ms = u64::from(reader.duration()) * 1000 / SAMPLE_RATE as u64;
		Ok(Self {
			reader,
			duration_ms,
		})
	}
}

impl SpeechAudio for WavAudio {
	fn read(&mut self, max: usize) -> Result<Vec<f32>, String> {
		self.reader
			.samples::<i16>()
			.take(max)
			.map(|sample| {
				sample
					.map(|s| f32::from(s) / 32_768.0)
					.map_err(|e| e.to_string())
			})
			.collect()
	}

	fn duration_ms(&self) -> Option<u64> {
		Some(self.duration_ms)
	}
}

/// The original file, decoded by FFmpeg as it is read.
struct DecodedAudio(sd_ffmpeg::AudioStream);

impl SpeechAudio for DecodedAudio {
	fn read(&mut self, max: usize) -> Result<Vec<f32>, String> {
		self.0.read(max).map_err(|e| e.to_string())
	}

	fn duration_ms(&self) -> Option<u64> {
		self.0.duration_secs().map(|secs| (secs * 1000.0) as u64)
	}
}

fn rms(frame: &[f32]) -> f32 {
	if frame.is_empty() {
		return 0.0;
	}
	(frame.iter().map(|s| s * s).sum::<f32>() / frame.len() as f32).sqrt()
}

/// How many 100 ms frames of `samples` carry sound above room tone.
pub fn voiced_frames(samples: &[f32]) -> usize {
	samples
		.chunks(FRAME)
		.filter(|frame| rms(frame) > VOICE_RMS)
		.count()
}

/// Silence this long splits speech into separate regions, about the pause
/// between two takes or two speakers' turns.
const REGION_GAP_FRAMES: usize = 30;

/// Room kept around a region so a soft first or last syllable is not cut.
const REGION_PAD_FRAMES: usize = 3;

/// The model refuses input much shorter than this.
const MIN_REGION: usize = SAMPLE_RATE;

/// The stretches of `samples` that hold speech, padded, in order.
///
/// Each region is transcribed on its own. Whisper places the first words of
/// an input near its start whatever silence leads it, so trimming the
/// silence is what keeps a timestamp on the second the words were said.
/// Stretches with fewer than `min_voiced` voiced frames are dropped as
/// clicks and bumps.
pub fn speech_regions(samples: &[f32], min_voiced: usize) -> Vec<std::ops::Range<usize>> {
	let voiced: Vec<bool> = samples
		.chunks(FRAME)
		.map(|frame| rms(frame) > VOICE_RMS)
		.collect();
	let mut regions: Vec<(usize, usize, usize)> = Vec::new();
	for (frame, _) in voiced.iter().enumerate().filter(|(_, voiced)| **voiced) {
		match regions.last_mut() {
			Some((_, end, count)) if frame - *end <= REGION_GAP_FRAMES => {
				*end = frame;
				*count += 1;
			}
			_ => regions.push((frame, frame, 1)),
		}
	}
	regions
		.into_iter()
		.filter(|(_, _, count)| *count >= min_voiced)
		.map(|(first, last, _)| {
			let start = first.saturating_sub(REGION_PAD_FRAMES) * FRAME;
			let mut end = ((last + 1 + REGION_PAD_FRAMES) * FRAME).min(samples.len());
			if end - start < MIN_REGION {
				end = (start + MIN_REGION).min(samples.len());
			}
			start..end
		})
		.collect()
}

/// Where to end a chunk: the middle of the quietest frame in its last
/// `window` samples, so a cut rarely falls inside a word.
pub fn quietest_cut(samples: &[f32], window: usize) -> usize {
	let start = samples.len().saturating_sub(window) / FRAME * FRAME;
	samples[start..]
		.chunks(FRAME)
		.enumerate()
		.filter(|(_, frame)| frame.len() == FRAME)
		.min_by(|(_, a), (_, b)| rms(a).total_cmp(&rms(b)))
		.map_or(samples.len(), |(i, _)| start + i * FRAME + FRAME / 2)
}

#[cfg(test)]
mod tests {
	use super::*;

	fn tone(len: usize, amplitude: f32) -> Vec<f32> {
		(0..len)
			.map(|i| amplitude * (i as f32 * 0.05).sin())
			.collect()
	}

	#[test]
	fn silence_has_no_voiced_frames() {
		assert_eq!(voiced_frames(&vec![0.0; SAMPLE_RATE]), 0);
		assert_eq!(voiced_frames(&tone(SAMPLE_RATE, 0.001)), 0);
		assert_eq!(voiced_frames(&tone(SAMPLE_RATE, 0.3)), 10);
	}

	#[test]
	fn speech_splits_at_long_pauses_and_keeps_its_place() {
		let mut samples = vec![0.0; SAMPLE_RATE * 20];
		let speak = |samples: &mut Vec<f32>, from: f32, to: f32| {
			let (from, to) = ((from * 16_000.0) as usize, (to * 16_000.0) as usize);
			samples[from..to].copy_from_slice(&tone(to - from, 0.3));
		};
		speak(&mut samples, 2.0, 4.0);
		speak(&mut samples, 5.0, 6.0);
		speak(&mut samples, 12.0, 15.0);
		samples[SAMPLE_RATE * 18..SAMPLE_RATE * 18 + FRAME].copy_from_slice(&tone(FRAME, 0.3));

		let regions = speech_regions(&samples, 3);
		assert_eq!(
			regions,
			vec![
				(2 * SAMPLE_RATE - 3 * FRAME)..(6 * SAMPLE_RATE + 3 * FRAME),
				(12 * SAMPLE_RATE - 3 * FRAME)..(15 * SAMPLE_RATE + 3 * FRAME),
			],
			"a short pause stays inside a region and a lone click is dropped"
		);
		assert!(speech_regions(&vec![0.0; SAMPLE_RATE * 5], 3).is_empty());
	}

	#[test]
	fn a_chunk_is_cut_in_its_quietest_moment() {
		let mut samples = tone(SAMPLE_RATE * 10, 0.3);
		let gap = SAMPLE_RATE * 8;
		samples[gap..gap + FRAME].fill(0.0);
		assert_eq!(quietest_cut(&samples, SAMPLE_RATE * 5), gap + FRAME / 2);
	}

	#[test]
	fn a_wav_sidecar_reads_back_as_written() {
		let dir = tempfile::tempdir().unwrap();
		let path = dir.path().join("16k_mono.wav");
		let spec = hound::WavSpec {
			channels: 1,
			sample_rate: 16_000,
			bits_per_sample: 16,
			sample_format: hound::SampleFormat::Int,
		};
		let mut writer = hound::WavWriter::create(&path, spec).unwrap();
		for i in 0..32_000i32 {
			writer
				.write_sample(((i % 200) * 100 - 10_000) as i16)
				.unwrap();
		}
		writer.finalize().unwrap();

		let mut audio = WavAudio::open(&path).unwrap();
		assert_eq!(audio.duration_ms(), Some(2_000));
		let mut total = 0;
		loop {
			let chunk = audio.read(5_000).unwrap();
			if chunk.is_empty() {
				break;
			}
			total += chunk.len();
		}
		assert_eq!(total, 32_000);
	}
}
