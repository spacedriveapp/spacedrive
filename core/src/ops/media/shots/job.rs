use std::{
	ffi::OsString,
	path::{Path, PathBuf},
	sync::Arc,
	time::{Duration, Instant},
};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use specta::Type;
use uuid::Uuid;

use super::{
	detect::{parse_scores, pick_cuts, DetectParams, FrameScore},
	sidecar::{partial_path, write_shots, AnalyzedMedia, FrameRate, Shot, ShotList, SHOTS_VERSION},
};
use crate::{
	domain::sidecar::{content_uuid_of_file, ContentSidecar, ProxyResolution},
	infra::job::{generic_progress::GenericProgress, prelude::*},
	service::external_tools::ExternalTools,
};

/// Height the scan decodes to. Scene scores barely change below 720p and
/// decoding gets several times faster.
const SCAN_HEIGHT: u32 = 270;
/// Upper bound on the scan's text output: two short lines per frame, so
/// about 30 hours of 60 fps video.
const SCAN_OUTPUT_LIMIT: usize = 512 * 1024 * 1024;
const PROBE_TIMEOUT: Duration = Duration::from_secs(30);
const KEYFRAME_TIMEOUT: Duration = Duration::from_secs(60);
const SMALL_OUTPUT_LIMIT: usize = 1024 * 1024;

#[derive(Debug, Serialize, Deserialize, Job)]
pub struct ShotDetectionJob {
	pub path: PathBuf,
	pub params: DetectParams,
	#[serde(skip, default = "Instant::now")]
	started_at: Instant,
}

impl ShotDetectionJob {
	pub fn new(path: PathBuf, params: DetectParams) -> Self {
		Self {
			path,
			params,
			started_at: Instant::now(),
		}
	}
}

impl Job for ShotDetectionJob {
	const NAME: &'static str = "shot_detection";
	const RESUMABLE: bool = false;
	const DESCRIPTION: Option<&'static str> = Some("Split a video into shots");
}

impl crate::infra::job::traits::DynJob for ShotDetectionJob {
	fn job_name(&self) -> &'static str {
		Self::NAME
	}
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct ShotDetectionOutput {
	pub content_uuid: Uuid,
	pub analyzed: AnalyzedMedia,
	pub shots: u32,
	/// Keyframes extracted by this run; the rest were already on disk.
	pub keyframes_written: u32,
}

impl From<ShotDetectionOutput> for JobOutput {
	fn from(output: ShotDetectionOutput) -> Self {
		JobOutput::custom(output)
	}
}

#[async_trait::async_trait]
impl JobHandler for ShotDetectionJob {
	type Output = ShotDetectionOutput;

	async fn run(&mut self, ctx: JobContext<'_>) -> JobResult<Self::Output> {
		let library_path = ctx.library().path().to_path_buf();
		let tools = ctx.library().core_context().external_tools.clone();

		let content_uuid = content_uuid_of_file(&self.path)
			.await
			.map_err(|error| JobError::execution(format!("could not hash the video: {error}")))?;
		let proxy =
			ContentSidecar::proxy(content_uuid, ProxyResolution::P720).path_in(&library_path);
		let (analyzed, scanned) = if tokio::fs::try_exists(&proxy).await.unwrap_or(false) {
			(AnalyzedMedia::Proxy, proxy)
		} else {
			(AnalyzedMedia::Original, self.path.clone())
		};

		progress(&ctx, 0.0, "Probing", "reading the frame rate", None);
		let probe = {
			let tools = tools.clone();
			let path = self.path.clone();
			blocking(move || probe(&tools, &path)).await?
		};

		progress(&ctx, 0.05, "Scanning", "scoring every frame", None);
		ctx.check_interrupt().await?;
		let frames = {
			let tools = tools.clone();
			let path = scanned.clone();
			let timeout = scan_timeout(probe.duration);
			blocking(move || scan(&tools, &path, timeout)).await?
		};
		if frames.is_empty() {
			return Err(JobError::execution("FFmpeg decoded no video frames"));
		}
		ctx.log(format!(
			"scanned {} frames of the {:?} file",
			frames.len(),
			analyzed
		));

		let scores: Vec<f32> = frames.iter().map(|frame| frame.score).collect();
		let cuts = pick_cuts(&scores, probe.rate.fps(), self.params);
		let list = build_list(
			content_uuid,
			analyzed,
			probe.rate,
			probe.start_timecode,
			frames.len() as u64,
			&cuts,
			self.params,
		);

		let total = list.shots.len() as u64;
		let mut written = 0u32;
		for (done, shot) in list.shots.iter().enumerate() {
			ctx.check_interrupt().await?;
			let path = list.keyframe_path(&library_path, shot);
			if !tokio::fs::try_exists(&path).await.unwrap_or(false) {
				let time = frames
					.get(shot.keyframe.frame as usize)
					.map_or(shot.keyframe.seconds, |frame| frame.time);
				let tools = tools.clone();
				let scanned = scanned.clone();
				blocking(move || extract_keyframe(&tools, &scanned, time, &path)).await?;
				written += 1;
			}
			progress(
				&ctx,
				0.3 + 0.65 * (done as f32 + 1.0) / total as f32,
				"Keyframes",
				format!("shot {} of {total}", done + 1),
				Some((done as u64 + 1, total)),
			);
		}

		remove_stale_keyframes(&library_path, &list).await;
		write_shots(&library_path, &list).await.map_err(|error| {
			JobError::execution(format!("could not write the shot list: {error}"))
		})?;

		progress(
			&ctx,
			1.0,
			"Complete",
			format!("{total} shots"),
			Some((total, total)),
		);
		ctx.log(format!(
			"{total} shots in {:.1}s, {written} keyframes written",
			self.started_at.elapsed().as_secs_f32()
		));
		Ok(ShotDetectionOutput {
			content_uuid,
			analyzed,
			shots: total as u32,
			keyframes_written: written,
		})
	}
}

fn progress(
	ctx: &JobContext<'_>,
	percentage: f32,
	phase: &str,
	message: impl Into<String>,
	completion: Option<(u64, u64)>,
) {
	let mut generic = GenericProgress::new(percentage, phase, message);
	if let Some((done, total)) = completion {
		generic = generic.with_completion(done, total);
	}
	ctx.progress(Progress::generic(generic));
}

async fn blocking<T: Send + 'static>(
	work: impl FnOnce() -> Result<T, String> + Send + 'static,
) -> JobResult<T> {
	tokio::task::spawn_blocking(work)
		.await
		.map_err(|error| JobError::execution(format!("shot detection worker failed: {error}")))?
		.map_err(JobError::execution)
}

/// Long enough for a slow machine to decode a 4K original without a proxy.
fn scan_timeout(duration: Option<f64>) -> Duration {
	let seconds = duration.unwrap_or(4.0 * 3600.0);
	Duration::from_secs_f64((seconds * 4.0).max(600.0))
}

struct Probe {
	rate: FrameRate,
	start_timecode: Option<String>,
	duration: Option<f64>,
}

fn probe(tools: &Arc<ExternalTools>, path: &Path) -> Result<Probe, String> {
	let args: Vec<OsString> = vec![
		"-v".into(),
		"error".into(),
		"-select_streams".into(),
		"v:0".into(),
		"-show_entries".into(),
		"stream=avg_frame_rate,r_frame_rate:stream_tags=timecode:format=duration:format_tags=timecode"
			.into(),
		"-of".into(),
		"json".into(),
		path.as_os_str().to_owned(),
	];
	let output = tools
		.run_ffprobe(&args, PROBE_TIMEOUT, SMALL_OUTPUT_LIMIT)
		.map_err(|error| error.to_string())?;
	if !output.status.success() {
		return Err(format!(
			"FFprobe could not read the video: {}",
			output.stderr.trim()
		));
	}
	let json: Value = serde_json::from_slice(&output.stdout)
		.map_err(|error| format!("FFprobe output was not JSON: {error}"))?;
	parse_probe(&json)
}

/// The original's frame rate, start timecode and duration. The average rate
/// is the one that maps frame numbers onto the file's clock; the nominal
/// rate is the fallback for containers that do not report an average.
fn parse_probe(json: &Value) -> Result<Probe, String> {
	let stream = json["streams"]
		.get(0)
		.ok_or("the file has no video stream")?;
	let rate = [&stream["avg_frame_rate"], &stream["r_frame_rate"]]
		.into_iter()
		.find_map(|value| value.as_str().and_then(FrameRate::parse))
		.ok_or("the video stream has no frame rate")?;
	let start_timecode = [
		&stream["tags"]["timecode"],
		&json["format"]["tags"]["timecode"],
	]
	.into_iter()
	.find_map(|value| value.as_str().map(str::to_string));
	let duration = json["format"]["duration"]
		.as_str()
		.and_then(|value| value.parse().ok());
	Ok(Probe {
		rate,
		start_timecode,
		duration,
	})
}

fn scan(
	tools: &Arc<ExternalTools>,
	path: &Path,
	timeout: Duration,
) -> Result<Vec<FrameScore>, String> {
	let args: Vec<OsString> = vec![
		"-hide_banner".into(),
		"-loglevel".into(),
		"error".into(),
		"-nostdin".into(),
		"-i".into(),
		path.as_os_str().to_owned(),
		"-map".into(),
		"0:v:0".into(),
		"-an".into(),
		"-sn".into(),
		"-dn".into(),
		"-vf".into(),
		format!("scale=-2:{SCAN_HEIGHT},select='gte(scene,0)',metadata=print:file=-").into(),
		"-f".into(),
		"null".into(),
		"-".into(),
	];
	let output = tools
		.run_ffmpeg(&args, timeout, SCAN_OUTPUT_LIMIT)
		.map_err(|error| error.to_string())?;
	if !output.status.success() {
		return Err(format!(
			"FFmpeg could not scan the video: {}",
			output.stderr.trim()
		));
	}
	Ok(parse_scores(&String::from_utf8_lossy(&output.stdout)))
}

fn build_list(
	content_uuid: Uuid,
	analyzed: AnalyzedMedia,
	rate: FrameRate,
	start_timecode: Option<String>,
	frame_count: u64,
	cuts: &[(u64, f32)],
	params: DetectParams,
) -> ShotList {
	let start = start_timecode.as_deref();
	let mut bounds: Vec<(u64, f32)> = vec![(0, 0.0)];
	bounds.extend_from_slice(cuts);
	let shots = bounds
		.iter()
		.enumerate()
		.map(|(index, &(in_frame, cut_score))| {
			let out_frame = bounds.get(index + 1).map_or(frame_count, |next| next.0);
			Shot {
				id: Shot::id_for(content_uuid, in_frame, out_frame),
				index: index as u32,
				in_point: rate.point(in_frame, start),
				out_point: rate.point(out_frame, start),
				duration_frames: out_frame - in_frame,
				duration_seconds: rate.seconds(out_frame - in_frame),
				keyframe: rate.point(in_frame + (out_frame - in_frame) / 2, start),
				cut_score,
			}
		})
		.collect();
	ShotList {
		version: SHOTS_VERSION,
		content_uuid,
		analyzed,
		rate,
		start_timecode,
		out_exclusive: true,
		duration: rate.point(frame_count, None),
		threshold: params.threshold,
		min_shot_seconds: params.min_shot_seconds,
		shots,
	}
}

/// One JPEG at `time` in the scanned file, short side at most 720.
fn extract_keyframe(
	tools: &Arc<ExternalTools>,
	scanned: &Path,
	time: f64,
	path: &Path,
) -> Result<(), String> {
	let partial = partial_path(path);
	if let Some(parent) = path.parent() {
		std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
	}
	let args: Vec<OsString> = vec![
		"-hide_banner".into(),
		"-loglevel".into(),
		"error".into(),
		"-nostdin".into(),
		"-y".into(),
		"-ss".into(),
		format!("{time:.6}").into(),
		"-i".into(),
		scanned.as_os_str().to_owned(),
		"-map".into(),
		"0:v:0".into(),
		"-an".into(),
		"-sn".into(),
		"-dn".into(),
		"-vf".into(),
		"scale='if(gt(iw,ih),-2,min(720,iw))':'if(gt(iw,ih),min(720,ih),-2)'".into(),
		"-frames:v".into(),
		"1".into(),
		"-c:v".into(),
		"mjpeg".into(),
		"-q:v".into(),
		"3".into(),
		"-f".into(),
		"image2".into(),
		"-update".into(),
		"1".into(),
		partial.as_os_str().to_owned(),
	];
	let output = tools
		.run_ffmpeg(&args, KEYFRAME_TIMEOUT, SMALL_OUTPUT_LIMIT)
		.map_err(|error| error.to_string())?;
	if !output.status.success() || !partial.exists() {
		let _ = std::fs::remove_file(&partial);
		return Err(format!(
			"FFmpeg could not extract a keyframe: {}",
			output.stderr.trim()
		));
	}
	std::fs::rename(&partial, path).map_err(|error| error.to_string())
}

/// Drop keyframes of shots this run no longer has, so a changed boundary
/// does not leave its old image behind.
async fn remove_stale_keyframes(library_path: &Path, list: &ShotList) {
	let Some(directory) = list.shots.first().and_then(|shot| {
		list.keyframe_path(library_path, shot)
			.parent()
			.map(Path::to_path_buf)
	}) else {
		return;
	};
	let keep: std::collections::HashSet<String> = list
		.shots
		.iter()
		.map(|shot| format!("{}.jpg", shot.id))
		.collect();
	let Ok(mut entries) = tokio::fs::read_dir(&directory).await else {
		return;
	};
	while let Ok(Some(entry)) = entries.next_entry().await {
		let name = entry.file_name().to_string_lossy().into_owned();
		if !keep.contains(&name) {
			// Best effort: a leftover image is unreferenced and harmless.
			let _ = tokio::fs::remove_file(entry.path()).await;
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn probe_prefers_the_average_rate_and_reads_the_timecode() {
		let json = serde_json::json!({
			"streams": [{ "avg_frame_rate": "24000/1001", "r_frame_rate": "24/1", "tags": { "timecode": "01:00:00:00" } }],
			"format": { "duration": "12.5" }
		});
		let probe = parse_probe(&json).unwrap();
		assert_eq!(
			probe.rate,
			FrameRate {
				num: 24000,
				den: 1001
			}
		);
		assert_eq!(probe.start_timecode.as_deref(), Some("01:00:00:00"));
		assert_eq!(probe.duration, Some(12.5));

		let json = serde_json::json!({
			"streams": [{ "avg_frame_rate": "0/0", "r_frame_rate": "25/1" }],
			"format": { "tags": { "timecode": "10:00:00:00" } }
		});
		let probe = parse_probe(&json).unwrap();
		assert_eq!(probe.rate, FrameRate { num: 25, den: 1 });
		assert_eq!(probe.start_timecode.as_deref(), Some("10:00:00:00"));
	}

	#[test]
	fn shots_tile_the_video_and_keep_ids_across_runs() {
		let content = Uuid::new_v4();
		let rate = FrameRate { num: 24, den: 1 };
		let params = DetectParams::default();
		let list = build_list(
			content,
			AnalyzedMedia::Proxy,
			rate,
			None,
			240,
			&[(100, 0.5)],
			params,
		);
		assert_eq!(list.shots.len(), 2);
		assert_eq!(list.shots[0].in_point.frame, 0);
		assert_eq!(list.shots[0].out_point.frame, 100);
		assert_eq!(list.shots[1].in_point.frame, 100);
		assert_eq!(list.shots[1].out_point.frame, 240);
		assert_eq!(list.shots[1].keyframe.frame, 170);
		assert_eq!(list.shots[1].cut_score, 0.5);

		let again = build_list(
			content,
			AnalyzedMedia::Original,
			rate,
			None,
			240,
			&[(100, 0.5), (200, 0.4)],
			params,
		);
		assert_eq!(again.shots[0].id, list.shots[0].id);
		assert_ne!(again.shots[1].id, list.shots[1].id);
	}
}
