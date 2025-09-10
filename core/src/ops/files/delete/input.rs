//! Input types for file deletion operations

use crate::register_library_action_input;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Input for deleting files
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileDeleteInput {
	/// The library ID where this operation takes place
	pub library_id: Option<uuid::Uuid>,

	/// Files or directories to delete
	pub targets: Vec<PathBuf>,

	/// Whether to permanently delete (true) or move to trash (false)
	pub permanent: bool,

	/// Whether to delete directories recursively
	pub recursive: bool,
}

impl crate::client::Wire for FileDeleteInput {
	const METHOD: &'static str = "action:files.delete.input.v1";
}

impl crate::ops::registry::BuildLibraryActionInput for FileDeleteInput {
	type Action = crate::ops::files::delete::action::FileDeleteAction;

	fn build(
		self,
		session: &crate::infra::daemon::state::SessionState,
	) -> Result<Self::Action, String> {
		use crate::ops::files::delete::job::DeleteOptions;

		let library_id = self
			.library_id
			.or(session.current_library_id)
			.ok_or("No library ID provided and no current library set")?;

		Ok(crate::ops::files::delete::action::FileDeleteAction {
			library_id,
			targets: self.targets,
			options: DeleteOptions {
				permanent: self.permanent,
				recursive: self.recursive,
			},
		})
	}
}

register_library_action_input!(FileDeleteInput);

impl FileDeleteInput {
	/// Create a new file deletion input
	pub fn new(targets: Vec<PathBuf>) -> Self {
		Self {
			library_id: None,
			targets,
			permanent: false,
			recursive: true,
		}
	}

	/// Set the library ID
	pub fn with_library_id(mut self, library_id: uuid::Uuid) -> Self {
		self.library_id = Some(library_id);
		self
	}

	/// Set permanent deletion
	pub fn with_permanent(mut self, permanent: bool) -> Self {
		self.permanent = permanent;
		self
	}

	/// Set recursive deletion
	pub fn with_recursive(mut self, recursive: bool) -> Self {
		self.recursive = recursive;
		self
	}

	/// Validate the input
	pub fn validate(&self) -> Result<(), Vec<String>> {
		let mut errors = Vec::new();

		if self.targets.is_empty() {
			errors.push("At least one target file must be specified".to_string());
		}

		if errors.is_empty() {
			Ok(())
		} else {
			Err(errors)
		}
	}
}
