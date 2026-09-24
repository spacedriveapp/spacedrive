use std::sync::Arc;

use serde::{Deserialize, Serialize};
use specta::Type;
use uuid::Uuid;

use crate::{
	context::CoreContext,
	infra::{
		action::{error::ActionError, LibraryAction},
		job::{journal::Effect, types::JobId},
	},
	ops::files::trash,
};

/// Remove for good what the journals put in the trash, the Spacedrive
/// trash directories, and the platform's trash when asked.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct FileTrashEmptyInput {
	/// Empty the platform's own trash as well, everything in it.
	#[serde(default)]
	pub os_trash: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, Type)]
pub struct TrashEmptyOutput {
	/// Items the journals named that were removed.
	pub purged: u64,
	/// Items removed from Spacedrive trash directories beyond those.
	pub spacedrive_trash: u64,
	pub os_trash_emptied: bool,
	pub failed: Vec<String>,
}

pub struct FileTrashEmptyAction {
	input: FileTrashEmptyInput,
}

impl LibraryAction for FileTrashEmptyAction {
	type Input = FileTrashEmptyInput;
	type Output = TrashEmptyOutput;

	fn from_input(input: Self::Input) -> Result<Self, String> {
		Ok(Self { input })
	}

	async fn execute(
		self,
		library: Arc<crate::library::Library>,
		context: Arc<CoreContext>,
	) -> Result<Self::Output, ActionError> {
		let db = library.jobs().database();
		let trashed = db
			.trashed()
			.await
			.map_err(|error| ActionError::Internal(error.to_string()))?;
		let mut output = TrashEmptyOutput::default();
		let mut forget: std::collections::HashMap<Uuid, Vec<i64>> =
			std::collections::HashMap::new();
		for (job, recorded) in trashed {
			let Effect::Trashed {
				to: Some(location), ..
			} = &recorded.effect
			else {
				continue;
			};
			let Ok(job) = job.parse::<Uuid>() else {
				continue;
			};
			match trash::purge(location).await {
				Ok(()) => {
					output.purged += 1;
					forget.entry(job).or_default().push(recorded.sequence);
				}
				Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
					forget.entry(job).or_default().push(recorded.sequence);
				}
				Err(error) => output
					.failed
					.push(format!("{}: {error}", location.display())),
			}
		}
		for (job, sequences) in forget {
			if let Err(error) = db.forget_trashed(JobId(job), &sequences).await {
				output.failed.push(format!("journal of {job}: {error}"));
			}
		}
		match trash::empty_spacedrive_trash(&context.volume_manager).await {
			Ok(removed) => output.spacedrive_trash = removed,
			Err(error) => output.failed.push(error.to_string()),
		}
		if self.input.os_trash {
			match trash::empty_os_trash().await {
				Ok(()) => output.os_trash_emptied = true,
				Err(error) => output.failed.push(error.to_string()),
			}
		}
		Ok(output)
	}

	fn action_kind(&self) -> &'static str {
		"files.trash_empty"
	}
}

crate::register_library_action!(FileTrashEmptyAction, "files.trash_empty");
