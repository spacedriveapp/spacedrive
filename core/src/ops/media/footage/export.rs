//! `media.footage.export`: a timeline of whole videos, cut at their shots.

use std::sync::Arc;

use serde::{Deserialize, Serialize};
use specta::Type;

use super::{local_footage_path, ExportMedia, TimelineFormat};
use crate::{
	context::CoreContext,
	domain::SdPath,
	infra::action::{error::ActionError, LibraryAction},
	library::Library,
};

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct FootageExportInput {
	/// Videos and folders of videos, in timeline order. A folder adds its
	/// videos at any depth in path order.
	pub paths: Vec<SdPath>,
	/// A folder on this device for the timeline files. A taken name gets a
	/// number.
	pub destination: SdPath,
	/// The project and file name.
	pub name: String,
	/// Every format when empty.
	#[serde(default)]
	pub formats: Vec<TimelineFormat>,
	#[serde(default)]
	pub media: ExportMedia,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct FootageExportOutput {
	/// The timeline files written, one per format.
	pub files: Vec<SdPath>,
	pub clips: u32,
	/// What an editor will do differently from what the timeline says.
	pub warnings: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct FootageExportAction {
	input: FootageExportInput,
}

impl LibraryAction for FootageExportAction {
	type Input = FootageExportInput;
	type Output = FootageExportOutput;

	fn from_input(input: Self::Input) -> Result<Self, String> {
		if input.paths.is_empty() {
			return Err("Choose at least one video to export".into());
		}
		for path in &input.paths {
			local_footage_path(path)?;
		}
		local_footage_path(&input.destination)?;
		Ok(Self { input })
	}

	async fn execute(
		self,
		library: Arc<Library>,
		context: Arc<CoreContext>,
	) -> Result<Self::Output, ActionError> {
		let capabilities = super::capabilities::capabilities(&context).await;
		if let Some(reason) = capabilities.export.reason {
			return Err(ActionError::InvalidInput(reason));
		}
		export(self.input, library, context).await
	}

	fn action_kind(&self) -> &'static str {
		"media.footage.export"
	}
}

crate::register_library_action!(FootageExportAction, "media.footage.export");

/// One clip per shot when shot detection has run on a video, so the editor
/// opens with the cuts already made, and the whole video otherwise.
#[cfg(feature = "footage-clips")]
async fn export(
	input: FootageExportInput,
	library: Arc<Library>,
	context: Arc<CoreContext>,
) -> Result<FootageExportOutput, ActionError> {
	use crate::ops::media::clips::{
		display_name, ExportClip, Footage, TimeInput, TimelineExportAction, TimelineExportInput,
	};

	let at = |frame: i64| TimeInput {
		frame: Some(frame),
		seconds: None,
	};
	let mut clips = Vec::new();
	for path in &input.paths {
		let path = local_footage_path(path).map_err(ActionError::InvalidInput)?;
		let videos = super::videos_at(&path)
			.await
			.map_err(|error| ActionError::InvalidInput(format!("{}: {error}", path.display())))?;
		for video in videos {
			let footage = Footage::load(&context, &library, &video)
				.await
				.map_err(ActionError::InvalidInput)?;
			let name = display_name(&video);
			match &footage.shots {
				Some(list) => clips.extend(list.shots.iter().map(|shot| ExportClip {
					path: SdPath::local(video.clone()),
					in_point: at(shot.in_point.frame as i64),
					out_point: at(shot.out_point.frame as i64),
					label: Some(format!("{name} shot {}", shot.index + 1)),
				})),
				None if footage.duration > 0 => clips.push(ExportClip {
					path: SdPath::local(video.clone()),
					in_point: at(0),
					out_point: at(footage.duration),
					label: Some(name),
				}),
				None => {
					return Err(ActionError::InvalidInput(format!(
						"{name} reports no length, so it cannot be placed on a timeline"
					)))
				}
			}
		}
	}
	if clips.is_empty() {
		return Err(ActionError::InvalidInput(
			"There are no videos in the selection".into(),
		));
	}
	let action = TimelineExportAction::from_input(TimelineExportInput {
		name: input.name,
		clips,
		formats: input.formats,
		media: input.media,
		destination: Some(input.destination),
	})
	.map_err(ActionError::InvalidInput)?;
	let output = action.execute(library, context).await?;
	Ok(FootageExportOutput {
		files: output.files.into_iter().map(|file| file.path).collect(),
		clips: output.timeline.clips,
		warnings: output.warnings,
	})
}

#[cfg(not(feature = "footage-clips"))]
async fn export(
	_input: FootageExportInput,
	_library: Arc<Library>,
	_context: Arc<CoreContext>,
) -> Result<FootageExportOutput, ActionError> {
	Err(ActionError::InvalidInput(
		"This build of Spacedrive cannot export timelines".into(),
	))
}
