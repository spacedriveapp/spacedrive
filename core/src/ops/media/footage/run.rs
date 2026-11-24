//! `media.footage.run`: start one footage job on a file or folder.

use std::{path::Path, sync::Arc};

use serde::{Deserialize, Serialize};
use specta::Type;
use uuid::Uuid;

use super::{capabilities::capabilities, local_footage_path, FootageJob};
use crate::{
	context::CoreContext,
	domain::SdPath,
	infra::action::{error::ActionError, LibraryAction},
	library::Library,
};

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct FootageRunInput {
	/// A video, or a folder whose videos are processed at any depth.
	pub path: SdPath,
	pub job: FootageJob,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct FootageRunOutput {
	/// The jobs started: one for most work, one per video for shot detection
	/// over a folder, none when the folder holds no video.
	pub job_ids: Vec<Uuid>,
}

#[derive(Debug, Clone)]
pub struct FootageRunAction {
	input: FootageRunInput,
}

impl LibraryAction for FootageRunAction {
	type Input = FootageRunInput;
	type Output = FootageRunOutput;

	fn from_input(input: Self::Input) -> Result<Self, String> {
		local_footage_path(&input.path)?;
		Ok(Self { input })
	}

	async fn execute(
		self,
		library: Arc<Library>,
		context: Arc<CoreContext>,
	) -> Result<Self::Output, ActionError> {
		let capabilities = capabilities(&context).await;
		let support = match self.input.job {
			FootageJob::Transcribe => capabilities.transcribe,
			FootageJob::Proxy => capabilities.proxy,
			FootageJob::Shots => capabilities.shots,
			FootageJob::Analyze => capabilities.analyze,
		};
		if let Some(reason) = support.reason {
			return Err(ActionError::InvalidInput(reason));
		}
		let path = local_footage_path(&self.input.path).map_err(ActionError::InvalidInput)?;
		if let Err(error) = tokio::fs::metadata(&path).await {
			return Err(ActionError::InvalidInput(format!(
				"{}: {error}",
				path.display()
			)));
		}
		let job_ids = match self.input.job {
			FootageJob::Transcribe => transcribe(&path, library, context).await?,
			FootageJob::Proxy => proxy(self.input.path, library, context).await?,
			FootageJob::Shots => shots(&path, library, context).await?,
			FootageJob::Analyze => analyze(&path, library, context).await?,
		};
		Ok(FootageRunOutput { job_ids })
	}

	fn action_kind(&self) -> &'static str {
		"media.footage.run"
	}
}

crate::register_library_action!(FootageRunAction, "media.footage.run");

#[cfg(feature = "speech-to-text")]
async fn transcribe(
	path: &Path,
	library: Arc<Library>,
	context: Arc<CoreContext>,
) -> Result<Vec<Uuid>, ActionError> {
	use crate::ops::transcription::action::{TranscribeAction, TranscribeInput};

	let action = TranscribeAction::from_input(TranscribeInput {
		path: path.to_path_buf(),
		model: None,
		language: None,
		force: false,
	})
	.map_err(ActionError::InvalidInput)?;
	Ok(vec![action.execute(library, context).await?.job_id])
}

#[cfg(not(feature = "speech-to-text"))]
async fn transcribe(
	_path: &Path,
	_library: Arc<Library>,
	_context: Arc<CoreContext>,
) -> Result<Vec<Uuid>, ActionError> {
	unreachable!("capabilities refuse transcription in this build")
}

#[cfg(feature = "video-proxies")]
async fn proxy(
	scope: SdPath,
	library: Arc<Library>,
	context: Arc<CoreContext>,
) -> Result<Vec<Uuid>, ActionError> {
	use crate::ops::proxies::generate::{ProxyGenerateAction, ProxyGenerateInput};

	let action = ProxyGenerateAction::from_input(ProxyGenerateInput {
		scope,
		recursive: true,
		resolution: Default::default(),
	})
	.map_err(ActionError::InvalidInput)?;
	Ok(vec![action.execute(library, context).await?.id.0])
}

#[cfg(not(feature = "video-proxies"))]
async fn proxy(
	_scope: SdPath,
	_library: Arc<Library>,
	_context: Arc<CoreContext>,
) -> Result<Vec<Uuid>, ActionError> {
	unreachable!("capabilities refuse proxies in this build")
}

/// Shot detection works on one file, so a folder gets a job per video.
#[cfg(feature = "video-shots")]
async fn shots(
	path: &Path,
	library: Arc<Library>,
	context: Arc<CoreContext>,
) -> Result<Vec<Uuid>, ActionError> {
	use crate::ops::media::shots::{ShotDetectAction, ShotDetectInput};

	let videos = super::videos_at(path)
		.await
		.map_err(|error| ActionError::InvalidInput(format!("{}: {error}", path.display())))?;
	let mut job_ids = Vec::with_capacity(videos.len());
	for video in videos {
		let action = ShotDetectAction::from_input(ShotDetectInput {
			path: SdPath::local(video),
			threshold: None,
			min_shot_seconds: None,
		})
		.map_err(ActionError::InvalidInput)?;
		job_ids.push(action.execute(library.clone(), context.clone()).await?.id.0);
	}
	Ok(job_ids)
}

#[cfg(not(feature = "video-shots"))]
async fn shots(
	_path: &Path,
	_library: Arc<Library>,
	_context: Arc<CoreContext>,
) -> Result<Vec<Uuid>, ActionError> {
	unreachable!("capabilities refuse shot detection in this build")
}

/// The Videos extension analyzes records, so the path is turned into the
/// records its source holds for the videos at or under it.
#[cfg(feature = "wasm")]
async fn analyze(
	path: &Path,
	library: Arc<Library>,
	context: Arc<CoreContext>,
) -> Result<Vec<Uuid>, ActionError> {
	let records = video_records(&context, path).await?;
	if records.is_empty() {
		return Ok(Vec::new());
	}
	let plugins = context
		.get_plugin_manager()
		.await
		.ok_or_else(|| ActionError::Internal("extensions are not initialized".into()))?;
	let state = serde_json::json!({ "records": records }).to_string();
	let job = plugins
		.read()
		.await
		.job_registry()
		.create_wasm_job(&format!("{}:analyze", super::VIDEOS_EXTENSION_ID), state)
		.map_err(ActionError::InvalidInput)?;
	let handle = library
		.jobs()
		.dispatch(job)
		.await
		.map_err(ActionError::Job)?;
	Ok(vec![handle.id().0])
}

#[cfg(not(feature = "wasm"))]
async fn analyze(
	_path: &Path,
	_library: Arc<Library>,
	_context: Arc<CoreContext>,
) -> Result<Vec<Uuid>, ActionError> {
	unreachable!("capabilities refuse analysis in this build")
}

/// Record uuids of the videos at or beneath `path` in the source that holds it.
#[cfg(feature = "wasm")]
async fn video_records(context: &CoreContext, path: &Path) -> Result<Vec<Uuid>, ActionError> {
	let index = context.volume_index();
	let not_tracked = || {
		ActionError::InvalidInput(format!(
			"{} is not under a source on this device, so the Videos extension cannot read it",
			path.display()
		))
	};
	let root = index.source_root_for(path).ok_or_else(not_tracked)?;
	let store = index.store_for(&root).await.ok_or_else(not_tracked)?;
	let canonical_root = tokio::fs::canonicalize(&root).await.unwrap_or(root);
	let canonical = tokio::fs::canonicalize(path)
		.await
		.map_err(|error| ActionError::InvalidInput(format!("{}: {error}", path.display())))?;
	let relative = canonical
		.strip_prefix(&canonical_root)
		.map_err(|_| not_tracked())?
		.components()
		.map(|part| part.as_os_str().to_string_lossy())
		.collect::<Vec<_>>()
		.join("/");
	let pool = store.db().pool();
	let failed = |error: sd_store::Error| ActionError::Internal(error.to_string());

	if !relative.is_empty() {
		if let Some(entry) = sd_store::read::entry_by_path(pool, &relative)
			.await
			.map_err(failed)?
		{
			if entry.kind == sd_store::FileKind::File {
				return Ok(vec![entry.uuid]);
			}
		}
	}
	let extensions: Vec<String> = VIDEO_EXTENSIONS.iter().map(|e| e.to_string()).collect();
	let mut records = Vec::new();
	let mut cursor: Option<(String, String)> = None;
	loop {
		let start = match &cursor {
			Some((directory, name)) => sd_store::read::Start::After { directory, name },
			None => sd_store::read::Start::First,
		};
		let batch =
			sd_store::read::files_beneath(pool, &relative, start, Some(&extensions), false, PAGE)
				.await
				.map_err(failed)?;
		records.extend(batch.iter().map(|entry| entry.uuid));
		if batch.len() < PAGE {
			break;
		}
		cursor = batch
			.last()
			.map(|entry| (entry.directory().to_string(), entry.name.clone()));
	}
	Ok(records)
}

#[cfg(feature = "wasm")]
const PAGE: usize = 1000;

/// The extensions the Videos manifest grants it to read.
#[cfg(feature = "wasm")]
const VIDEO_EXTENSIONS: &[&str] = &[
	"mp4", "mov", "m4v", "mkv", "avi", "webm", "mts", "m2ts", "mxf",
];
