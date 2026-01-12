//! Library open action handler

use super::{input::LibraryOpenInput, output::LibraryOpenOutput};
use crate::{
	context::CoreContext,
	infra::action::{error::ActionError, CoreAction},
	library::LibraryError,
};
use async_trait::async_trait;
use std::path::PathBuf;
use std::sync::Arc;
use tracing::info;
use uuid::Uuid;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct LibraryOpenAction {
	input: LibraryOpenInput,
}

impl LibraryOpenAction {
	pub fn new(input: LibraryOpenInput) -> Self {
		Self { input }
	}
}

impl CoreAction for LibraryOpenAction {
	type Input = LibraryOpenInput;
	type Output = LibraryOpenOutput;

	fn from_input(input: LibraryOpenInput) -> Result<Self, String> {
		Ok(LibraryOpenAction::new(input))
	}

	async fn execute(self, context: Arc<CoreContext>) -> Result<Self::Output, ActionError> {
		let library_manager = context.libraries().await;

		// Open the library
		let library = library_manager
			.open_library(&self.input.path, context.clone())
			.await
			.map_err(|e| match e {
				LibraryError::AlreadyOpen(id) => ActionError::Validation {
					field: "path".to_string(),
					message: format!("Library {} is already open", id),
				},
				LibraryError::NotALibrary(path) => ActionError::Validation {
					field: "path".to_string(),
					message: format!("Path {:?} is not a valid library directory", path),
				},
				other => ActionError::Internal(other.to_string()),
			})?;

		// Initialize sidecar manager for the opened library
		if let Err(e) = context
			.get_sidecar_manager()
			.await
			.ok_or_else(|| ActionError::Internal("Sidecar manager not available".to_string()))?
			.init_library(&library)
			.await
		{
			tracing::error!(
				"Failed to initialize sidecar manager for library {}: {}",
				library.id(),
				e
			);
		}

		info!("Opened library {} from {:?}", library.id(), self.input.path);

		// Get the library details
		let library_id = library.id();
		let name = library.name().await;
		let path = library.path().to_path_buf();

		Ok(LibraryOpenOutput::new(library_id, name, path))
	}

	fn action_kind(&self) -> &'static str {
		"library.open"
	}

	async fn validate(
		&self,
		_context: Arc<CoreContext>,
	) -> Result<crate::infra::action::ValidationResult, ActionError> {
		// Check if the path exists
		if !self.input.path.exists() {
			return Err(ActionError::Validation {
				field: "path".to_string(),
				message: format!("Path {:?} does not exist", self.input.path),
			});
		}

		// Check if it's a valid library directory
		if !self
			.input
			.path
			.extension()
			.and_then(|e| e.to_str())
			.map(|e| e == "sdlibrary")
			.unwrap_or(false)
		{
			return Err(ActionError::Validation {
				field: "path".to_string(),
				message: format!(
					"Path {:?} is not a library directory (.sdlibrary)",
					self.input.path
				),
			});
		}

		// Check if library.json exists
		let config_path = self.input.path.join("library.json");
		if !config_path.exists() {
			return Err(ActionError::Validation {
				field: "path".to_string(),
				message: format!("Library configuration not found at {:?}", config_path),
			});
		}

		Ok(crate::infra::action::ValidationResult::Success { metadata: None })
	}
}

// Register core action
crate::register_core_action!(LibraryOpenAction, "libraries.open");
