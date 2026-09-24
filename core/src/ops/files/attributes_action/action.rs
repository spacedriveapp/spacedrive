use std::sync::Arc;

use serde::{Deserialize, Serialize};
use specta::Type;

use super::job::AttributesJob;
use crate::{
	context::CoreContext,
	domain::SdPath,
	infra::{
		action::{error::ActionError, LibraryAction},
		job::{handle::JobReceipt, journal::Attributes},
	},
};

/// Set attributes on files; each absent attribute stays as it is.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct FileSetAttributesInput {
	pub paths: Vec<SdPath>,
	pub attributes: Attributes,
}

impl FileSetAttributesInput {
	pub fn validate(&self) -> Result<(), Vec<String>> {
		let mut errors = Vec::new();
		if self.paths.is_empty() {
			errors.push("name at least one file".to_string());
		}
		if self.attributes.mode.is_none()
			&& self.attributes.modified_ms.is_none()
			&& self.attributes.hidden.is_none()
		{
			errors.push("name at least one attribute to set".to_string());
		}
		if self.attributes.mode.is_some_and(|mode| mode > 0o7777) {
			errors.push("a mode is at most 0o7777".to_string());
		}
		if errors.is_empty() {
			Ok(())
		} else {
			Err(errors)
		}
	}
}

pub struct FileSetAttributesAction {
	input: FileSetAttributesInput,
}

impl LibraryAction for FileSetAttributesAction {
	type Input = FileSetAttributesInput;
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
			.dispatch(AttributesJob::new(self.input))
			.await
			.map_err(ActionError::Job)?;
		Ok(job_handle.into())
	}

	fn action_kind(&self) -> &'static str {
		"files.set_attributes"
	}
}

crate::register_library_action!(FileSetAttributesAction, "files.set_attributes");
