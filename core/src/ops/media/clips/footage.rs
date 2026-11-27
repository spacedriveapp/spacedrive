//! What one video file is, as the clip ops need it.

use std::{path::Path, time::Duration};

use sd_timeline::{timecode, Audio, Rate, Rounding};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use specta::Type;
use uuid::Uuid;

use crate::{
	context::CoreContext,
	domain::{
		sidecar::{ContentSidecar, ProxyResolution},
		SdPath,
	},
	library::Library,
	ops::{
		indexing::metadata::{refuse_dataless, DATALESS_REASON},
		media::shots::{content_uuid_of_video, read_shots, ShotList},
		transcription::transcript::{read_transcript, Transcript},
	},
	service::sidecars::SidecarRegistry,
};

/// A time a caller hands in: a frame at the file's rate, or seconds, which
/// round to the nearest frame. A frame wins when both are present, so a clip
/// read back from `media.clips.search` can be passed on as it is.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize, Type)]
pub struct TimeInput {
	#[serde(default)]
	pub frame: Option<i64>,
	#[serde(default)]
	pub seconds: Option<f64>,
}

impl TimeInput {
	pub fn frame_at(&self, rate: Rate) -> Result<i64, String> {
		match (self.frame, self.seconds) {
			(Some(frame), _) => Ok(frame),
			(None, Some(seconds)) if seconds.is_finite() => {
				Ok(rate.frame_at(seconds, Rounding::Nearest))
			}
			_ => Err("a time needs a frame or a number of seconds".to_string()),
		}
	}

	pub fn seconds_at(&self, rate: Rate) -> Result<f64, String> {
		match (self.frame, self.seconds) {
			(Some(frame), _) => Ok(rate.seconds(frame)),
			(None, Some(seconds)) if seconds.is_finite() => Ok(seconds),
			_ => Err("a time needs a frame or a number of seconds".to_string()),
		}
	}
}

/// The picture and sound of a file, as FFprobe reports them.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Probe {
	pub rate: Rate,
	pub start_timecode: i64,
	pub drop_frame: bool,
	pub duration: i64,
	pub width: u32,
	pub height: u32,
	pub audio: Option<Audio>,
}

/// One local video and the sidecars made for its bytes.
pub(crate) struct Footage {
	pub content_uuid: Uuid,
	pub rate: Rate,
	pub start_timecode: i64,
	pub drop_frame: bool,
	/// Frames. Zero when nothing reported a length.
	pub duration: i64,
	pub shots: Option<ShotList>,
	pub transcript: Option<Transcript>,
	pub proxy: bool,
}

impl Footage {
	/// Reads the sidecars of `path`. The frame rate, start timecode and length
	/// come from the shot list when shot detection has run, since it probed
	/// the same bytes, and from FFprobe otherwise.
	pub async fn load(
		context: &CoreContext,
		library: &Library,
		path: &Path,
	) -> Result<Self, String> {
		let content_uuid = content_uuid_of_video(path)
			.await
			.map_err(|error| format!("could not read {}: {error}", display_name(path)))?;
		let finder = SidecarRegistry::of(library).finder();
		let shots = read_shots(&finder, content_uuid).await.map_err(|error| {
			format!(
				"could not read the shot list of {}: {error}",
				display_name(path)
			)
		})?;
		let transcript = read_transcript(&finder, content_uuid)
			.await
			.map_err(|error| {
				format!(
					"could not read the transcript of {}: {error}",
					display_name(path)
				)
			})?;
		let proxy = proxy_path(library, content_uuid).is_some();
		let facts = match shots.as_ref().and_then(timing_of_shots) {
			Some(facts) => facts,
			None => {
				let probe = probe(path).await?;
				(
					probe.rate,
					probe.start_timecode,
					probe.drop_frame,
					probe.duration,
				)
			}
		};
		let (rate, start_timecode, drop_frame, duration) = facts;
		Ok(Self {
			content_uuid,
			rate,
			start_timecode,
			drop_frame,
			duration,
			shots,
			transcript,
			proxy,
		})
	}

	pub fn point(&self, frame: i64) -> sd_timeline::TimePoint {
		self.rate.point(frame, self.start_timecode, self.drop_frame)
	}

	pub fn start_timecode_text(&self) -> String {
		timecode::format(self.start_timecode, self.rate, self.drop_frame)
	}

	/// The shot holding `frame`.
	pub fn shot_at(&self, frame: i64) -> Option<&crate::ops::media::shots::Shot> {
		self.shots.as_ref()?.shot_at(u64::try_from(frame).ok()?)
	}

	/// Transcript text said between two frames.
	pub fn text_between(&self, in_frame: i64, out_frame: i64) -> String {
		let Some(transcript) = &self.transcript else {
			return String::new();
		};
		let from_ms = (self.rate.seconds(in_frame) * 1000.0) as u64;
		let to_ms = (self.rate.seconds(out_frame) * 1000.0).ceil() as u64;
		transcript
			.segments
			.iter()
			.filter(|segment| segment.end_ms > from_ms && segment.start_ms < to_ms)
			.map(|segment| segment.text.trim())
			.filter(|text| !text.is_empty())
			.collect::<Vec<_>>()
			.join(" ")
	}
}

fn timing_of_shots(list: &ShotList) -> Option<(Rate, i64, bool, i64)> {
	let rate = Rate::new(list.rate.num, list.rate.den).ok()?;
	let (start, drop_frame) = match &list.start_timecode {
		Some(text) => timecode::parse(text, rate).ok()?,
		None => (0, false),
	};
	Some((
		rate,
		start,
		drop_frame,
		i64::try_from(list.duration.frame).ok()?,
	))
}

/// The 720p proxy of a content, wherever the library's sources keep it.
pub(crate) fn proxy_path(library: &Library, content_uuid: Uuid) -> Option<std::path::PathBuf> {
	SidecarRegistry::of(library)
		.finder()
		.find(&ContentSidecar::proxy(content_uuid, ProxyResolution::P720))
}

pub(crate) fn display_name(path: &Path) -> String {
	path.file_name()
		.map(|name| name.to_string_lossy().into_owned())
		.unwrap_or_else(|| path.display().to_string())
}

/// The path of a file on this device. Clips of another device's files would
/// reference paths that only exist there, so they are refused.
pub(crate) fn local_path(path: &SdPath) -> Result<std::path::PathBuf, String> {
	if let Some(local) = path.as_local_path() {
		return Ok(local.to_path_buf());
	}
	match path {
		SdPath::Physical { device_slug, .. } => Err(format!(
			"{device_slug} is another device; clips and timelines cover this device's files only"
		)),
		other => Err(format!("{other:?} does not name a local file")),
	}
}

/// The record uuid the source store gives `path`, when a source on this
/// device has walked it.
pub(crate) async fn record_uuid(context: &CoreContext, path: &SdPath) -> Option<Uuid> {
	let index = context.volume_index();
	for reach in
		crate::ops::paths::reach::stores_beneath_in(&context.volume_manager, index, path).await
	{
		if !reach.prefix.is_empty() {
			continue;
		}
		let db = index.read_store(reach.source.id).await?;
		if let Ok(Some(entry)) = sd_store::read::entry_by_path(db.pool(), &reach.scope).await {
			return Some(entry.uuid);
		}
	}
	None
}

const PROBE_TIMEOUT: Duration = Duration::from_secs(30);

pub(crate) async fn probe(path: &Path) -> Result<Probe, String> {
	// Opening a cloud placeholder would download the whole file.
	if refuse_dataless(path).await.is_err() {
		return Err(format!(
			"{} is a cloud placeholder ({DATALESS_REASON}); it is not read so it is not downloaded",
			display_name(path)
		));
	}
	let name = display_name(path);
	let path = path.to_path_buf();
	let json = tokio::task::spawn_blocking(move || sd_ffmpeg::footage::probe(&path, PROBE_TIMEOUT))
		.await
		.map_err(|_| "the probe did not finish".to_string())?
		.map_err(|error| format!("could not read {name}: {error}"))?;
	parse_probe(&json).map_err(|reason| format!("{name}: {reason}"))
}

/// The first video stream's rate and size, the first audio stream's format,
/// and the start timecode from whichever stream or the container carries it.
/// The average rate maps frame numbers onto the file's clock, so it is read
/// before the nominal one, as the shot detection job reads it.
pub(crate) fn parse_probe(json: &Value) -> Result<Probe, String> {
	let streams = json["streams"].as_array().cloned().unwrap_or_default();
	let video = streams
		.iter()
		.find(|stream| stream["codec_type"] == "video")
		.ok_or("the file has no video stream")?;
	let rate = [&video["avg_frame_rate"], &video["r_frame_rate"]]
		.into_iter()
		.find_map(|value| value.as_str().and_then(|text| Rate::parse(text).ok()))
		.ok_or("the video stream has no frame rate")?;
	let timecode_text = std::iter::once(&video["tags"]["timecode"])
		.chain(streams.iter().map(|stream| &stream["tags"]["timecode"]))
		.chain(std::iter::once(&json["format"]["tags"]["timecode"]))
		.find_map(Value::as_str);
	let (start_timecode, drop_frame) = match timecode_text {
		Some(text) => timecode::parse(text, rate).unwrap_or((0, false)),
		None => (0, false),
	};
	let frames = video["nb_frames"]
		.as_str()
		.and_then(|text| text.parse::<i64>().ok())
		.filter(|frames| *frames > 0);
	let duration = frames.unwrap_or_else(|| {
		json["format"]["duration"]
			.as_str()
			.and_then(|text| text.parse::<f64>().ok())
			.map_or(0, |seconds| rate.frame_at(seconds, Rounding::Floor))
	});
	let audio = streams
		.iter()
		.find(|stream| stream["codec_type"] == "audio")
		.map(|stream| Audio {
			sample_rate: stream["sample_rate"]
				.as_str()
				.and_then(|text| text.parse().ok())
				.unwrap_or(48000),
			channels: stream["channels"]
				.as_u64()
				.map_or(2, |channels| channels as u32),
		});
	Ok(Probe {
		rate,
		start_timecode,
		drop_frame,
		duration,
		width: video["width"].as_u64().unwrap_or(0) as u32,
		height: video["height"].as_u64().unwrap_or(0) as u32,
		audio,
	})
}

#[cfg(test)]
mod tests {
	use super::*;
	use serde_json::json;

	#[test]
	fn probe_reads_rate_timecode_and_sound() {
		let probe = parse_probe(&json!({
			"streams": [
				{ "codec_type": "video", "width": 1920, "height": 1080, "avg_frame_rate": "24000/1001", "r_frame_rate": "24000/1001", "nb_frames": "2400", "tags": { "timecode": "14:01:20:00" } },
				{ "codec_type": "audio", "sample_rate": "48000", "channels": 2 },
				{ "codec_type": "data", "tags": { "timecode": "14:01:20:00" } }
			],
			"format": { "duration": "100.100000" }
		}))
		.unwrap();
		assert_eq!(probe.rate, Rate::new(24000, 1001).unwrap());
		assert_eq!(probe.start_timecode, (14 * 3600 + 80) * 24);
		assert_eq!(probe.duration, 2400);
		assert_eq!((probe.width, probe.height), (1920, 1080));
		assert_eq!(
			probe.audio,
			Some(Audio {
				sample_rate: 48000,
				channels: 2
			})
		);
	}

	#[test]
	fn probe_falls_back_to_the_container_and_duration() {
		let probe = parse_probe(&json!({
			"streams": [
				{ "codec_type": "video", "width": 1280, "height": 720, "avg_frame_rate": "0/0", "r_frame_rate": "30000/1001" }
			],
			"format": { "duration": "10.010000", "tags": { "timecode": "01:00:00;00" } }
		}))
		.unwrap();
		assert_eq!(probe.rate, Rate::new(30000, 1001).unwrap());
		assert!(probe.drop_frame);
		assert_eq!(probe.start_timecode, 107892);
		assert_eq!(probe.duration, 300);
		assert_eq!(probe.audio, None);
	}

	#[test]
	fn a_frame_wins_over_seconds() {
		let rate = Rate::new(25, 1).unwrap();
		let both = TimeInput {
			frame: Some(10),
			seconds: Some(99.0),
		};
		assert_eq!(both.frame_at(rate).unwrap(), 10);
		let seconds = TimeInput {
			frame: None,
			seconds: Some(1.02),
		};
		assert_eq!(seconds.frame_at(rate).unwrap(), 26);
		assert!(TimeInput::default().frame_at(rate).is_err());
	}
}
