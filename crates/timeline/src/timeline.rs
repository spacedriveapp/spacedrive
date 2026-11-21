//! The timeline every writer reads.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{Rate, Rounding};

/// The most events a CMX 3600 EDL can number.
pub const MAX_CLIPS: usize = 999;

/// One source file a timeline cuts from, as the editor will find it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Media {
	/// Absolute path the editor opens: the original, or a proxy.
	pub path: PathBuf,
	/// The name editors show, usually the original's file name.
	pub name: String,
	pub rate: Rate,
	pub width: u32,
	pub height: u32,
	/// The timecode of the first frame, in frames at `rate`. Zero when the
	/// file carries none.
	pub start_timecode: i64,
	pub drop_frame: bool,
	/// Length in frames at `rate`.
	pub duration: i64,
	pub audio: Option<Audio>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Audio {
	pub sample_rate: u32,
	pub channels: u32,
}

/// A range of one media file, in frames from its first frame at its own rate.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Clip {
	/// Index into [`Timeline::media`].
	pub media: usize,
	pub source_in: i64,
	/// Exclusive.
	pub source_out: i64,
	pub label: Option<String>,
}

/// Clips placed end to end on one video track, with their audio below.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Timeline {
	pub name: String,
	pub rate: Rate,
	pub width: u32,
	pub height: u32,
	pub drop_frame: bool,
	pub media: Vec<Media>,
	pub clips: Vec<Clip>,
}

/// A clip with its place on the timeline, in timeline frames.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Edit<'a> {
	pub clip: &'a Clip,
	pub media: &'a Media,
	pub record_in: i64,
	/// Exclusive.
	pub record_out: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum TimelineError {
	#[error("a timeline needs at least one clip")]
	Empty,
	#[error("a timeline holds at most {MAX_CLIPS} clips")]
	TooManyClips,
	#[error("clip {0} names media that is not in the timeline")]
	UnknownMedia(usize),
	#[error("clip {index} of {name} ends at or before it starts")]
	EmptyRange { index: usize, name: String },
	#[error("clip {index} of {name} runs outside the file, which has {duration} frames")]
	OutOfRange {
		index: usize,
		name: String,
		duration: i64,
	},
	#[error("the timeline's picture size {0}x{1} is not usable")]
	BadSize(u32, u32),
}

impl Timeline {
	/// Checks the clips against their media and places them, or says what
	/// is wrong with the first clip that cannot be placed.
	pub fn edits(&self) -> Result<Vec<Edit<'_>>, TimelineError> {
		if self.clips.is_empty() {
			return Err(TimelineError::Empty);
		}
		if self.clips.len() > MAX_CLIPS {
			return Err(TimelineError::TooManyClips);
		}
		if self.width == 0 || self.height == 0 {
			return Err(TimelineError::BadSize(self.width, self.height));
		}
		let mut record = 0;
		let mut edits = Vec::with_capacity(self.clips.len());
		for (index, clip) in self.clips.iter().enumerate() {
			let media = self
				.media
				.get(clip.media)
				.ok_or(TimelineError::UnknownMedia(index))?;
			if clip.source_out <= clip.source_in {
				return Err(TimelineError::EmptyRange {
					index,
					name: media.name.clone(),
				});
			}
			if clip.source_in < 0 || (media.duration > 0 && clip.source_out > media.duration) {
				return Err(TimelineError::OutOfRange {
					index,
					name: media.name.clone(),
					duration: media.duration,
				});
			}
			let length = media
				.rate
				.rescale(
					clip.source_out - clip.source_in,
					self.rate,
					Rounding::Nearest,
				)
				.max(1);
			edits.push(Edit {
				clip,
				media,
				record_in: record,
				record_out: record + length,
			});
			record += length;
		}
		Ok(edits)
	}

	/// Total length in timeline frames.
	pub fn duration(&self) -> Result<i64, TimelineError> {
		Ok(self.edits()?.last().map_or(0, |edit| edit.record_out))
	}

	/// Things an editor will do differently from what the timeline says,
	/// one sentence each.
	pub fn warnings(&self) -> Vec<String> {
		let mut warnings = Vec::new();
		let mut seen = Vec::new();
		for clip in &self.clips {
			let Some(media) = self.media.get(clip.media) else {
				continue;
			};
			if media.rate != self.rate && !seen.contains(&clip.media) {
				seen.push(clip.media);
				warnings.push(format!(
					"{} is {} fps on a {} fps timeline; Final Cut and Resolve conform it from the FCPXML or OTIO, but an EDL holds one rate, so do not use the EDL for this cut",
					media.name,
					fps_label(media.rate),
					fps_label(self.rate)
				));
			}
		}
		warnings
	}
}

/// `23.976`, `25`, `29.97`.
pub(crate) fn fps_label(rate: Rate) -> String {
	let fps = rate.fps();
	if rate.den == 1 {
		return rate.num.to_string();
	}
	let text = format!("{fps:.3}");
	text.trim_end_matches('0').trim_end_matches('.').to_string()
}

#[cfg(test)]
pub(crate) mod tests {
	use super::*;

	pub fn media(name: &str, rate: Rate, duration: i64) -> Media {
		Media {
			path: PathBuf::from(format!("/Volumes/Footage/Japan Trip/{name}")),
			name: name.to_string(),
			rate,
			width: 1920,
			height: 1080,
			start_timecode: 0,
			drop_frame: false,
			duration,
			audio: Some(Audio {
				sample_rate: 48000,
				channels: 2,
			}),
		}
	}

	pub fn sample() -> Timeline {
		let ntsc = Rate::new(24000, 1001).unwrap();
		let mut first = media("A001_C004.mov", ntsc, 21600);
		first.start_timecode = crate::timecode::parse("14:01:20:00", ntsc).unwrap().0;
		let mut second = media("A003_C011 & co.mov", Rate::new(25, 1).unwrap(), 2500);
		second.audio = None;
		Timeline {
			name: "Train <rough> cut".into(),
			rate: ntsc,
			width: 1920,
			height: 1080,
			drop_frame: false,
			media: vec![first, second],
			clips: vec![
				Clip {
					media: 0,
					source_in: 1230,
					source_out: 1302,
					label: None,
				},
				Clip {
					media: 1,
					source_in: 313,
					source_out: 450,
					label: Some("platform wide".into()),
				},
				Clip {
					media: 0,
					source_in: 2000,
					source_out: 2048,
					label: None,
				},
			],
		}
	}

	#[test]
	fn clips_sit_end_to_end_in_timeline_frames() {
		let timeline = sample();
		let edits = timeline.edits().unwrap();
		let places: Vec<_> = edits.iter().map(|e| (e.record_in, e.record_out)).collect();
		assert_eq!(places, vec![(0, 72), (72, 203), (203, 251)]);
		assert_eq!(timeline.duration().unwrap(), 251);
	}

	#[test]
	fn bad_clips_are_refused_by_name() {
		let mut timeline = sample();
		timeline.clips[1].source_out = 313;
		assert!(matches!(
			timeline.edits(),
			Err(TimelineError::EmptyRange { index: 1, .. })
		));
		let mut timeline = sample();
		timeline.clips[0].source_out = 30000;
		assert!(matches!(
			timeline.edits(),
			Err(TimelineError::OutOfRange { index: 0, .. })
		));
		let mut timeline = sample();
		timeline.clips.clear();
		assert_eq!(timeline.edits(), Err(TimelineError::Empty));
	}

	#[test]
	fn mixed_rates_are_named_once() {
		assert_eq!(
			sample().warnings(),
			vec!["A003_C011 & co.mov is 25 fps on a 23.976 fps timeline; Final Cut and Resolve conform it from the FCPXML or OTIO, but an EDL holds one rate, so do not use the EDL for this cut".to_string()]
		);
	}
}
