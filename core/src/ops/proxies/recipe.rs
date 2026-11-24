//! Proxy and speech audio recipes, and an interruptible runner.

use std::{
	path::{Path, PathBuf},
	sync::{
		atomic::{AtomicBool, AtomicU64, Ordering},
		Arc,
	},
	time::Duration,
};

use sd_ffmpeg::footage::{AudioEncode, Transcode, VideoEncode};
use serde::Deserialize;
use serde_json::Value;

use crate::{
	domain::ProxyResolution,
	infra::job::prelude::{JobContext, JobResult},
};

const PROBE_TIMEOUT: Duration = Duration::from_secs(30);

/// What a video holds, from one probe.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct Probe {
	pub has_video: bool,
	pub has_audio: bool,
	pub duration: Option<f64>,
	/// Start timecode, from a tmcd track, the video stream or the container.
	pub timecode: Option<String>,
}

#[derive(Deserialize)]
struct ProbeOutput {
	#[serde(default)]
	streams: Vec<ProbeStream>,
	format: Option<ProbeFormat>,
}

#[derive(Deserialize)]
struct ProbeStream {
	codec_type: Option<String>,
	#[serde(default)]
	tags: ProbeTags,
}

#[derive(Deserialize)]
struct ProbeFormat {
	duration: Option<String>,
	#[serde(default)]
	tags: ProbeTags,
}

#[derive(Deserialize, Default)]
struct ProbeTags {
	timecode: Option<String>,
}

pub(super) fn probe(input: &Path) -> Result<Probe, String> {
	let report = sd_ffmpeg::footage::probe(input, PROBE_TIMEOUT)
		.map_err(|error| format!("could not read the video: {error}"))?;
	parse_probe(report)
}

fn parse_probe(report: Value) -> Result<Probe, String> {
	let parsed: ProbeOutput =
		serde_json::from_value(report).map_err(|error| format!("parse the probe: {error}"))?;
	let has = |kind: &str| {
		parsed
			.streams
			.iter()
			.any(|stream| stream.codec_type.as_deref() == Some(kind))
	};
	let stream_timecode = |kind: &str| {
		parsed
			.streams
			.iter()
			.filter(|stream| stream.codec_type.as_deref() == Some(kind))
			.find_map(|stream| stream.tags.timecode.clone())
	};
	let timecode = stream_timecode("data")
		.or_else(|| stream_timecode("video"))
		.or_else(|| {
			parsed
				.format
				.as_ref()
				.and_then(|format| format.tags.timecode.clone())
		});
	Ok(Probe {
		has_video: has("video"),
		has_audio: has("audio"),
		timecode,
		duration: parsed
			.format
			.and_then(|format| format.duration)
			.and_then(|duration| duration.parse::<f64>().ok())
			.filter(|duration| duration.is_finite() && *duration > 0.0),
	})
}

/// H.264 at the requested short side, never upscaled, with a keyframe every
/// second so a seek or a clip cut needs no re-encode.
///
/// Frame timing passes through: no rate conversion, so frame n of the proxy
/// is frame n of the original at the same time. Shot lists and timelines are
/// computed on the proxy and applied to the original, and a muxer left to
/// make the rate constant duplicates frames of a variable-rate source. The
/// start timecode is written as a tmcd track and a tag, so an editor that
/// relinks to the proxy shows the original's timecodes.
pub(super) fn proxy(resolution: ProxyResolution, timecode: Option<&str>) -> Transcode {
	let side = resolution.short_side();
	// Autorotation runs before this filter, so iw and ih are the displayed
	// orientation and a portrait clip keeps its short side at `side`.
	let filter = format!(
		"scale='if(gte(iw,ih),-2,min({side},trunc(iw/2)*2))':'if(gte(iw,ih),min({side},trunc(ih/2)*2),-2)',format=yuv420p"
	);
	let pairs = |pairs: &[(&str, &str)]| -> Vec<(String, String)> {
		pairs
			.iter()
			.map(|(key, value)| (key.to_string(), value.to_string()))
			.collect()
	};
	let mut muxer_options = pairs(&[("movflags", "+faststart")]);
	let mut metadata = Vec::new();
	if let Some(timecode) = timecode {
		metadata.push(("timecode".to_string(), timecode.to_string()));
		muxer_options.extend(pairs(&[("write_tmcd", "on")]));
	}
	Transcode {
		format: "mp4".into(),
		video: Some(VideoEncode {
			filter,
			codec: "libx264".into(),
			options: pairs(&[("preset", "veryfast"), ("crf", "23"), ("profile", "high")]),
			keyframe_seconds: Some(1),
		}),
		audio: Some(AudioEncode {
			codec: "aac".into(),
			bit_rate: Some(128_000),
			channels: 2,
			sample_rate: None,
		}),
		metadata,
		muxer_options,
	}
}

/// The first audio track as 16 kHz mono 16-bit PCM, what speech models read.
pub(super) fn speech_audio() -> Transcode {
	Transcode {
		format: "wav".into(),
		video: None,
		audio: Some(AudioEncode {
			codec: "pcm_s16le".into(),
			bit_rate: None,
			channels: 1,
			sample_rate: Some(16_000),
		}),
		metadata: Vec::new(),
		muxer_options: Vec::new(),
	}
}

/// Encode until done, checking for pause and cancel as it goes.
///
/// A 4K file takes minutes, so waiting for the encode before honouring an
/// interrupt would hold a pause that long. On interrupt the encode stops
/// and the partial output stays behind, to be overwritten when the job
/// resumes. `on_progress` receives the seconds of output written so far.
pub(super) async fn run(
	ctx: &JobContext<'_>,
	input: &Path,
	output: &Path,
	recipe: Transcode,
	mut on_progress: impl FnMut(f64),
) -> JobResult<Result<(), String>> {
	let cancel = Arc::new(AtomicBool::new(false));
	let written_us = Arc::new(AtomicU64::new(0));
	let mut work = tokio::task::spawn_blocking({
		let (input, output): (PathBuf, PathBuf) = (input.into(), output.into());
		let (cancel, written_us) = (cancel.clone(), written_us.clone());
		move || sd_ffmpeg::footage::transcode(&input, &output, &recipe, &cancel, &written_us)
	});
	let mut tick = tokio::time::interval(Duration::from_millis(500));
	let result = loop {
		tokio::select! {
			done = &mut work => break done,
			_ = tick.tick() => {
				if let Err(interrupt) = ctx.check_interrupt().await {
					cancel.store(true, Ordering::Relaxed);
					let _ = work.await;
					return Err(interrupt);
				}
				on_progress(written_us.load(Ordering::Relaxed) as f64 / 1_000_000.0);
			}
		}
	};
	Ok(match result {
		Ok(Ok(())) => Ok(()),
		Ok(Err(error)) => Err(format!("encode failed: {error}")),
		Err(error) => Err(format!("encoder worker failed: {error}")),
	})
}

#[cfg(test)]
mod tests {
	use super::*;
	use serde_json::json;
	use std::sync::atomic::AtomicBool;

	#[test]
	fn probe_reads_streams_duration_and_timecode() {
		let probe = parse_probe(json!({
			"streams": [
				{ "codec_type": "video", "tags": { "timecode": "01:00:00:00" } },
				{ "codec_type": "audio" },
				{ "codec_type": "data", "tags": { "timecode": "14:02:11;06" } }
			],
			"format": { "duration": "40.04" }
		}))
		.unwrap();
		assert_eq!(
			probe,
			Probe {
				has_video: true,
				has_audio: true,
				duration: Some(40.04),
				timecode: Some("14:02:11;06".into()),
			}
		);
		let silent =
			parse_probe(json!({ "streams": [{ "codec_type": "video" }], "format": {} })).unwrap();
		assert!(!silent.has_audio);
		assert_eq!(silent.duration, None);
		assert_eq!(silent.timecode, None);
	}

	#[test]
	fn proxy_recipe_passes_timing_through() {
		let recipe = proxy(ProxyResolution::P1080, Some("01:00:00:00"));
		let video = recipe.video.as_ref().unwrap();
		assert_eq!(video.keyframe_seconds, Some(1));
		assert_eq!(video.codec, "libx264");
		assert!(video.filter.contains("min(1080,"));
		assert!(video.filter.ends_with(",format=yuv420p"));
		let option = |key: &str| {
			video
				.options
				.iter()
				.find(|(name, _)| name == key)
				.map(|(_, value)| value.as_str())
		};
		assert_eq!(option("preset"), Some("veryfast"));
		assert_eq!(option("crf"), Some("23"));
		assert_eq!(option("profile"), Some("high"));
		assert_eq!(recipe.format, "mp4");
		assert_eq!(
			recipe.metadata,
			vec![("timecode".to_string(), "01:00:00:00".to_string())]
		);
		assert!(recipe
			.muxer_options
			.contains(&("write_tmcd".to_string(), "on".to_string())));
		assert!(recipe
			.muxer_options
			.contains(&("movflags".to_string(), "+faststart".to_string())));
		let audio = recipe.audio.as_ref().unwrap();
		assert_eq!(
			(audio.codec.as_str(), audio.bit_rate, audio.channels),
			("aac", Some(128_000), 2)
		);
		let untimed = proxy(ProxyResolution::P720, None);
		assert!(untimed.metadata.is_empty());
		assert!(!untimed
			.muxer_options
			.iter()
			.any(|(name, _)| name == "write_tmcd"));
	}

	fn transcode(input: &Path, output: &Path, recipe: &Transcode) {
		sd_ffmpeg::footage::transcode(
			input,
			output,
			recipe,
			&AtomicBool::new(false),
			&AtomicU64::new(0),
		)
		.unwrap();
	}

	/// Frame count and the first and last presentation times, in seconds.
	fn frame_times(path: &Path) -> (usize, f64, f64) {
		let times: Vec<f64> = sd_ffmpeg::footage::scene_scores(
			path,
			"select='gte(scene,0)'",
			&AtomicBool::new(false),
		)
		.unwrap()
		.iter()
		.map(|frame| frame.time)
		.collect();
		(times.len(), times[0], *times.last().unwrap())
	}

	fn render(source: &str, output: &Path, timecode: Option<&str>) {
		let options = [("preset".to_string(), "ultrafast".to_string())];
		let metadata: Vec<(String, String)> = timecode
			.map(|timecode| ("timecode".to_string(), timecode.to_string()))
			.into_iter()
			.collect();
		let format = if output.extension().is_some_and(|ext| ext == "mkv") {
			"matroska"
		} else {
			"mov"
		};
		sd_ffmpeg::footage::render(source, "libx264", &options, format, &metadata, output).unwrap();
	}

	/// A 29.97 fps source and a variable-rate one keep every frame at its
	/// time, and a start timecode survives as a tmcd track and a tag. Runs
	/// on the linked libraries, with no `ffmpeg` executable involved.
	#[test]
	fn proxy_keeps_every_frame_at_its_original_time_and_timecode() {
		let dir = tempfile::tempdir().unwrap();
		let ntsc = dir.path().join("ntsc.mov");
		let vfr = dir.path().join("vfr.mkv");
		render(
			"testsrc2=size=320x180:rate=30000/1001:duration=4",
			&ntsc,
			Some("14:02:11;06"),
		);
		// Two gaps in a 30 fps run, which a constant-rate muxer would fill
		// with duplicates.
		render(
			"testsrc2=size=320x180:rate=30:duration=4,settb=1/1000,setpts='N/(30*TB)+if(gte(N,30),0.37/TB,0)+if(gte(N,60),0.21/TB,0)'",
			&vfr,
			None,
		);
		assert_eq!(
			probe(&ntsc).unwrap().timecode.as_deref(),
			Some("14:02:11;06")
		);
		for source in [&ntsc, &vfr] {
			let output = source.with_extension("proxy.mp4");
			let original = probe(source).unwrap();
			transcode(
				source,
				&output,
				&proxy(ProxyResolution::P720, original.timecode.as_deref()),
			);
			let (frames, first, last) = frame_times(source);
			let (proxy_frames, proxy_first, proxy_last) = frame_times(&output);
			assert_eq!(proxy_frames, frames, "{}", source.display());
			assert_eq!(proxy_first, 0.0, "{}", source.display());
			assert_eq!(first, 0.0, "{}", source.display());
			// Matroska stores milliseconds, so its own times are rounded.
			assert!(
				(proxy_last - last).abs() < 0.002,
				"{}: {last} vs {proxy_last}",
				source.display()
			);
			assert_eq!(probe(&output).unwrap().timecode, original.timecode);
		}
		let streams =
			sd_ffmpeg::footage::probe(&ntsc.with_extension("proxy.mp4"), PROBE_TIMEOUT).unwrap();
		assert!(
			streams["streams"]
				.as_array()
				.unwrap()
				.iter()
				.any(|stream| stream["codec_type"] == "data"
					&& stream["tags"]["timecode"] == "14:02:11;06"),
			"{streams}"
		);
	}

	/// The speech recipe writes 16 kHz mono PCM, and a file without sound
	/// is refused rather than written empty.
	#[test]
	fn speech_audio_is_16_khz_mono() {
		let dir = tempfile::tempdir().unwrap();
		let silent = dir.path().join("silent.mov");
		render("testsrc2=size=160x90:rate=25:duration=1", &silent, None);
		assert!(sd_ffmpeg::footage::transcode(
			&silent,
			&dir.path().join("none.wav"),
			&speech_audio(),
			&AtomicBool::new(false),
			&AtomicU64::new(0),
		)
		.is_err());

		let tone = dir.path().join("tone.wav");
		sd_ffmpeg::footage::render(
			"sine=frequency=440:sample_rate=48000:duration=2",
			"pcm_s16le",
			&[],
			"wav",
			&[],
			&tone,
		)
		.unwrap();
		let speech = dir.path().join("speech.wav");
		transcode(&tone, &speech, &speech_audio());
		let report = sd_ffmpeg::footage::probe(&speech, PROBE_TIMEOUT).unwrap();
		let stream = &report["streams"][0];
		assert_eq!(stream["sample_rate"], "16000");
		assert_eq!(stream["channels"], 1);
		let duration: f64 = report["format"]["duration"]
			.as_str()
			.unwrap()
			.parse()
			.unwrap();
		assert!((duration - 2.0).abs() < 0.01, "{duration}");
	}
}
