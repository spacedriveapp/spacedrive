use std::{
	collections::HashMap,
	path::{Path, PathBuf},
	sync::{
		atomic::{AtomicBool, Ordering},
		Arc, LazyLock, Mutex, PoisonError, Weak,
	},
	time::{Duration, Instant},
};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use specta::Type;
use uuid::Uuid;

use super::{
	detect::{pick_cuts, DetectParams, FrameScore},
	sidecar::{
		content_uuid_of_video, write_shots, AnalyzedMedia, FrameRate, Shot, ShotList, SHOTS_VERSION,
	},
};
use crate::{
	domain::sidecar::{ContentSidecar, ProxyResolution},
	infra::job::{generic_progress::GenericProgress, prelude::*},
	service::sidecars::{Home, SidecarRegistry},
};

/// Height the scan decodes to. Scene scores barely change below 720p and
/// decoding gets several times faster.
const SCAN_HEIGHT: u32 = 270;
/// Scores every frame, at the scan height, for the cut picker.
const SCAN_FILTER: &str = "scale=-2:270,select='gte(scene,0)'";
/// Short side at most 720, in the full-range 4:2:0 a JPEG stores.
const KEYFRAME_FILTER: &str =
	"scale='if(gt(iw,ih),-2,min(720,iw))':'if(gt(iw,ih),min(720,ih),-2)',format=yuvj420p";
const PROBE_TIMEOUT: Duration = Duration::from_secs(30);
const KEYFRAME_TIMEOUT: Duration = Duration::from_secs(60);
/// MJPEG quantizer scale, as `-q:v 3`.
const KEYFRAME_QUALITY: i32 = 3;

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
		let registry = SidecarRegistry::of(ctx.library());
		let finder = registry.finder();

		let content_uuid = content_uuid_of_video(&self.path)
			.await
			.map_err(|error| JobError::execution(format!("could not hash the video: {error}")))?;
		// Two runs on the same content would write the same partial files and
		// prune each other's keyframes, so they take turns.
		let lock = content_lock(ctx.library().id(), content_uuid);
		let _turn = lock.lock().await;
		let home = registry
			.home(content_uuid, Some(&self.path))
			.await
			.map_err(|error| JobError::execution(format!("nowhere to keep the shots: {error}")))?;
		let (analyzed, scanned) =
			match finder.find(&ContentSidecar::proxy(content_uuid, ProxyResolution::P720)) {
				Some(proxy) => (AnalyzedMedia::Proxy, proxy),
				None => (AnalyzedMedia::Original, self.path.clone()),
			};

		progress(&ctx, 0.0, "Probing", "reading the frame rate", None);
		let probe = {
			let path = self.path.clone();
			blocking(move || probe(&path)).await?
		};

		progress(&ctx, 0.05, "Scanning", "scoring every frame", None);
		ctx.check_interrupt().await?;
		let frames = scan(&ctx, scanned.clone()).await?;
		if frames.is_empty() {
			return Err(JobError::execution("the scan decoded no video frames"));
		}
		ctx.log(format!(
			"scanned {} frames of the {:?} file",
			frames.len(),
			analyzed
		));

		let cuts = pick_cuts(&frames, probe.rate.fps(), self.params);
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
			let keyframe = list.keyframe(shot);
			if finder.find(&keyframe).is_none() {
				let time = frames
					.get(shot.keyframe.frame as usize)
					.map_or(shot.keyframe.seconds, |frame| frame.time);
				let (scanned, partial) = (scanned.clone(), home.partial(&keyframe));
				blocking(move || extract_keyframe(&scanned, time, &partial)).await?;
				registry
					.publish(&home, &keyframe, &home.partial(&keyframe), None)
					.await
					.map_err(|error| {
						JobError::execution(format!("could not publish a keyframe: {error}"))
					})?;
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

		write_shots(&registry, &home, &list)
			.await
			.map_err(|error| {
				JobError::execution(format!("could not write the shot list: {error}"))
			})?;
		remove_stale_keyframes(&registry, &home, &list).await;

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

/// One lock per library and content, alive while a run holds or waits on it.
fn content_lock(library_id: Uuid, content_uuid: Uuid) -> Arc<tokio::sync::Mutex<()>> {
	type Locks = HashMap<(Uuid, Uuid), Weak<tokio::sync::Mutex<()>>>;
	static LOCKS: LazyLock<Mutex<Locks>> = LazyLock::new(Default::default);
	let mut locks = LOCKS.lock().unwrap_or_else(PoisonError::into_inner);
	locks.retain(|_, lock| lock.strong_count() > 0);
	let key = (library_id, content_uuid);
	if let Some(lock) = locks.get(&key).and_then(Weak::upgrade) {
		return lock;
	}
	let lock = Arc::new(tokio::sync::Mutex::new(()));
	locks.insert(key, Arc::downgrade(&lock));
	lock
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

struct Probe {
	rate: FrameRate,
	start_timecode: Option<String>,
}

fn probe(path: &Path) -> Result<Probe, String> {
	let json = sd_ffmpeg::footage::probe(path, PROBE_TIMEOUT)
		.map_err(|error| format!("could not read the video: {error}"))?;
	parse_probe(&json)
}

/// The original's frame rate and start timecode. The average rate
/// is the one that maps frame numbers onto the file's clock; the nominal
/// rate is the fallback for containers that do not report an average.
fn parse_probe(json: &Value) -> Result<Probe, String> {
	let stream = json["streams"]
		.as_array()
		.and_then(|streams| {
			streams
				.iter()
				.find(|stream| stream["codec_type"] == "video")
		})
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
	Ok(Probe {
		rate,
		start_timecode,
	})
}

/// Score every frame, checking for pause and cancel as the scan runs.
///
/// A 4K original without a proxy takes minutes to decode, so waiting for
/// the scan before honouring an interrupt would hold a pause that long.
async fn scan(ctx: &JobContext<'_>, path: PathBuf) -> JobResult<Vec<FrameScore>> {
	let cancel = Arc::new(AtomicBool::new(false));
	let mut work = tokio::task::spawn_blocking({
		let cancel = cancel.clone();
		move || sd_ffmpeg::footage::scene_scores(&path, SCAN_FILTER, &cancel)
	});
	let mut tick = tokio::time::interval(Duration::from_millis(500));
	let scores = loop {
		tokio::select! {
			done = &mut work => break done,
			_ = tick.tick() => {
				if let Err(interrupt) = ctx.check_interrupt().await {
					cancel.store(true, Ordering::Relaxed);
					let _ = work.await;
					return Err(interrupt);
				}
			}
		}
	};
	let scores = scores
		.map_err(|error| JobError::execution(format!("shot detection worker failed: {error}")))?
		.map_err(|error| JobError::execution(format!("could not scan the video: {error}")))?;
	Ok(scores
		.into_iter()
		.map(|frame| FrameScore {
			time: frame.time,
			score: frame.score,
		})
		.collect())
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
/// Write the frame at `time` to `partial`, which the caller publishes.
fn extract_keyframe(scanned: &Path, time: f64, partial: &Path) -> Result<(), String> {
	if let Some(parent) = partial.parent() {
		std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
	}
	if let Err(error) = sd_ffmpeg::footage::write_jpeg(
		scanned,
		time,
		KEYFRAME_FILTER,
		KEYFRAME_QUALITY,
		partial,
		KEYFRAME_TIMEOUT,
	) {
		let _ = std::fs::remove_file(partial);
		return Err(format!("could not extract a keyframe: {error}"));
	}
	Ok(())
}

/// Drop keyframes of shots this run no longer has, so a changed boundary
/// does not leave its old image behind.
async fn remove_stale_keyframes(registry: &SidecarRegistry, home: &Home, list: &ShotList) {
	let Some(directory) = list.shots.first().and_then(|shot| {
		home.path(&list.keyframe(shot))
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
		if keep.contains(&name) {
			continue;
		}
		// Best effort: a leftover image is unreferenced and harmless.
		let stale = entry
			.path()
			.strip_prefix(&home.root)
			.ok()
			.and_then(ContentSidecar::from_relative_path);
		match stale {
			Some(stale) => {
				let _ = registry.remove(&stale).await;
			}
			None => {
				let _ = tokio::fs::remove_file(entry.path()).await;
			}
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn probe_prefers_the_average_rate_and_reads_the_timecode() {
		let json = serde_json::json!({
			"streams": [{ "codec_type": "video", "avg_frame_rate": "24000/1001", "r_frame_rate": "24/1", "tags": { "timecode": "01:00:00:00" } }],
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

		let json = serde_json::json!({
			"streams": [{ "codec_type": "audio" }, { "codec_type": "video", "avg_frame_rate": "0/0", "r_frame_rate": "25/1" }],
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

	/// Three shots rendered with the linked libraries, cut where the
	/// picture changes, with a keyframe for each. No `ffmpeg` executable is
	/// involved.
	#[test]
	fn hard_cuts_in_rendered_footage_split_into_shots() {
		let dir = tempfile::tempdir().unwrap();
		let video = dir.path().join("three.mov");
		sd_ffmpeg::footage::render(
			"color=c=red:s=320x180:r=24:d=2,format=yuv420p[a];testsrc2=s=320x180:r=24:d=2,format=yuv420p[b];color=c=blue:s=320x180:r=24:d=2,format=yuv420p[c];[a][b][c]concat=n=3:v=1:a=0",
			"libx264",
			&[("preset".to_string(), "ultrafast".to_string())],
			"mov",
			&[("timecode".to_string(), "01:00:00:00".to_string())],
			&video,
		)
		.unwrap();
		let probe = probe(&video).unwrap();
		assert_eq!(probe.rate, FrameRate { num: 24, den: 1 });
		assert_eq!(probe.start_timecode.as_deref(), Some("01:00:00:00"));

		let frames: Vec<FrameScore> =
			sd_ffmpeg::footage::scene_scores(&video, SCAN_FILTER, &AtomicBool::new(false))
				.unwrap()
				.into_iter()
				.map(|frame| FrameScore {
					time: frame.time,
					score: frame.score,
				})
				.collect();
		assert_eq!(frames.len(), 144);
		assert_eq!(frames[48].time, 2.0);
		let cuts: Vec<u64> = pick_cuts(&frames, probe.rate.fps(), DetectParams::default())
			.into_iter()
			.map(|(frame, _)| frame)
			.collect();
		assert_eq!(cuts, vec![48, 96]);

		let keyframe = dir.path().join("shot.jpg");
		extract_keyframe(&video, frames[72].time, &keyframe).unwrap();
		let image = image::open(&keyframe).unwrap();
		assert_eq!((image.width(), image.height()), (320, 180));
	}
}
