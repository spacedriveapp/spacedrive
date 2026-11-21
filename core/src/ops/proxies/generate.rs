//! # Proxy generation job
//!
//! A path selects video files on this device, and each gets a proxy and its
//! speech audio. Files are taken in path order and the last settled path is
//! checkpointed after each one, so a killed daemon resumes at the next file.
//! A file whose sidecars already exist for its content is skipped, which is
//! what makes a rerun, or a run over moved and renamed files, do nothing.

use std::{
	path::{Component, Path, PathBuf},
	sync::Arc,
};

use serde::{Deserialize, Serialize};
use specta::Type;

use super::recipe;
use crate::{
	context::CoreContext,
	domain::{sidecar::content_uuid_of_file, ContentSidecar, ProxyResolution, SdPath},
	infra::{
		action::{error::ActionError, LibraryAction},
		job::{handle::JobReceipt, prelude::*, traits::DynJob},
	},
	library::Library,
	service::thumbs::is_video,
};

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct ProxyGenerateInput {
	/// A video file, or a directory whose videos are processed.
	pub scope: SdPath,
	/// Include subdirectories. False selects only immediate files.
	#[serde(default)]
	pub recursive: bool,
	#[serde(default)]
	pub resolution: ProxyResolution,
}

pub struct ProxyGenerateAction {
	input: ProxyGenerateInput,
}

impl LibraryAction for ProxyGenerateAction {
	type Input = ProxyGenerateInput;
	type Output = JobReceipt;

	fn from_input(input: Self::Input) -> Result<Self, String> {
		let path = input
			.scope
			.as_local_path()
			.ok_or("Proxy generation requires a path on this device")?;
		if !path.is_absolute()
			|| path
				.components()
				.any(|part| matches!(part, Component::ParentDir))
		{
			return Err("Proxy scope must be an absolute path without parent traversal".into());
		}
		Ok(Self { input })
	}

	async fn execute(
		self,
		library: Arc<Library>,
		context: Arc<CoreContext>,
	) -> Result<JobReceipt, ActionError> {
		let scope = self.input.scope.as_local_path().unwrap().to_path_buf();
		let metadata = tokio::fs::metadata(&scope)
			.await
			.map_err(|error| ActionError::InvalidInput(format!("{}: {error}", scope.display())))?;
		if !metadata.is_file() && !metadata.is_dir() {
			return Err(ActionError::InvalidInput(
				"Select a regular file or directory".into(),
			));
		}
		if context.external_tools.ffmpeg_path().is_none()
			|| context.external_tools.ffprobe_path().is_none()
		{
			return Err(ActionError::InvalidInput(
				"Proxy generation needs FFmpeg and FFprobe installed".into(),
			));
		}
		let job = ProxyGenerateJob {
			scope,
			recursive: self.input.recursive,
			resolution: self.input.resolution,
			cursor: None,
			generated: 0,
			skipped: 0,
			failed: 0,
		};
		library
			.jobs()
			.dispatch(job)
			.await
			.map(Into::into)
			.map_err(ActionError::Job)
	}

	fn action_kind(&self) -> &'static str {
		"media.proxies.generate"
	}
}

crate::register_library_action!(ProxyGenerateAction, "media.proxies.generate");

#[derive(Debug, Serialize, Deserialize, Job)]
pub struct ProxyGenerateJob {
	scope: PathBuf,
	recursive: bool,
	resolution: ProxyResolution,
	/// Last settled file, so a resumed run starts after it.
	cursor: Option<PathBuf>,
	generated: u64,
	skipped: u64,
	failed: u64,
}

impl Job for ProxyGenerateJob {
	const NAME: &'static str = "proxy_generate";
	const RESUMABLE: bool = true;
	const DESCRIPTION: Option<&'static str> =
		Some("Generate video proxies and speech audio for a file or folder");
}

impl DynJob for ProxyGenerateJob {
	fn job_name(&self) -> &'static str {
		Self::NAME
	}
	fn dedup_key(&self) -> Option<String> {
		Some(format!(
			"{:?}:{}:{:?}",
			self.scope, self.recursive, self.resolution
		))
	}
}

enum Outcome {
	Generated,
	Skipped,
	Failed(String),
}

#[async_trait::async_trait]
impl JobHandler for ProxyGenerateJob {
	type Output = JobOutput;

	async fn run(&mut self, ctx: JobContext<'_>) -> JobResult<JobOutput> {
		// Checkpoints can be newer than the job row after an abrupt restart.
		if let Some(saved) = ctx.load_state::<Self>().await? {
			*self = saved;
		}
		let tools = ctx.library().core_context().external_tools.clone();
		let ffmpeg = tools
			.ffmpeg_path()
			.ok_or_else(|| JobError::execution("FFmpeg is not installed"))?;
		let library_path = ctx.library().path().to_path_buf();

		let (scope, recursive) = (self.scope.clone(), self.recursive);
		let videos = tokio::task::spawn_blocking(move || list_videos(&scope, recursive))
			.await
			.map_err(|error| JobError::execution(error.to_string()))?
			.map_err(|error| JobError::execution(format!("{}: {error}", self.scope.display())))?;
		let videos: Vec<_> = videos
			.into_iter()
			.filter(|path| self.cursor.as_ref().is_none_or(|cursor| path > cursor))
			.collect();
		let total = self.generated + self.skipped + self.failed + videos.len() as u64;
		ctx.log(format!(
			"Generating {} proxies and speech audio for {} videos at {}",
			self.resolution.variant(),
			videos.len(),
			self.scope.display()
		));

		for video in videos {
			ctx.check_interrupt().await?;
			let done = self.generated + self.skipped + self.failed;
			let report = |phase: &str, fraction: f64| {
				ctx.progress(Progress::generic(GenericProgress::new(
					((done as f64 + fraction.clamp(0.0, 1.0)) / total.max(1) as f64) as f32,
					phase.to_string(),
					format!("{}/{total}: {}", done + 1, video.display()),
				)));
			};
			match self
				.process(&ctx, &tools, &ffmpeg, &library_path, &video, report)
				.await?
			{
				Outcome::Generated => self.generated += 1,
				Outcome::Skipped => self.skipped += 1,
				Outcome::Failed(reason) => {
					self.failed += 1;
					ctx.add_warning(format!("{}: {reason}", video.display()));
				}
			}
			self.cursor = Some(video);
			ctx.checkpoint_with_state(self).await?;
		}

		ctx.log(format!(
			"Proxies: {} generated, {} skipped, {} failed",
			self.generated, self.skipped, self.failed
		));
		Ok(JobOutput::ProxyGeneration {
			generated_count: self.generated,
			skipped_count: self.skipped,
			error_count: self.failed,
		})
	}

	fn is_resuming(&self) -> bool {
		self.cursor.is_some()
	}
}

impl ProxyGenerateJob {
	async fn process(
		&self,
		ctx: &JobContext<'_>,
		tools: &Arc<crate::service::external_tools::ExternalTools>,
		ffmpeg: &Path,
		library_path: &Path,
		video: &Path,
		report: impl Fn(&str, f64),
	) -> JobResult<Outcome> {
		let content_uuid = match content_uuid_of_file(video).await {
			Ok(uuid) => uuid,
			Err(error) => return Ok(Outcome::Failed(format!("hash: {error}"))),
		};
		let proxy = ContentSidecar::proxy(content_uuid, self.resolution).path_in(library_path);
		let audio = ContentSidecar::speech_audio(content_uuid).path_in(library_path);
		let proxy_missing = !proxy.is_file();
		let audio_missing = !audio.is_file();
		if !proxy_missing && !audio_missing {
			return Ok(Outcome::Skipped);
		}

		let probe = {
			let (tools, video) = (tools.clone(), video.to_path_buf());
			match tokio::task::spawn_blocking(move || recipe::probe(&tools, &video))
				.await
				.map_err(|error| JobError::execution(error.to_string()))?
			{
				Ok(probe) => probe,
				Err(error) => return Ok(Outcome::Failed(error)),
			}
		};
		if !probe.has_video {
			return Ok(Outcome::Failed("no video stream".into()));
		}
		let audio_missing = audio_missing && probe.has_audio;
		if !proxy_missing && !audio_missing {
			return Ok(Outcome::Skipped);
		}
		let duration = probe.duration.unwrap_or(0.0);

		if proxy_missing {
			let phase = format!("Writing {} proxy", self.resolution.variant());
			let result = publish(
				ctx,
				ffmpeg,
				&proxy,
				|partial| {
					recipe::proxy_args(video, partial, self.resolution, probe.timecode.as_deref())
				},
				|seconds| {
					if duration > 0.0 {
						report(&phase, seconds / duration * 0.9);
					}
				},
			)
			.await?;
			if let Err(error) = result {
				return Ok(Outcome::Failed(error));
			}
		}
		if audio_missing {
			let result = publish(
				ctx,
				ffmpeg,
				&audio,
				|partial| recipe::speech_audio_args(video, partial),
				|seconds| {
					if duration > 0.0 {
						report("Extracting speech audio", 0.9 + seconds / duration * 0.1);
					}
				},
			)
			.await?;
			if let Err(error) = result {
				return Ok(Outcome::Failed(error));
			}
		}
		Ok(Outcome::Generated)
	}
}

/// Write a sidecar beside its final path and rename it into place, so a file
/// at a sidecar path is always complete.
async fn publish(
	ctx: &JobContext<'_>,
	ffmpeg: &Path,
	output: &Path,
	args: impl FnOnce(&Path) -> Vec<std::ffi::OsString>,
	on_progress: impl FnMut(f64),
) -> JobResult<Result<(), String>> {
	let parent = output.parent().expect("sidecar paths have a parent");
	if let Err(error) = tokio::fs::create_dir_all(parent).await {
		return Ok(Err(format!("create {}: {error}", parent.display())));
	}
	let name = output
		.file_name()
		.expect("sidecar paths have a file name")
		.to_string_lossy();
	let (stem, extension) = name.rsplit_once('.').unwrap_or((&name, ""));
	let partial = parent.join(format!(".{stem}.partial.{extension}"));
	if let Err(error) = recipe::run_ffmpeg(ctx, ffmpeg, &args(&partial), on_progress).await? {
		let _ = tokio::fs::remove_file(&partial).await;
		return Ok(Err(error));
	}
	match tokio::fs::metadata(&partial).await {
		Ok(metadata) if metadata.len() > 0 => {}
		_ => return Ok(Err(format!("FFmpeg wrote no output for {name}"))),
	}
	Ok(tokio::fs::rename(&partial, output)
		.await
		.map_err(|error| format!("publish {}: {error}", output.display())))
}

/// Video files under `scope` in path order, or `scope` itself when it is one.
/// Hidden files and directories are left out.
fn list_videos(scope: &Path, recursive: bool) -> std::io::Result<Vec<PathBuf>> {
	if scope.is_file() {
		return Ok(is_video(scope)
			.then(|| scope.to_path_buf())
			.into_iter()
			.collect());
	}
	let mut videos = Vec::new();
	let mut pending = vec![scope.to_path_buf()];
	while let Some(directory) = pending.pop() {
		for entry in std::fs::read_dir(&directory)? {
			let entry = entry?;
			if entry.file_name().to_string_lossy().starts_with('.') {
				continue;
			}
			let file_type = entry.file_type()?;
			let path = entry.path();
			if file_type.is_dir() && recursive {
				pending.push(path);
			} else if file_type.is_file() && is_video(&path) {
				videos.push(path);
			}
		}
	}
	videos.sort();
	Ok(videos)
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn videos_are_listed_in_path_order_without_hidden_files() {
		let dir = tempfile::tempdir().unwrap();
		let root = dir.path();
		std::fs::create_dir_all(root.join("b/.cache")).unwrap();
		for name in [
			"z.mov",
			"a.MP4",
			"notes.txt",
			".hidden.mov",
			"b/c.mkv",
			"b/.cache/d.mov",
		] {
			std::fs::write(root.join(name), b"x").unwrap();
		}
		assert_eq!(
			list_videos(root, true).unwrap(),
			vec![root.join("a.MP4"), root.join("b/c.mkv"), root.join("z.mov")]
		);
		assert_eq!(
			list_videos(root, false).unwrap(),
			vec![root.join("a.MP4"), root.join("z.mov")]
		);
		assert_eq!(
			list_videos(&root.join("z.mov"), false).unwrap(),
			vec![root.join("z.mov")]
		);
	}

	#[test]
	fn checkpoint_keeps_the_cursor_and_counts() {
		let job = ProxyGenerateJob {
			scope: "/footage".into(),
			recursive: true,
			resolution: ProxyResolution::P1080,
			cursor: Some("/footage/b/c.mkv".into()),
			generated: 3,
			skipped: 2,
			failed: 1,
		};
		let restored: ProxyGenerateJob =
			rmp_serde::from_slice(&rmp_serde::to_vec(&job).unwrap()).unwrap();
		assert_eq!(restored.cursor, job.cursor);
		assert_eq!(restored.resolution, ProxyResolution::P1080);
		assert_eq!(restored.dedup_key(), job.dedup_key());
		assert_eq!(
			(restored.generated, restored.skipped, restored.failed),
			(3, 2, 1)
		);
	}

	#[test]
	fn scope_must_be_a_local_absolute_path() {
		for scope in [
			SdPath::Physical {
				device_slug: "another-machine".into(),
				path: "/footage".into(),
			},
			SdPath::Physical {
				device_slug: "local".into(),
				path: "footage".into(),
			},
			SdPath::Physical {
				device_slug: "local".into(),
				path: "/footage/../private".into(),
			},
		] {
			assert!(ProxyGenerateAction::from_input(ProxyGenerateInput {
				scope,
				recursive: true,
				resolution: ProxyResolution::P720,
			})
			.is_err());
		}
	}
}
