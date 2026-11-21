//! FFmpeg recipes for the proxy and speech audio, and an interruptible runner.

use std::{
	ffi::OsString,
	path::Path,
	process::Stdio,
	sync::{
		atomic::{AtomicU64, Ordering},
		Arc,
	},
	time::Duration,
};

use serde::Deserialize;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, BufReader};

use crate::{
	domain::ProxyResolution,
	infra::job::prelude::{JobContext, JobResult},
	service::external_tools::ExternalTools,
};

const STDERR_LIMIT: usize = 64 * 1024;

/// What a video holds, from one FFprobe call.
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

pub(super) fn probe(tools: &ExternalTools, input: &Path) -> Result<Probe, String> {
	let args: Vec<OsString> = vec![
		"-v".into(),
		"error".into(),
		"-show_entries".into(),
		"stream=codec_type:stream_tags=timecode:format=duration:format_tags=timecode".into(),
		"-of".into(),
		"json".into(),
		input.as_os_str().to_owned(),
	];
	let output = tools
		.run_ffprobe(&args, Duration::from_secs(30), 1024 * 1024)
		.map_err(|error| error.to_string())?;
	if !output.status.success() {
		return Err(failure("FFprobe could not read the video", &output.stderr));
	}
	parse_probe(&output.stdout)
}

fn parse_probe(stdout: &[u8]) -> Result<Probe, String> {
	let parsed: ProbeOutput =
		serde_json::from_slice(stdout).map_err(|error| format!("parse FFprobe output: {error}"))?;
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
/// computed on the proxy and applied to the original, and an MP4 muxer left
/// to its default duplicates frames of a variable-rate source to make it
/// constant. The start timecode is written as a tmcd track and a tag, so an
/// editor that relinks to the proxy shows the original's timecodes.
pub(super) fn proxy_args(
	input: &Path,
	output: &Path,
	resolution: ProxyResolution,
	timecode: Option<&str>,
) -> Vec<OsString> {
	let side = resolution.short_side();
	// Autorotation runs before this filter, so iw and ih are the displayed
	// orientation and a portrait clip keeps its short side at `side`.
	let scale = format!(
		"scale='if(gte(iw,ih),-2,min({side},trunc(iw/2)*2))':'if(gte(iw,ih),min({side},trunc(ih/2)*2),-2)',format=yuv420p"
	);
	let mut args = common_args(input);
	args.extend(
		[
			"-map",
			"0:v:0",
			"-map",
			"0:a:0?",
			"-sn",
			"-dn",
			"-map_metadata",
			"-1",
			"-vf",
			&scale,
			"-fps_mode",
			"passthrough",
			"-c:v",
			"libx264",
			"-preset",
			"veryfast",
			"-crf",
			"23",
			"-profile:v",
			"high",
			"-force_key_frames",
			"expr:gte(t,n_forced*1)",
			"-c:a",
			"aac",
			"-b:a",
			"128k",
			"-ac",
			"2",
			"-movflags",
			"+faststart",
			"-f",
			"mp4",
		]
		.map(OsString::from),
	);
	if let Some(timecode) = timecode {
		args.extend(["-timecode", timecode, "-write_tmcd", "on"].map(OsString::from));
	}
	args.push(output.as_os_str().to_owned());
	args
}

/// The first audio track as 16 kHz mono 16-bit PCM, what speech models read.
pub(super) fn speech_audio_args(input: &Path, output: &Path) -> Vec<OsString> {
	let mut args = common_args(input);
	args.extend(
		[
			"-map",
			"0:a:0",
			"-vn",
			"-sn",
			"-dn",
			"-map_metadata",
			"-1",
			"-ac",
			"1",
			"-ar",
			"16000",
			"-c:a",
			"pcm_s16le",
			"-f",
			"wav",
		]
		.map(OsString::from),
	);
	args.push(output.as_os_str().to_owned());
	args
}

fn common_args(input: &Path) -> Vec<OsString> {
	let mut args: Vec<OsString> = [
		"-hide_banner",
		"-loglevel",
		"error",
		"-nostdin",
		"-nostats",
		"-y",
		"-progress",
		"pipe:1",
		"-i",
	]
	.map(OsString::from)
	.to_vec();
	args.push(input.as_os_str().to_owned());
	args
}

/// Run FFmpeg until it exits, checking for pause and cancel as it goes.
///
/// A 4K file takes minutes, so waiting for the process before honouring an
/// interrupt would hold a pause that long. On interrupt the process is killed
/// and the partial output stays behind, to be overwritten when the job
/// resumes. `on_progress` receives the seconds of output written so far.
pub(super) async fn run_ffmpeg(
	ctx: &JobContext<'_>,
	ffmpeg: &Path,
	args: &[OsString],
	mut on_progress: impl FnMut(f64),
) -> JobResult<Result<(), String>> {
	let mut child = match tokio::process::Command::new(ffmpeg)
		.args(args)
		.stdin(Stdio::null())
		.stdout(Stdio::piped())
		.stderr(Stdio::piped())
		.kill_on_drop(true)
		.spawn()
	{
		Ok(child) => child,
		Err(error) => return Ok(Err(format!("start FFmpeg: {error}"))),
	};

	let written_us = Arc::new(AtomicU64::new(0));
	let stdout = child.stdout.take().expect("stdout is piped");
	let progress = {
		let written_us = written_us.clone();
		tokio::spawn(async move {
			let mut lines = BufReader::new(stdout).lines();
			while let Ok(Some(line)) = lines.next_line().await {
				if let Some(value) = line.strip_prefix("out_time_us=") {
					if let Ok(value) = value.trim().parse::<u64>() {
						written_us.store(value, Ordering::Relaxed);
					}
				}
			}
		})
	};
	let stderr = child.stderr.take().expect("stderr is piped");
	let errors = tokio::spawn(async move {
		let mut buffer = Vec::new();
		let _ = stderr
			.take(STDERR_LIMIT as u64)
			.read_to_end(&mut buffer)
			.await;
		String::from_utf8_lossy(&buffer).into_owned()
	});

	let mut tick = tokio::time::interval(Duration::from_millis(500));
	let status = loop {
		tokio::select! {
			status = child.wait() => break status,
			_ = tick.tick() => {
				if let Err(interrupt) = ctx.check_interrupt().await {
					let _ = child.kill().await;
					progress.abort();
					errors.abort();
					return Err(interrupt);
				}
				on_progress(written_us.load(Ordering::Relaxed) as f64 / 1_000_000.0);
			}
		}
	};
	let _ = progress.await;
	let stderr = errors.await.unwrap_or_default();
	Ok(match status {
		Ok(status) if status.success() => Ok(()),
		Ok(_) => Err(failure("FFmpeg failed", &stderr)),
		Err(error) => Err(format!("wait for FFmpeg: {error}")),
	})
}

fn failure(context: &str, stderr: &str) -> String {
	match stderr.lines().map(str::trim).find(|line| !line.is_empty()) {
		Some(reason) => format!("{context}: {reason}"),
		None => context.to_string(),
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn probe_reads_streams_duration_and_timecode() {
		let probe = parse_probe(
			br#"{"streams":[{"codec_type":"video","tags":{"timecode":"01:00:00:00"}},{"codec_type":"audio"},{"codec_type":"data","tags":{"timecode":"14:02:11;06"}}],"format":{"duration":"40.04"}}"#,
		)
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
		let silent = parse_probe(br#"{"streams":[{"codec_type":"video"}],"format":{}}"#).unwrap();
		assert!(!silent.has_audio);
		assert_eq!(silent.duration, None);
		assert_eq!(silent.timecode, None);
	}

	#[test]
	fn proxy_recipe_passes_timing_through() {
		let args = proxy_args(
			Path::new("/in.mov"),
			Path::new("/out.mp4"),
			ProxyResolution::P1080,
			Some("01:00:00:00"),
		);
		let args: Vec<_> = args.iter().map(|arg| arg.to_string_lossy()).collect();
		let after = |flag: &str| args[args.iter().position(|arg| arg == flag).unwrap() + 1].clone();
		assert_eq!(after("-force_key_frames"), "expr:gte(t,n_forced*1)");
		assert_eq!(after("-fps_mode"), "passthrough");
		assert!(!args.iter().any(|arg| arg == "-r"));
		assert_eq!(after("-c:v"), "libx264");
		assert!(after("-vf").contains("min(1080,"));
		assert_eq!(after("-timecode"), "01:00:00:00");
		assert_eq!(after("-write_tmcd"), "on");
		assert_eq!(args.last().unwrap(), "/out.mp4");
		let untimed = proxy_args(
			Path::new("/in.mov"),
			Path::new("/out.mp4"),
			ProxyResolution::P720,
			None,
		);
		assert!(!untimed.iter().any(|arg| arg == "-timecode"));
	}

	fn ffmpeg(tools: &ExternalTools, args: &[&str]) {
		let args: Vec<OsString> = args.iter().map(OsString::from).collect();
		let output = tools
			.run_ffmpeg(&args, Duration::from_secs(120), 1024 * 1024)
			.unwrap();
		assert!(output.status.success(), "{}", output.stderr);
	}

	fn ffprobe(tools: &ExternalTools, path: &Path, query: &[&str]) -> String {
		let args: Vec<OsString> = ["-v", "error"]
			.iter()
			.chain(query)
			.map(OsString::from)
			.chain([path.as_os_str().to_owned()])
			.collect();
		let output = tools
			.run_ffprobe(&args, Duration::from_secs(60), 16 * 1024 * 1024)
			.unwrap();
		String::from_utf8_lossy(&output.stdout).into_owned()
	}

	/// Frame count and the first and last presentation times, in seconds.
	fn frame_times(tools: &ExternalTools, path: &Path) -> (usize, f64, f64) {
		let times: Vec<f64> = ffprobe(
			tools,
			path,
			&[
				"-select_streams",
				"v:0",
				"-show_entries",
				"frame=pts_time",
				"-of",
				"csv=p=0",
			],
		)
		.lines()
		.filter_map(|line| line.trim().trim_end_matches(',').parse().ok())
		.collect();
		(times.len(), times[0], *times.last().unwrap())
	}

	/// A 29.97 fps source and a variable-rate one keep every frame at its
	/// time, and a start timecode survives as a tmcd track and a tag.
	#[test]
	fn proxy_keeps_every_frame_at_its_original_time_and_timecode() {
		let tools = ExternalTools::new();
		if tools.ffmpeg_path().is_none() || tools.ffprobe_path().is_none() {
			eprintln!("skipped: FFmpeg is not installed");
			return;
		}
		let dir = tempfile::tempdir().unwrap();
		let ntsc = dir.path().join("ntsc.mov");
		let vfr = dir.path().join("vfr.mkv");
		ffmpeg(
			&tools,
			&[
				"-v",
				"error",
				"-y",
				"-f",
				"lavfi",
				"-i",
				"testsrc2=size=320x180:rate=30000/1001:duration=4",
				"-c:v",
				"libx264",
				"-preset",
				"ultrafast",
				"-timecode",
				"14:02:11;06",
				ntsc.to_str().unwrap(),
			],
		);
		// Two gaps in a 30 fps run, which an MP4 muxer would fill with duplicates.
		ffmpeg(
			&tools,
			&[
				"-v",
				"error",
				"-y",
				"-f",
				"lavfi",
				"-i",
				"testsrc2=size=320x180:rate=30:duration=4",
				"-vf",
				"setpts='N/(30*TB)+if(gte(N,30),0.37/TB,0)+if(gte(N,60),0.21/TB,0)'",
				"-fps_mode",
				"passthrough",
				"-c:v",
				"libx264",
				"-preset",
				"ultrafast",
				vfr.to_str().unwrap(),
			],
		);
		assert_eq!(
			probe(&tools, &ntsc).unwrap().timecode.as_deref(),
			Some("14:02:11;06")
		);
		for source in [&ntsc, &vfr] {
			let proxy = source.with_extension("proxy.mp4");
			let original = probe(&tools, source).unwrap();
			let args: Vec<String> = proxy_args(
				source,
				&proxy,
				ProxyResolution::P720,
				original.timecode.as_deref(),
			)
			.iter()
			.map(|arg| arg.to_string_lossy().into_owned())
			.collect();
			let args: Vec<&str> = args.iter().map(String::as_str).collect();
			ffmpeg(&tools, &args);
			let (frames, first, last) = frame_times(&tools, source);
			let (proxy_frames, proxy_first, proxy_last) = frame_times(&tools, &proxy);
			assert_eq!(proxy_frames, frames, "{}", source.display());
			assert_eq!(proxy_first, 0.0, "{}", source.display());
			assert_eq!(first, 0.0, "{}", source.display());
			// Matroska stores milliseconds, so its own times are rounded.
			assert!(
				(proxy_last - last).abs() < 0.002,
				"{}: {last} vs {proxy_last}",
				source.display()
			);
			assert_eq!(probe(&tools, &proxy).unwrap().timecode, original.timecode);
		}
		let tags = ffprobe(
			&tools,
			&ntsc.with_extension("proxy.mp4"),
			&["-show_entries", "stream=codec_tag_string", "-of", "csv=p=0"],
		);
		assert!(tags.lines().any(|line| line.trim() == "tmcd"), "{tags}");
	}
}
