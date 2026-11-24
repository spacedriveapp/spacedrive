//! `media.footage.capabilities`: which footage jobs this daemon can run.

use std::sync::Arc;

use serde::{Deserialize, Serialize};
use specta::Type;

use crate::{
	context::CoreContext,
	infra::{
		api::SessionContext,
		query::{LibraryQuery, QueryResult},
	},
};

/// Whether a footage job can start, and if not, what the person can do.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct FootageSupport {
	pub available: bool,
	/// Why it cannot start, written for a person. `None` when available.
	pub reason: Option<String>,
}

impl FootageSupport {
	fn yes() -> Self {
		Self {
			available: true,
			reason: None,
		}
	}

	fn no(reason: impl Into<String>) -> Self {
		Self {
			available: false,
			reason: Some(reason.into()),
		}
	}
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, Type)]
pub struct FootageCapabilitiesInput {}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct FootageCapabilities {
	pub transcribe: FootageSupport,
	pub proxy: FootageSupport,
	pub shots: FootageSupport,
	pub analyze: FootageSupport,
	pub export: FootageSupport,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FootageCapabilitiesQuery;

impl LibraryQuery for FootageCapabilitiesQuery {
	type Input = FootageCapabilitiesInput;
	type Output = FootageCapabilities;

	fn from_input(_input: Self::Input) -> QueryResult<Self> {
		Ok(Self)
	}

	async fn execute(
		self,
		context: Arc<CoreContext>,
		_session: SessionContext,
	) -> QueryResult<Self::Output> {
		Ok(capabilities(&context).await)
	}
}

crate::register_library_query!(FootageCapabilitiesQuery, "media.footage.capabilities");

/// What this daemon can run now. Checked again when a job is asked for,
/// since FFmpeg, a model or the extension can come or go in between.
pub(crate) async fn capabilities(context: &CoreContext) -> FootageCapabilities {
	let tools = &context.external_tools;
	let has_ffmpeg = tools.ffmpeg_path().is_some() && tools.ffprobe_path().is_some();
	let needs_ffmpeg = || FootageSupport::no("Install FFmpeg in Settings > Services first");
	let host_ffmpeg = |built: bool, missing: &str| {
		if !built {
			FootageSupport::no(missing)
		} else if has_ffmpeg {
			FootageSupport::yes()
		} else {
			needs_ffmpeg()
		}
	};
	FootageCapabilities {
		transcribe: transcribe_support().await,
		proxy: host_ffmpeg(
			cfg!(feature = "video-proxies"),
			"This build of Spacedrive does not make proxies",
		),
		shots: host_ffmpeg(
			cfg!(feature = "video-shots"),
			"This build of Spacedrive does not detect shots",
		),
		analyze: analyze_support(context).await,
		export: host_ffmpeg(
			cfg!(feature = "footage-clips"),
			"This build of Spacedrive does not export timelines",
		),
	}
}

#[cfg(feature = "speech-to-text")]
async fn transcribe_support() -> FootageSupport {
	use crate::ops::models::{WhisperModel, WhisperModelManager};

	let Ok(data_dir) = crate::config::default_data_dir() else {
		return FootageSupport::no("The data directory could not be found");
	};
	let models = WhisperModelManager::new(&data_dir);
	for model in [
		WhisperModel::Tiny,
		WhisperModel::Base,
		WhisperModel::Small,
		WhisperModel::Medium,
		WhisperModel::Large,
	] {
		if models.is_downloaded(&model).await {
			return FootageSupport::yes();
		}
	}
	FootageSupport::no("Download a Whisper speech model first")
}

#[cfg(not(feature = "speech-to-text"))]
async fn transcribe_support() -> FootageSupport {
	FootageSupport::no("This build of Spacedrive does not transcribe")
}

#[cfg(feature = "wasm")]
async fn analyze_support(context: &CoreContext) -> FootageSupport {
	let Some(plugins) = context.get_plugin_manager().await else {
		return FootageSupport::no("Extensions have not started yet");
	};
	let job = format!("{}:analyze", super::VIDEOS_EXTENSION_ID);
	if plugins.read().await.job_registry().get_job(&job).is_some() {
		FootageSupport::yes()
	} else {
		FootageSupport::no("Install the Videos extension first")
	}
}

#[cfg(not(feature = "wasm"))]
async fn analyze_support(_context: &CoreContext) -> FootageSupport {
	FootageSupport::no("This build of Spacedrive has no extension runtime")
}
