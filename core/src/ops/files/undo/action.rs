//! Undo action handler

use std::sync::Arc;

use super::{input::FileUndoInput, job::UndoJob};
use crate::{
	context::CoreContext,
	infra::{
		action::{error::ActionError, LibraryAction},
		job::handle::JobReceipt,
	},
};

pub struct FileUndoAction {
	input: FileUndoInput,
}

impl LibraryAction for FileUndoAction {
	type Input = FileUndoInput;
	type Output = JobReceipt;

	fn from_input(input: Self::Input) -> Result<Self, String> {
		if input.effects.as_ref().is_some_and(Vec::is_empty) {
			return Err("name at least one effect, or none for the whole job".to_string());
		}
		Ok(Self { input })
	}

	async fn execute(
		self,
		library: Arc<crate::library::Library>,
		_context: Arc<CoreContext>,
	) -> Result<Self::Output, ActionError> {
		let job_handle = library
			.jobs()
			.dispatch(UndoJob::new(self.input.job, self.input.effects))
			.await
			.map_err(ActionError::Job)?;
		Ok(job_handle.into())
	}

	fn action_kind(&self) -> &'static str {
		"files.undo"
	}
}

crate::register_library_action!(FileUndoAction, "files.undo");
