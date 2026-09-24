//! Archive and extract action handlers

use std::sync::Arc;

use super::{
	input::{FileArchiveInput, FileExtractInput},
	job::{ArchiveJob, ExtractJob},
};
use crate::{
	context::CoreContext,
	infra::{
		action::{error::ActionError, LibraryAction},
		job::handle::JobReceipt,
	},
};

pub struct FileArchiveAction {
	input: FileArchiveInput,
}

impl LibraryAction for FileArchiveAction {
	type Input = FileArchiveInput;
	type Output = JobReceipt;

	fn from_input(input: Self::Input) -> Result<Self, String> {
		input.validate().map_err(|errors| errors.join("; "))?;
		Ok(Self { input })
	}

	async fn execute(
		self,
		library: Arc<crate::library::Library>,
		_context: Arc<CoreContext>,
	) -> Result<Self::Output, ActionError> {
		let job_handle = library
			.jobs()
			.dispatch(ArchiveJob::new(self.input))
			.await
			.map_err(ActionError::Job)?;
		Ok(job_handle.into())
	}

	fn action_kind(&self) -> &'static str {
		"files.archive"
	}
}

crate::register_library_action!(FileArchiveAction, "files.archive");

pub struct FileExtractAction {
	input: FileExtractInput,
}

impl LibraryAction for FileExtractAction {
	type Input = FileExtractInput;
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
			.dispatch(ExtractJob::new(self.input))
			.await
			.map_err(ActionError::Job)?;
		Ok(job_handle.into())
	}

	fn action_kind(&self) -> &'static str {
		"files.extract"
	}
}

crate::register_library_action!(FileExtractAction, "files.extract");
