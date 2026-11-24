//! # Shots sidecar
//!
//! The shape of the shot list, and the one place that reads and writes it.

use std::{
	io,
	path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};
use specta::Type;
use uuid::Uuid;

use crate::{
	domain::sidecar::{content_uuid_of_file, ContentSidecar},
	ops::indexing::metadata::refuse_dataless,
};

/// The `version` of the shot list this build writes. A reader refuses a
/// newer one rather than guess at fields it does not know the meaning of.
pub const SHOTS_VERSION: u32 = 1;

/// A frame rate as a reduced fraction, such as 24000/1001.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct FrameRate {
	pub num: u32,
	pub den: u32,
}

impl FrameRate {
	/// Parse FFprobe's `num/den`, reduced. A zero part means no rate.
	pub fn parse(value: &str) -> Option<Self> {
		let (num, den) = value.trim().split_once('/')?;
		let (num, den): (u32, u32) = (num.parse().ok()?, den.parse().ok()?);
		if num == 0 || den == 0 {
			return None;
		}
		let divisor = gcd(num, den);
		Some(Self {
			num: num / divisor,
			den: den / divisor,
		})
	}

	pub fn fps(&self) -> f64 {
		self.num as f64 / self.den as f64
	}

	pub fn seconds(&self, frame: u64) -> f64 {
		frame as f64 * self.den as f64 / self.num as f64
	}

	/// Whole frames per timecode second: 24 for 23.976, 30 for 29.97.
	fn nominal(&self) -> u64 {
		(self.fps().round() as u64).max(1)
	}

	/// A frame as frame count, seconds, and timecode from `start`.
	pub fn point(&self, frame: u64, start: Option<&str>) -> FramePoint {
		FramePoint {
			frame,
			seconds: self.seconds(frame),
			timecode: self.timecode(frame, start),
		}
	}

	/// `HH:MM:SS:FF` of `frame` counted from `start`, the file's own start
	/// timecode. A start written with `;` is drop-frame, and so is the
	/// result. Without a start the count begins at 00:00:00:00.
	pub fn timecode(&self, frame: u64, start: Option<&str>) -> String {
		let fps = self.nominal();
		let start = start.and_then(|start| StartTimecode::parse(start, fps));
		let drop = match start {
			Some(start) if start.drop_frame && fps % 30 == 0 => fps / 15,
			_ => 0,
		};
		let mut total = start.map_or(0, |start| start.frames(fps, drop)) + frame;
		if drop > 0 {
			// SMPTE drop-frame skips frame numbers, not frames: the first
			// `drop` labels of every minute except each tenth.
			let per_ten = fps * 600 - drop * 9;
			let per_minute = fps * 60 - drop;
			let tens = total / per_ten;
			let rest = total % per_ten;
			total += drop * 9 * tens;
			if rest > drop {
				total += drop * ((rest - drop) / per_minute);
			}
		}
		let separator = if drop > 0 { ';' } else { ':' };
		format!(
			"{:02}:{:02}:{:02}{}{:02}",
			total / (fps * 3600) % 24,
			total / (fps * 60) % 60,
			total / fps % 60,
			separator,
			total % fps
		)
	}
}

#[derive(Debug, Clone, Copy)]
struct StartTimecode {
	hours: u64,
	minutes: u64,
	seconds: u64,
	frames: u64,
	drop_frame: bool,
}

impl StartTimecode {
	fn parse(value: &str, fps: u64) -> Option<Self> {
		let drop_frame = value.contains(';');
		let parts: Vec<u64> = value
			.split([':', ';', '.'])
			.map(|part| part.parse().ok())
			.collect::<Option<_>>()?;
		let [hours, minutes, seconds, frames] = parts[..] else {
			return None;
		};
		if minutes > 59 || seconds > 59 || frames >= fps {
			return None;
		}
		Some(Self {
			hours,
			minutes,
			seconds,
			frames,
			drop_frame,
		})
	}

	/// Frames since midnight.
	fn frames(&self, fps: u64, drop: u64) -> u64 {
		let minutes = self.hours * 60 + self.minutes;
		let labels = (minutes * 60 + self.seconds) * fps + self.frames;
		labels.saturating_sub(drop * (minutes - minutes / 10))
	}
}

fn gcd(mut a: u32, mut b: u32) -> u32 {
	while b != 0 {
		(a, b) = (b, a % b);
	}
	a
}

/// One frame as the frame count from the start of the file (the source of
/// truth), and as seconds and timecode for people.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
pub struct FramePoint {
	pub frame: u64,
	pub seconds: f64,
	pub timecode: String,
}

/// Which file the scene scores were read from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum AnalyzedMedia {
	Proxy,
	Original,
}

/// One continuous shot.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
pub struct Shot {
	/// UUID v5 of the content uuid and the in and out frames.
	pub id: Uuid,
	pub index: u32,
	/// The first frame of the shot.
	#[serde(rename = "in")]
	pub in_point: FramePoint,
	/// The first frame after the shot.
	#[serde(rename = "out")]
	pub out_point: FramePoint,
	pub duration_frames: u64,
	pub duration_seconds: f64,
	/// The frame the keyframe image shows, the middle of the shot.
	pub keyframe: FramePoint,
	/// Scene score of the cut that opens this shot, 0 for the first shot.
	pub cut_score: f32,
}

impl Shot {
	pub fn id_for(content_uuid: Uuid, in_frame: u64, out_frame: u64) -> Uuid {
		Uuid::new_v5(
			&content_uuid,
			format!("shot:{in_frame}:{out_frame}").as_bytes(),
		)
	}
}

/// Every shot of one video, in order, with no gaps.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
pub struct ShotList {
	pub version: u32,
	pub content_uuid: Uuid,
	pub analyzed: AnalyzedMedia,
	/// The original's frame rate. Every frame number counts at this rate.
	pub rate: FrameRate,
	/// The original's embedded start timecode, when it has one.
	pub start_timecode: Option<String>,
	/// Always true: an out point is the first frame after the range.
	pub out_exclusive: bool,
	pub duration: FramePoint,
	pub threshold: f64,
	pub min_shot_seconds: f64,
	pub shots: Vec<Shot>,
}

impl ShotList {
	/// The keyframe image of a shot, under a library directory.
	pub fn keyframe_path(&self, library_path: &Path, shot: &Shot) -> PathBuf {
		ContentSidecar::keyframe(self.content_uuid, shot.id).path_in(library_path)
	}

	/// The shot that holds `frame`.
	pub fn shot_at(&self, frame: u64) -> Option<&Shot> {
		let index = self
			.shots
			.partition_point(|shot| shot.out_point.frame <= frame);
		self.shots
			.get(index)
			.filter(|shot| shot.in_point.frame <= frame)
	}
}

/// The shot list of a content uuid under a library directory, or `None`
/// when shot detection has not run for it.
pub async fn read_shots(library_path: &Path, content_uuid: Uuid) -> io::Result<Option<ShotList>> {
	let path = ContentSidecar::shots(content_uuid).path_in(library_path);
	let bytes = match tokio::fs::read(&path).await {
		Ok(bytes) => bytes,
		Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
		Err(error) => return Err(error),
	};
	let list: ShotList = serde_json::from_slice(&bytes)
		.map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
	if list.version > SHOTS_VERSION {
		return Err(io::Error::new(
			io::ErrorKind::InvalidData,
			format!(
				"shot list version {} is newer than this build reads",
				list.version
			),
		));
	}
	Ok(Some(list))
}

/// The shot list of a local file, found by its candidate content uuid.
pub async fn shots_of_file(library_path: &Path, file: &Path) -> io::Result<Option<ShotList>> {
	read_shots(library_path, content_uuid_of_video(file).await?).await
}

/// The candidate content uuid of a local video. A cloud placeholder is
/// refused rather than hashed, since hashing would download it.
pub async fn content_uuid_of_video(file: &Path) -> io::Result<Uuid> {
	refuse_dataless(file).await?;
	content_uuid_of_file(file)
		.await
		.map_err(|error| io::Error::other(error.to_string()))
}

/// Write the shot list through a partial file, so a reader never sees half
/// of one.
pub(super) async fn write_shots(library_path: &Path, list: &ShotList) -> io::Result<()> {
	let path = ContentSidecar::shots(list.content_uuid).path_in(library_path);
	let partial = partial_path(&path);
	if let Some(parent) = path.parent() {
		tokio::fs::create_dir_all(parent).await?;
	}
	let bytes = serde_json::to_vec_pretty(list).map_err(io::Error::other)?;
	tokio::fs::write(&partial, bytes).await?;
	tokio::fs::rename(&partial, &path).await
}

/// `.<variant>.partial.<ext>` beside `path`, the name the media sidecar
/// rules reserve for a file being written.
pub(super) fn partial_path(path: &Path) -> PathBuf {
	let stem = path
		.file_stem()
		.map(|stem| stem.to_string_lossy().into_owned())
		.unwrap_or_default();
	let extension = path
		.extension()
		.map(|extension| extension.to_string_lossy().into_owned())
		.unwrap_or_default();
	path.with_file_name(format!(".{stem}.partial.{extension}"))
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn rates_reduce() {
		assert_eq!(
			FrameRate::parse("48000/2002"),
			Some(FrameRate {
				num: 24000,
				den: 1001
			})
		);
		assert_eq!(FrameRate::parse("0/0"), None);
	}

	#[test]
	fn timecode_counts_from_the_start_timecode() {
		let rate = FrameRate::parse("24000/1001").unwrap();
		assert_eq!(rate.timecode(0, None), "00:00:00:00");
		assert_eq!(rate.timecode(1230, None), "00:00:51:06");
		assert_eq!(rate.timecode(1230, Some("14:01:20:00")), "14:02:11:06");
	}

	#[test]
	fn drop_frame_skips_labels_but_not_frames() {
		let rate = FrameRate::parse("30000/1001").unwrap();
		assert_eq!(rate.timecode(1799, Some("00:00:00;00")), "00:00:59;29");
		assert_eq!(rate.timecode(1800, Some("00:00:00;00")), "00:01:00;02");
		assert_eq!(rate.timecode(17982, Some("00:00:00;00")), "00:10:00;00");
		assert_eq!(rate.timecode(0, Some("01:00:00;00")), "01:00:00;00");
		assert_eq!(rate.timecode(1800, Some("00:00:00:00")), "00:01:00:00");
	}

	#[test]
	fn shot_ids_follow_boundaries() {
		let content = Uuid::new_v4();
		assert_eq!(Shot::id_for(content, 0, 96), Shot::id_for(content, 0, 96));
		assert_ne!(Shot::id_for(content, 0, 96), Shot::id_for(content, 0, 97));
		assert_ne!(
			Shot::id_for(content, 0, 96),
			Shot::id_for(Uuid::new_v4(), 0, 96)
		);
	}

	#[test]
	fn shot_at_finds_the_holding_shot() {
		let content = Uuid::new_v4();
		let rate = FrameRate { num: 24, den: 1 };
		let shot = |index: u32, a: u64, b: u64| Shot {
			id: Shot::id_for(content, a, b),
			index,
			in_point: rate.point(a, None),
			out_point: rate.point(b, None),
			duration_frames: b - a,
			duration_seconds: rate.seconds(b - a),
			keyframe: rate.point((a + b) / 2, None),
			cut_score: 0.0,
		};
		let list = ShotList {
			version: SHOTS_VERSION,
			content_uuid: content,
			analyzed: AnalyzedMedia::Original,
			rate,
			start_timecode: None,
			out_exclusive: true,
			duration: rate.point(100, None),
			threshold: 0.3,
			min_shot_seconds: 1.0,
			shots: vec![shot(0, 0, 40), shot(1, 40, 100)],
		};
		assert_eq!(list.shot_at(39).map(|shot| shot.index), Some(0));
		assert_eq!(list.shot_at(40).map(|shot| shot.index), Some(1));
		assert_eq!(list.shot_at(100), None);
	}

	#[test]
	fn partial_names_follow_the_sidecar_rule() {
		assert_eq!(
			partial_path(Path::new("/s/shots/default.json")),
			PathBuf::from("/s/shots/.default.partial.json")
		);
	}
}
