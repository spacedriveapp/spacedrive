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
use std::{path::PathBuf, sync::Arc};
use tracing::{debug, info};

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

		Ok(ValidationResult::Success { metadata: None })
	}

	async fn execute(
		self,
		library: Arc<crate::library::Library>,
		context: Arc<CoreContext>,
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
			SdPath::Cloud {
				service,
				identifier,
				path,
			} => {
				// Resolve the CloudBackend via the tracked volume that owns this
				// (service, identifier) pair. The backend was wired up when the
				// volume was added or rehydrated from the database, so there is
				// no need to re-authenticate or rebuild the operator here.
				let volume = context
					.volume_manager
					.find_cloud_volume(*service, identifier)
					.await
					.ok_or_else(|| {
						ActionError::Internal(format!(
							"No cloud volume found for {}://{}",
							service.scheme(),
							identifier
						))
					})?;

				let backend = volume.backend.as_ref().ok_or_else(|| {
					ActionError::Internal(format!(
						"Cloud volume {}://{} has no backend attached",
						service.scheme(),
						identifier
					))
				})?;

				// OpenDAL's `create_dir` is idempotent on trailing-slash paths
				// for every provider in our matrix (S3 no-op, OneDrive/Gdrive
				// return 200 on existing folder), so double-create does not
				// need a pre-existence check at this layer.
				backend
					.create_directory(&PathBuf::from(path), false)
					.await
					.map_err(|e| {
						ActionError::Internal(format!("Failed to create cloud folder: {}", e))
					})?;

				info!(
					volume_id = %volume.id,
					path = %path,
					service = %service.scheme(),
					"created cloud folder",
				);
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

	/// Regression: the Physical branch of `execute` must keep using
	/// [`LocalBackend::create_directory`] after the Cloud branch was
	/// reworked in Set 2. Building a full `CoreContext` is heavier than
	/// this test needs, so we exercise `LocalBackend::create_directory`
	/// directly, mirroring the exact call the Physical branch makes after
	/// resolving the parent as the backend root.
	#[tokio::test]
	async fn test_create_folder_local_still_works() {
		let parent = std::env::temp_dir().join(format!(
			"sd-create-folder-regression-{}",
			uuid::Uuid::new_v4()
		));
		tokio::fs::create_dir_all(&parent)
			.await
			.expect("set up parent directory");
		let target = parent.join("new_folder");

		let backend = LocalBackend::new(&parent);
		backend
			.create_directory(&target, false)
			.await
			.expect("local create_directory should succeed");

		let meta = tokio::fs::metadata(&target)
			.await
			.expect("created directory must exist");
		assert!(meta.is_dir());

		tokio::fs::remove_dir_all(&parent).await.ok();
	}
}
