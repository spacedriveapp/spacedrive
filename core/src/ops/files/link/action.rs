use std::sync::Arc;

use serde::{Deserialize, Serialize};
use specta::Type;

use super::job::LinkJob;
use crate::{
	context::CoreContext,
	domain::SdPath,
	infra::{
		action::{error::ActionError, LibraryAction},
		job::handle::JobReceipt,
	},
};

/// Make a link at `at` to `target`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct FileLinkInput {
	pub at: SdPath,
	pub target: SdPath,
	pub kind: LinkKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "cli", derive(clap::ValueEnum))]
pub enum LinkKind {
	Symlink,
	Hardlink,
}

pub struct FileLinkAction {
	input: FileLinkInput,
}

impl LibraryAction for FileLinkAction {
	type Input = FileLinkInput;
	type Output = JobReceipt;

	fn from_input(input: Self::Input) -> Result<Self, String> {
		Ok(Self { input })
	}

	async fn execute(
		self,
		library: Arc<crate::library::Library>,
		_context: Arc<CoreContext>,
	) -> Result<Self::Output, ActionError> {
		let job_handle = library
			.jobs()
			.dispatch(LinkJob::new(self.input))
			.await
			.map_err(ActionError::Job)?;
		Ok(job_handle.into())
	}

	fn action_kind(&self) -> &'static str {
		"files.link"
	}
}

crate::register_library_action!(FileLinkAction, "files.link");
