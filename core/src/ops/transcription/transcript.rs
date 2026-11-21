//! # Transcript format
//!
//! The transcript sidecar format, and the subtitle files made from it.

use std::path::Path;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::domain::sidecar::{ContentSidecar, SidecarFormat};

/// Raised when a change to the format would mislead an older reader.
pub const TRANSCRIPT_VERSION: u32 = 1;

/// File extensions the transcription job reads, lowercase. `ts` is left
/// out because it is far more often TypeScript than an MPEG stream.
pub const MEDIA_EXTENSIONS: &[&str] = &[
	"mp3", "wav", "m4a", "m4b", "aac", "flac", "ogg", "oga", "opus", "wma", "aif", "aiff", "caf",
	"amr", "mp4", "m4v", "mov", "qt", "mkv", "webm", "avi", "wmv", "mpg", "mpeg", "mts", "m2ts",
	"mxf", "3gp", "flv",
];

/// What was said in a recording, as `transcript/default.json` holds it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Transcript {
	pub version: u32,
	/// The model that made it, such as `whisper-base`.
	pub model: String,
	/// ISO 639-1 code of the language, detected unless one was asked for.
	/// None when the recording had no speech to detect it from.
	pub language: Option<String>,
	/// How much audio was read.
	pub duration_ms: u64,
	/// In order, without overlaps.
	pub segments: Vec<TranscriptSegment>,
}

/// One timed stretch of speech.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TranscriptSegment {
	/// `seg_<start_ms>`, unique within the transcript. It names the same
	/// segment for as long as this transcript file exists, and a search hit
	/// carries the same start, so a hit can be joined back to its segment.
	pub id: String,
	pub start_ms: u64,
	pub end_ms: u64,
	pub text: String,
}

impl Transcript {
	/// Give every segment its id, adding a suffix when two start in the same
	/// millisecond.
	pub fn assign_ids(&mut self) {
		let mut seen = std::collections::HashSet::new();
		for segment in &mut self.segments {
			let base = format!("seg_{}", segment.start_ms);
			let mut id = base.clone();
			let mut n = 2;
			while !seen.insert(id.clone()) {
				id = format!("{base}_{n}");
				n += 1;
			}
			segment.id = id;
		}
	}

	/// SubRip, the subtitle format most editors import.
	pub fn to_srt(&self) -> String {
		let mut out = String::new();
		for (index, segment) in self.segments.iter().enumerate() {
			out.push_str(&format!(
				"{}\n{} --> {}\n{}\n\n",
				index + 1,
				clock(segment.start_ms, ','),
				clock(segment.end_ms, ','),
				segment.text
			));
		}
		out
	}

	/// WebVTT, the subtitle format browsers play.
	pub fn to_vtt(&self) -> String {
		let mut out = String::from("WEBVTT\n\n");
		for segment in &self.segments {
			out.push_str(&format!(
				"{} --> {}\n{}\n\n",
				clock(segment.start_ms, '.'),
				clock(segment.end_ms, '.'),
				segment.text
			));
		}
		out
	}
}

/// The transcript sidecar of `content_uuid` in a library, the candidate
/// content uuid media sidecars are keyed by. None when the recording has not
/// been transcribed, or its transcript is from a newer, unreadable version.
pub async fn read_transcript(
	library_path: &Path,
	content_uuid: Uuid,
) -> std::io::Result<Option<Transcript>> {
	let path = ContentSidecar::transcript(content_uuid, SidecarFormat::Json).path_in(library_path);
	let bytes = match tokio::fs::read(&path).await {
		Ok(bytes) => bytes,
		Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
		Err(error) => return Err(error),
	};
	Ok(serde_json::from_slice::<Transcript>(&bytes)
		.ok()
		.filter(|transcript| transcript.version <= TRANSCRIPT_VERSION))
}

/// `HH:MM:SS<sep>mmm`, the time format both subtitle formats share apart from
/// the separator before the milliseconds.
fn clock(ms: u64, separator: char) -> String {
	format!(
		"{:02}:{:02}:{:02}{separator}{:03}",
		ms / 3_600_000,
		ms / 60_000 % 60,
		ms / 1000 % 60,
		ms % 1000
	)
}

#[cfg(test)]
mod tests {
	use super::*;

	fn sample() -> Transcript {
		Transcript {
			version: TRANSCRIPT_VERSION,
			model: "whisper-base".into(),
			language: Some("en".into()),
			duration_ms: 3_725_500,
			segments: vec![
				TranscriptSegment {
					id: "seg_0".into(),
					start_ms: 0,
					end_ms: 1_500,
					text: "Hello there.".into(),
				},
				TranscriptSegment {
					id: "seg_3723040".into(),
					start_ms: 3_723_040,
					end_ms: 3_725_500,
					text: "An hour in.".into(),
				},
			],
		}
	}

	#[test]
	fn srt_numbers_cues_and_uses_a_comma() {
		assert_eq!(
			sample().to_srt(),
			"1\n00:00:00,000 --> 00:00:01,500\nHello there.\n\n\
			 2\n01:02:03,040 --> 01:02:05,500\nAn hour in.\n\n"
		);
	}

	#[test]
	fn vtt_has_a_header_and_uses_a_dot() {
		assert_eq!(
			sample().to_vtt(),
			"WEBVTT\n\n00:00:00.000 --> 00:00:01.500\nHello there.\n\n\
			 01:02:03.040 --> 01:02:05.500\nAn hour in.\n\n"
		);
	}

	#[test]
	fn ids_are_unique_even_for_segments_starting_together() {
		let mut transcript = sample();
		transcript.segments[1].start_ms = 0;
		transcript.assign_ids();
		assert_eq!(transcript.segments[0].id, "seg_0");
		assert_eq!(transcript.segments[1].id, "seg_0_2");
	}

	#[tokio::test]
	async fn a_written_transcript_reads_back_and_a_missing_one_is_none() {
		let library = tempfile::tempdir().unwrap();
		let uuid = Uuid::now_v7();
		assert!(read_transcript(library.path(), uuid)
			.await
			.unwrap()
			.is_none());
		let path = ContentSidecar::transcript(uuid, SidecarFormat::Json).path_in(library.path());
		std::fs::create_dir_all(path.parent().unwrap()).unwrap();
		std::fs::write(&path, serde_json::to_vec(&sample()).unwrap()).unwrap();
		assert_eq!(
			read_transcript(library.path(), uuid).await.unwrap(),
			Some(sample())
		);
	}

	#[test]
	fn the_json_round_trips() {
		let json = serde_json::to_string(&sample()).unwrap();
		assert_eq!(serde_json::from_str::<Transcript>(&json).unwrap(), sample());
	}
}
