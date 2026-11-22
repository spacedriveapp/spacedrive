//! `media.clips.search`: the moments where words were spoken, as clips.

use std::{
	collections::HashMap,
	path::{Path, PathBuf},
	sync::Arc,
};

use sd_timeline::{Rate, Rounding, TimePoint};
use serde::{Deserialize, Serialize};
use specta::Type;
use uuid::Uuid;

use super::footage::{local_path, record_uuid, Footage};
use crate::{
	context::CoreContext,
	domain::SdPath,
	infra::{
		api::SessionContext,
		query::{LibraryQuery, QueryError, QueryResult},
	},
	ops::{
		indexing::text_extract::TextKind,
		search::content::{ContentSearchInput, ContentSearchQuery},
	},
};

/// Handles added around speech when the caller does not say.
pub const DEFAULT_PAD_SECONDS: f64 = 0.5;
pub const MAX_PAD_SECONDS: f64 = 5.0;

/// Where a clip's ends fall.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum ClipSnap {
	/// The speech plus padding, with the padding kept inside the shot.
	#[default]
	Segment,
	/// The whole shot that holds the speech.
	Shot,
}

/// Find the moments under `scope` where `query` was said.
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct ClipSearchInput {
	/// A directory or video under a source tracked on this device.
	pub scope: SdPath,
	/// Words that must all appear in one transcript segment, with
	/// `search.content`'s syntax.
	pub query: String,
	#[serde(default)]
	pub snap: ClipSnap,
	/// Seconds of handle before and after the speech, 0 to 5. Defaults to 0.5.
	#[serde(default)]
	pub pad_seconds: Option<f64>,
	/// Transcript hits read per page, 1 to 100. Adjacent hits in one shot
	/// merge, so a page can hold fewer clips. Defaults to 20.
	#[serde(default)]
	pub max_results: Option<u32>,
	#[serde(default)]
	pub cursor: Option<String>,
}

/// What was said in a clip.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
pub struct ClipTranscript {
	pub text: String,
	pub language: Option<String>,
}

/// A range of one video.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
pub struct Clip {
	/// The video, relative to the scope with `/` separators. Its name when
	/// the scope is the video itself.
	pub path: String,
	pub sd_path: SdPath,
	/// The record uuid in the source store. None until a walk has reached the
	/// file.
	pub file_id: Option<Uuid>,
	/// The candidate content uuid the video's sidecars are keyed by.
	pub content_uuid: Uuid,
	pub rate: Rate,
	#[serde(rename = "in")]
	pub in_point: TimePoint,
	/// Exclusive: the first frame after the clip.
	#[serde(rename = "out")]
	pub out_point: TimePoint,
	/// The shot the clip sits in. None before shot detection has run.
	pub shot_id: Option<Uuid>,
	pub transcript: ClipTranscript,
	/// The words of the query the transcript matched.
	pub matched: Vec<String>,
	/// BM25 relevance of the best hit in the clip. Only the order within one
	/// response means anything.
	pub score: f64,
	/// Whether a 720p proxy exists for the video.
	pub proxy: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, Type)]
pub struct ClipSearchOutput {
	/// Ordered by relevance.
	pub clips: Vec<Clip>,
	pub next_cursor: Option<String>,
	/// Files under the scope whose text is not indexed yet.
	pub pending_files: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClipSearchQuery {
	input: ClipSearchInput,
}

impl LibraryQuery for ClipSearchQuery {
	type Input = ClipSearchInput;
	type Output = ClipSearchOutput;

	fn from_input(input: Self::Input) -> QueryResult<Self> {
		local_path(&input.scope).map_err(QueryError::InvalidInput)?;
		if let Some(pad) = input.pad_seconds {
			if !(0.0..=MAX_PAD_SECONDS).contains(&pad) {
				return Err(QueryError::InvalidInput(format!(
					"pad_seconds must be between 0 and {MAX_PAD_SECONDS}"
				)));
			}
		}
		if let Some(max) = input.max_results {
			if !(1..=100).contains(&max) {
				return Err(QueryError::InvalidInput(
					"max_results must be between 1 and 100".to_string(),
				));
			}
		}
		Ok(Self { input })
	}

	async fn execute(
		self,
		context: Arc<CoreContext>,
		session: SessionContext,
	) -> QueryResult<Self::Output> {
		let library_id = session
			.current_library_id
			.ok_or_else(|| QueryError::Internal("No library in session".to_string()))?;
		let library = context
			.libraries()
			.await
			.get_library(library_id)
			.await
			.ok_or(QueryError::LibraryNotFound(library_id))?;
		let scope = local_path(&self.input.scope).map_err(QueryError::InvalidInput)?;
		let found = ContentSearchQuery::from_input(ContentSearchInput {
			scope: self.input.scope.clone(),
			query: self.input.query.clone(),
			kinds: vec![TextKind::Transcript],
			max_results: self.input.max_results,
			cursor: self.input.cursor.clone(),
		})?
		.execute(context.clone(), session)
		.await?;

		let file_scope = tokio::fs::metadata(&scope)
			.await
			.is_ok_and(|meta| meta.is_file());
		let pad = self.input.pad_seconds.unwrap_or(DEFAULT_PAD_SECONDS);
		let mut footage: HashMap<String, Option<(PathBuf, Footage, Option<Uuid>)>> = HashMap::new();
		let mut clips: Vec<Clip> = Vec::new();
		for hit in found.hits {
			let Some(time) = hit.time else {
				continue;
			};
			if !footage.contains_key(&hit.path) {
				let path = if file_scope {
					scope.clone()
				} else {
					scope.join(&hit.path)
				};
				let loaded = match Footage::load(&context, &library, &path).await {
					Ok(loaded) => {
						let file_id = record_uuid(&context, &SdPath::local(path.clone())).await;
						Some((path, loaded, file_id))
					}
					Err(error) => {
						tracing::debug!(%error, "a transcript hit's video could not be read");
						None
					}
				};
				footage.insert(hit.path.clone(), loaded);
			}
			let Some((path, video, file_id)) = &footage[&hit.path] else {
				continue;
			};
			let range = clip_range(
				video,
				time.start_seconds,
				time.end_seconds,
				pad,
				self.input.snap,
			);
			let mut matched: Vec<String> = hit
				.snippet
				.iter()
				.filter(|part| part.matched)
				.map(|part| part.text.clone())
				.collect();
			matched.sort_unstable();
			matched.dedup();
			let candidate = Candidate {
				path: hit.path.clone(),
				range,
				matched,
				score: hit.score,
			};
			if let Some(existing) = clips.iter_mut().find(|clip| {
				clip.path == candidate.path
					&& clip.shot_id == range.shot
					&& range.in_frame <= clip.out_point.frame
					&& range.out_frame >= clip.in_point.frame
			}) {
				let in_frame = existing.in_point.frame.min(range.in_frame);
				let out_frame = existing.out_point.frame.max(range.out_frame);
				existing.in_point = video.point(in_frame);
				existing.out_point = video.point(out_frame);
				existing.transcript.text = video.text_between(in_frame, out_frame);
				existing.matched.extend(candidate.matched);
				existing.matched.sort_unstable();
				existing.matched.dedup();
				existing.score = existing.score.max(candidate.score);
				continue;
			}
			clips.push(make_clip(path, video, *file_id, candidate));
		}
		Ok(ClipSearchOutput {
			clips,
			next_cursor: found.next_cursor,
			pending_files: found.pending_files,
		})
	}
}

struct Candidate {
	path: String,
	range: ClipRange,
	matched: Vec<String>,
	score: f64,
}

fn make_clip(path: &Path, video: &Footage, file_id: Option<Uuid>, candidate: Candidate) -> Clip {
	let range = candidate.range;
	Clip {
		path: candidate.path,
		sd_path: SdPath::local(path.to_path_buf()),
		file_id,
		content_uuid: video.content_uuid,
		rate: video.rate,
		in_point: video.point(range.in_frame),
		out_point: video.point(range.out_frame),
		shot_id: range.shot,
		transcript: ClipTranscript {
			text: video.text_between(range.in_frame, range.out_frame),
			language: video
				.transcript
				.as_ref()
				.and_then(|transcript| transcript.language.clone()),
		},
		matched: candidate.matched,
		score: candidate.score,
		proxy: video.proxy,
	}
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ClipRange {
	pub in_frame: i64,
	pub out_frame: i64,
	pub shot: Option<Uuid>,
}

/// The frames a clip of the speech between `start` and `end` seconds covers.
///
/// The speech itself is never cut: an in point rounds down and an out point
/// up. Padding stops at the edges of the shot that holds the middle of the
/// speech, so a clip does not flash a frame of the neighbouring shot.
pub(crate) fn clip_range(
	video: &Footage,
	start: f64,
	end: f64,
	pad: f64,
	snap: ClipSnap,
) -> ClipRange {
	let rate = video.rate;
	let speech_in = rate.frame_at(start, Rounding::Floor).max(0);
	let speech_out = rate.frame_at(end, Rounding::Ceil).max(speech_in + 1);
	let shot = video.shot_at((speech_in + speech_out - 1) / 2);
	let (mut in_frame, mut out_frame) = match (snap, shot) {
		(ClipSnap::Shot, Some(shot)) => (shot.in_point.frame as i64, shot.out_point.frame as i64),
		(_, shot) => {
			let padded_in = rate.frame_at(start - pad, Rounding::Floor);
			let padded_out = rate.frame_at(end + pad, Rounding::Ceil);
			match shot {
				Some(shot) => (
					padded_in.max((shot.in_point.frame as i64).min(speech_in)),
					padded_out.min((shot.out_point.frame as i64).max(speech_out)),
				),
				None => (padded_in, padded_out),
			}
		}
	};
	in_frame = in_frame.max(0);
	if video.duration > 0 {
		out_frame = out_frame.min(video.duration);
	}
	ClipRange {
		in_frame,
		out_frame: out_frame.max(in_frame + 1),
		shot: shot.map(|shot| shot.id),
	}
}

crate::register_library_query!(ClipSearchQuery, "media.clips.search");

#[cfg(test)]
pub(crate) mod tests {
	use super::*;
	use crate::ops::{
		media::shots::{AnalyzedMedia, FramePoint, FrameRate, Shot, ShotList},
		transcription::transcript::{Transcript, TranscriptSegment},
	};

	fn point(frame: u64) -> FramePoint {
		FramePoint {
			frame,
			seconds: frame as f64 / 24.0,
			timecode: String::new(),
		}
	}

	pub fn footage(cuts: &[u64]) -> Footage {
		let content_uuid = Uuid::nil();
		let shots = cuts
			.windows(2)
			.enumerate()
			.map(|(index, pair)| Shot {
				id: Shot::id_for(content_uuid, pair[0], pair[1]),
				index: index as u32,
				in_point: point(pair[0]),
				out_point: point(pair[1]),
				duration_frames: pair[1] - pair[0],
				duration_seconds: 0.0,
				keyframe: point(pair[0]),
				cut_score: 0.5,
			})
			.collect();
		Footage {
			content_uuid,
			rate: Rate::new(24, 1).unwrap(),
			start_timecode: 0,
			drop_frame: false,
			duration: *cuts.last().unwrap() as i64,
			shots: Some(ShotList {
				version: 1,
				content_uuid,
				analyzed: AnalyzedMedia::Original,
				rate: FrameRate { num: 24, den: 1 },
				start_timecode: None,
				out_exclusive: true,
				duration: point(*cuts.last().unwrap()),
				threshold: 0.3,
				min_shot_seconds: 1.0,
				shots,
			}),
			transcript: Some(Transcript {
				version: 1,
				model: "test".into(),
				language: Some("en".into()),
				duration_ms: 20_000,
				segments: vec![
					TranscriptSegment {
						id: "seg_1000".into(),
						start_ms: 1000,
						end_ms: 2500,
						text: " we're at the station".into(),
					},
					TranscriptSegment {
						id: "seg_2600".into(),
						start_ms: 2600,
						end_ms: 4100,
						text: " and this is the train".into(),
					},
				],
			}),
			proxy: false,
		}
	}

	#[test]
	fn padding_stays_inside_the_shot_but_speech_is_never_cut() {
		// Shots: [0, 60) [60, 240) [240, 480).
		let video = footage(&[0, 60, 240, 480]);
		// Half a second before 2.6 s would reach into the first shot.
		let range = clip_range(&video, 2.6, 4.1, 0.5, ClipSnap::Segment);
		assert_eq!((range.in_frame, range.out_frame), (60, 111));
		assert_eq!(range.shot, Some(Shot::id_for(Uuid::nil(), 60, 240)));
		let range = clip_range(&video, 3.5, 4.1, 0.5, ClipSnap::Segment);
		assert_eq!(range.in_frame, 72);
		let range = clip_range(&video, 2.6, 9.9, 0.5, ClipSnap::Segment);
		assert_eq!(range.out_frame, 240);
		// Speech that starts before its shot keeps its first word.
		let range = clip_range(&video, 2.4, 4.1, 0.5, ClipSnap::Segment);
		assert_eq!(range.in_frame, 57);
	}

	#[test]
	fn snap_shot_returns_the_whole_shot() {
		let video = footage(&[0, 60, 240, 480]);
		let range = clip_range(&video, 2.6, 4.1, 0.5, ClipSnap::Shot);
		assert_eq!((range.in_frame, range.out_frame), (60, 240));
	}

	#[test]
	fn without_shots_a_clip_is_the_padded_speech_inside_the_file() {
		let mut video = footage(&[0, 480]);
		video.shots = None;
		let range = clip_range(&video, 0.2, 19.9, 0.5, ClipSnap::Shot);
		assert_eq!(
			(range.in_frame, range.out_frame, range.shot),
			(0, 480, None)
		);
	}

	#[test]
	fn clip_text_is_every_segment_it_overlaps() {
		let video = footage(&[0, 480]);
		assert_eq!(video.text_between(62, 111), "and this is the train");
		assert_eq!(
			video.text_between(0, 111),
			"we're at the station and this is the train"
		);
	}
}
