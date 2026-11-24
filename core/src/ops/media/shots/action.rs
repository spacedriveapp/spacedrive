use std::{path::PathBuf, sync::Arc};

use serde::{Deserialize, Serialize};
use specta::Type;

use super::{detect::DetectParams, job::ShotDetectionJob};
use crate::{
	context::CoreContext,
	domain::SdPath,
	infra::{
		action::{error::ActionError, LibraryAction},
		job::handle::JobReceipt,
	},
};

/// Split a local video into shots. Omitted parameters take the defaults in
/// `DetectParams`.
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct ShotDetectInput {
	pub path: SdPath,
	pub threshold: Option<f64>,
	pub min_shot_seconds: Option<f64>,
}

pub struct ShotDetectAction {
	path: PathBuf,
	params: DetectParams,
}

impl LibraryAction for ShotDetectAction {
	type Input = ShotDetectInput;
	type Output = JobReceipt;

	fn from_input(input: Self::Input) -> Result<Self, String> {
		let path = local_path(&input.path)?;
		let defaults = DetectParams::default();
		let params = DetectParams {
			threshold: input.threshold.unwrap_or(defaults.threshold),
			min_shot_seconds: input.min_shot_seconds.unwrap_or(defaults.min_shot_seconds),
		};
		params.validate()?;
		Ok(Self { path, params })
	}

	async fn execute(
		self,
		library: Arc<crate::library::Library>,
		_context: Arc<CoreContext>,
	) -> Result<Self::Output, ActionError> {
		let job_handle = library
			.jobs()
			.dispatch(ShotDetectionJob::new(self.path, self.params))
			.await
			.map_err(ActionError::Job)?;
		Ok(job_handle.into())
	}

	fn action_kind(&self) -> &'static str {
		"media.shots.detect"
	}
}

/// Shot detection reads the file on this device, so it has to be here.
pub(super) fn local_path(path: &SdPath) -> Result<PathBuf, String> {
	match path {
		SdPath::Physical { device_slug, path }
			if *device_slug == crate::device::get_current_device_slug() =>
		{
			Ok(path.clone())
		}
		SdPath::Physical { device_slug, .. } => {
			Err(format!("{device_slug} does not name this device"))
		}
		other => Err(format!("{other:?} does not name a local file")),
	}
}

crate::register_library_action!(ShotDetectAction, "media.shots.detect");
