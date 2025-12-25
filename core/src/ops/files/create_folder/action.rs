//! Create folder action handler

use super::input::CreateFolderInput;
use super::output::CreateFolderOutput;
use crate::{
	context::CoreContext,
	domain::addressing::{SdPath, SdPathBatch},
	infra::action::{error::ActionError, LibraryAction, ValidationResult},
	ops::files::{
		copy::job::{FileCopyJob, MoveMode},
		rename::validation::validate_filename,
	},
	volume::{LocalBackend, VolumeBackend},
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::debug;

/// Action for creating a new folder
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateFolderAction {
	/// Parent directory where the folder will be created
	pub parent: SdPath,
	/// Name for the new folder
	pub name: String,
	/// Optional items to move into the new folder after creation
	pub items: Vec<SdPath>,
}

impl CreateFolderAction {
	/// Create a new folder action
	pub fn new(parent: SdPath, name: impl Into<String>) -> Self {
		Self {
			parent,
			name: name.into(),
			items: Vec::new(),
		}
	}

	/// Create a folder action with items to move
	pub fn with_items(parent: SdPath, name: impl Into<String>, items: Vec<SdPath>) -> Self {
		Self {
			parent,
			name: name.into(),
			items,
		}
	}
}

impl LibraryAction for CreateFolderAction {
	type Input = CreateFolderInput;
	type Output = CreateFolderOutput;

	fn from_input(input: Self::Input) -> Result<Self, String> {
		Ok(CreateFolderAction {
			parent: input.parent,
			name: input.name,
			items: input.items,
		})
	}

	async fn validate(
		&self,
		_library: &Arc<crate::library::Library>,
		_context: Arc<CoreContext>,
	) -> Result<ValidationResult, ActionError> {
		// Validate folder name
		validate_filename(&self.name).map_err(|e| ActionError::Validation {
			field: "name".to_string(),
			message: e.to_string(),
		})?;

		// Validate parent is a physical or cloud path (not Content/Sidecar)
		match &self.parent {
			SdPath::Physical { .. } | SdPath::Cloud { .. } => {}
			SdPath::Content { .. } => {
				return Err(ActionError::Validation {
					field: "parent".to_string(),
					message: "Cannot create folders in content-addressed storage".to_string(),
				});
			}
			SdPath::Sidecar { .. } => {
				return Err(ActionError::Validation {
					field: "parent".to_string(),
					message: "Cannot create folders in sidecar storage".to_string(),
				});
			}
		}

		Ok(ValidationResult::Success)
	}

	async fn execute(
		self,
		library: Arc<crate::library::Library>,
		_context: Arc<CoreContext>,
	) -> Result<Self::Output, ActionError> {
		// Construct the destination folder path
		let folder_path = self.parent.join(&self.name);

		debug!(
			"Creating folder: {} in parent: {}",
			self.name,
			self.parent.display()
		);

		// Create the directory based on path type
		match &folder_path {
			SdPath::Physical { path, .. } => {
				// Use LocalBackend to create the directory
				let backend = LocalBackend::new(path.parent().unwrap_or(path));
				backend.create_directory(path, false).await.map_err(|e| {
					ActionError::Internal(format!("Failed to create directory: {}", e))
				})?;
			}
			SdPath::Cloud { .. } => {
				// Cloud folder creation would use CloudBackend
				// For now, return an error as cloud support needs more infrastructure
				return Err(ActionError::Internal(
					"Cloud folder creation not yet implemented".to_string(),
				));
			}
			_ => {
				return Err(ActionError::Internal(
					"Unexpected path type after validation".to_string(),
				));
			}
		}

		// If items were provided, dispatch a move job
		if !self.items.is_empty() {
			debug!(
				"Moving {} items into new folder: {}",
				self.items.len(),
				folder_path.display()
			);

			let job = FileCopyJob::new_move(
				SdPathBatch::new(self.items),
				folder_path.clone(),
				MoveMode::Move,
			);

			let job_handle = library
				.jobs()
				.dispatch(job)
				.await
				.map_err(ActionError::Job)?;

			Ok(CreateFolderOutput::with_items(
				folder_path,
				job_handle.into(),
			))
		} else {
			Ok(CreateFolderOutput::without_items(folder_path))
		}
	}

	fn action_kind(&self) -> &'static str {
		"files.createFolder"
	}
}

// Register with the action-centric registry
crate::register_library_action!(CreateFolderAction, "files.createFolder");

#[cfg(test)]
mod tests {
	use super::*;
	use std::path::PathBuf;

	#[test]
	fn test_action_creation() {
		let parent = SdPath::local(PathBuf::from("/test"));
		let action = CreateFolderAction::new(parent, "new_folder");
		assert_eq!(action.name, "new_folder");
		assert!(action.items.is_empty());
	}

	#[test]
	fn test_action_with_items() {
		let parent = SdPath::local(PathBuf::from("/test"));
		let items = vec![
			SdPath::local(PathBuf::from("/test/file1.txt")),
			SdPath::local(PathBuf::from("/test/file2.txt")),
		];
		let action = CreateFolderAction::with_items(parent, "new_folder", items);
		assert_eq!(action.name, "new_folder");
		assert_eq!(action.items.len(), 2);
	}
}
