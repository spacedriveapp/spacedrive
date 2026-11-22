//! `media.footage.read`: one video's shots and transcript.

use std::sync::Arc;

use sd_timeline::{Rate, TimePoint};
use serde::{Deserialize, Serialize};
use specta::Type;
use uuid::Uuid;

use super::footage::{local_path, record_uuid, Footage, TimeInput};
use crate::{
	context::CoreContext,
	domain::SdPath,
	infra::{
		api::SessionContext,
		query::{LibraryQuery, QueryError, QueryResult},
	},
};

/// The most transcript segments one answer holds. `next_from` continues.
pub const MAX_READ_SEGMENTS: usize = 500;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct FootageReadInput {
	/// A video on this device.
	pub path: SdPath,
	/// Only what overlaps the range from here. The start of the file when
	/// absent.
	#[serde(default)]
	pub from: Option<TimeInput>,
	/// Only what overlaps the range up to here. The end when absent.
	#[serde(default)]
	pub to: Option<TimeInput>,
}

/// A shot with what was said in it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
pub struct FootageShot {
	pub shot_id: Uuid,
	#[serde(rename = "in")]
	pub in_point: TimePoint,
	/// Exclusive.
	#[serde(rename = "out")]
	pub out_point: TimePoint,
	pub text: String,
}

/// A transcript segment in the transcript's own seconds.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
pub struct FootageSegment {
	pub id: String,
	pub start: f64,
	pub end: f64,
	pub text: String,
	pub shot_id: Option<Uuid>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
pub struct FootageReadOutput {
	pub file_id: Option<Uuid>,
	pub content_uuid: Uuid,
	pub rate: Rate,
	pub duration: TimePoint,
	pub start_timecode: String,
	pub language: Option<String>,
	pub proxy: bool,
	pub shots: Vec<FootageShot>,
	pub segments: Vec<FootageSegment>,
	/// Sidecars not made yet: `transcript`, `shots` or `proxy`.
	pub missing: Vec<String>,
	/// Seconds to pass as `from` for the rest, when the range held more
	/// than `MAX_READ_SEGMENTS` segments.
	pub next_from: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FootageReadQuery {
	input: FootageReadInput,
}

impl LibraryQuery for FootageReadQuery {
	type Input = FootageReadInput;
	type Output = FootageReadOutput;

	fn from_input(input: Self::Input) -> QueryResult<Self> {
		local_path(&input.path).map_err(QueryError::InvalidInput)?;
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
		let path = local_path(&self.input.path).map_err(QueryError::InvalidInput)?;
		let video = Footage::load(&context, &library, &path)
			.await
			.map_err(QueryError::InvalidInput)?;
		let file_id = record_uuid(&context, &self.input.path).await;
		read(&video, file_id, self.input.from, self.input.to).map_err(QueryError::InvalidInput)
	}
}

pub(crate) fn read(
	video: &Footage,
	file_id: Option<Uuid>,
	from: Option<TimeInput>,
	to: Option<TimeInput>,
) -> Result<FootageReadOutput, String> {
	let rate = video.rate;
	let from = from
		.map(|time| time.seconds_at(rate))
		.transpose()?
		.unwrap_or(0.0);
	let to = to
		.map(|time| time.seconds_at(rate))
		.transpose()?
		.unwrap_or(f64::INFINITY);
	if to <= from {
		return Err("to must be after from".to_string());
	}
	let overlaps = |start: f64, end: f64| end > from && start < to;

	let mut segments = Vec::new();
	let mut next_from = None;
	if let Some(transcript) = &video.transcript {
		for segment in &transcript.segments {
			let (start, end) = (
				segment.start_ms as f64 / 1000.0,
				segment.end_ms as f64 / 1000.0,
			);
			if !overlaps(start, end) {
				continue;
			}
			if segments.len() == MAX_READ_SEGMENTS {
				next_from = Some(start);
				break;
			}
			let middle = rate.frame_at((start + end) / 2.0, sd_timeline::Rounding::Floor);
			segments.push(FootageSegment {
				id: segment.id.clone(),
				start,
				end,
				text: segment.text.trim().to_string(),
				shot_id: video.shot_at(middle).map(|shot| shot.id),
			});
		}
	}
	let last = next_from.unwrap_or(to);
	let shots = video
		.shots
		.iter()
		.flat_map(|list| &list.shots)
		.filter(|shot| {
			overlaps(shot.in_point.seconds, shot.out_point.seconds) && shot.in_point.seconds < last
		})
		.map(|shot| {
			let (in_frame, out_frame) = (shot.in_point.frame as i64, shot.out_point.frame as i64);
			FootageShot {
				shot_id: shot.id,
				in_point: video.point(in_frame),
				out_point: video.point(out_frame),
				text: video.text_between(in_frame, out_frame),
			}
		})
		.collect();
	let mut missing = Vec::new();
	if video.transcript.is_none() {
		missing.push("transcript".to_string());
	}
	if video.shots.is_none() {
		missing.push("shots".to_string());
	}
	if !video.proxy {
		missing.push("proxy".to_string());
	}
	Ok(FootageReadOutput {
		file_id,
		content_uuid: video.content_uuid,
		rate,
		duration: rate.point(video.duration, 0, video.drop_frame),
		start_timecode: video.start_timecode_text(),
		language: video.transcript.as_ref().and_then(|t| t.language.clone()),
		proxy: video.proxy,
		shots,
		segments,
		missing,
		next_from,
	})
}

crate::register_library_query!(FootageReadQuery, "media.footage.read");

#[cfg(test)]
mod tests {
	use super::*;
	use crate::ops::media::clips::search::tests::footage;

	#[test]
	fn segments_carry_their_shot_and_shots_their_words() {
		let video = footage(&[0, 60, 240, 480]);
		let out = read(&video, None, None, None).unwrap();
		assert_eq!(out.shots.len(), 3);
		assert_eq!(out.shots[0].text, "we're at the station");
		assert_eq!(out.shots[1].text, "and this is the train");
		assert_eq!(out.segments[1].shot_id, Some(out.shots[1].shot_id));
		assert_eq!(out.missing, vec!["proxy".to_string()]);
		assert_eq!(out.duration.frame, 480);
	}

	#[test]
	fn a_range_bounds_both_lists() {
		let video = footage(&[0, 60, 240, 480]);
		let out = read(
			&video,
			None,
			Some(TimeInput {
				frame: None,
				seconds: Some(11.0),
			}),
			Some(TimeInput {
				frame: None,
				seconds: Some(15.0),
			}),
		)
		.unwrap();
		assert!(out.segments.is_empty());
		assert_eq!(out.shots.len(), 1);
		assert!(read(
			&video,
			None,
			Some(TimeInput {
				frame: Some(10),
				seconds: None
			}),
			Some(TimeInput {
				frame: Some(5),
				seconds: None
			})
		)
		.is_err());
	}
}
