//! Rename action handlers

use std::sync::Arc;

use super::{
	input::{FileRenameBatchInput, FileRenameInput},
	job::RenameJob,
	naming::check_portable,
};
use crate::{
	context::CoreContext,
	domain::addressing::SdPath,
	infra::{
		action::{error::ActionError, LibraryAction},
		job::handle::JobReceipt,
	},
};
use serde::{Deserialize, Serialize};

/// Rename one file or directory.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileRenameAction {
	pub target: SdPath,
	pub new_name: String,
}

impl FileRenameAction {
	pub fn new(target: SdPath, new_name: impl Into<String>) -> Self {
		Self {
			target,
			new_name: new_name.into(),
		}
	}
}

impl LibraryAction for FileRenameAction {
	type Input = FileRenameInput;
	type Output = JobReceipt;

	fn from_input(input: Self::Input) -> Result<Self, String> {
		check_portable(&input.new_name).map_err(|e| e.to_string())?;
		refuse_unnameable(&input.target)?;
		Ok(FileRenameAction {
			target: input.target,
			new_name: input.new_name,
		})
	}

	async fn execute(
		self,
		library: Arc<crate::library::Library>,
		_context: Arc<CoreContext>,
	) -> Result<Self::Output, ActionError> {
		let job_handle = library
			.jobs()
			.dispatch(RenameJob::named(self.target, self.new_name))
			.await
			.map_err(ActionError::Job)?;
		Ok(job_handle.into())
	}

	fn action_kind(&self) -> &'static str {
		"files.rename"
	}
}

crate::register_library_action!(FileRenameAction, "files.rename");

/// Rename several files by rules.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileRenameBatchAction {
	input: FileRenameBatchInput,
}

impl LibraryAction for FileRenameBatchAction {
	type Input = FileRenameBatchInput;
	type Output = JobReceipt;

	fn from_input(input: Self::Input) -> Result<Self, String> {
		input.validate().map_err(|errors| errors.join("; "))?;
		for target in &input.targets {
			refuse_unnameable(target)?;
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
			.dispatch(RenameJob::ruled(self.input.targets, self.input.rules))
			.await
			.map_err(ActionError::Job)?;
		Ok(job_handle.into())
	}

	fn action_kind(&self) -> &'static str {
		"files.rename_batch"
	}
}

crate::register_library_action!(FileRenameBatchAction, "files.rename_batch");

fn refuse_unnameable(target: &SdPath) -> Result<(), String> {
	match target {
		SdPath::Content { .. } => Err("Cannot rename content-addressed files directly".to_string()),
		SdPath::Sidecar { .. } => Err("Cannot rename sidecar files directly".to_string()),
		_ => Ok(()),
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn a_structurally_broken_name_never_reaches_a_job() {
		let target = SdPath::local(std::path::PathBuf::from("/test/file.txt"));
		assert!(FileRenameAction::from_input(FileRenameInput::new(target.clone(), "a/b")).is_err());
		assert!(FileRenameAction::from_input(FileRenameInput::new(target, "b.txt")).is_ok());
	}
}
