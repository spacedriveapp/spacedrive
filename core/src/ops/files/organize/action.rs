//! Organize and flatten action handlers

use std::sync::Arc;

use super::{
	input::{FileFlattenInput, FileOrganizeInput},
	job::{Rearrange, RearrangeJob},
};
use crate::{
	context::CoreContext,
	infra::{
		action::{error::ActionError, LibraryAction},
		job::handle::JobReceipt,
	},
};

pub struct FileOrganizeAction {
	input: FileOrganizeInput,
}

impl LibraryAction for FileOrganizeAction {
	type Input = FileOrganizeInput;
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
			.dispatch(RearrangeJob::new(Rearrange::Organize(self.input)))
			.await
			.map_err(ActionError::Job)?;
		Ok(job_handle.into())
	}

	fn action_kind(&self) -> &'static str {
		"files.organize"
	}
}

crate::register_library_action!(FileOrganizeAction, "files.organize");

pub struct FileFlattenAction {
	input: FileFlattenInput,
}

impl LibraryAction for FileFlattenAction {
	type Input = FileFlattenInput;
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
			.dispatch(RearrangeJob::new(Rearrange::Flatten(self.input)))
			.await
			.map_err(ActionError::Job)?;
		Ok(job_handle.into())
	}

	fn action_kind(&self) -> &'static str {
		"files.flatten"
	}
}

crate::register_library_action!(FileFlattenAction, "files.flatten");
