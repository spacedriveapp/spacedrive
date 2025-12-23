use super::{input::ResetDataInput, output::ResetDataOutput};
use crate::infra::action::{error::ActionError, CoreAction};
use std::sync::Arc;
use tracing::{error, info, warn};

pub struct ResetDataAction {
	input: ResetDataInput,
}

impl CoreAction for ResetDataAction {
	type Output = ResetDataOutput;
	type Input = ResetDataInput;

	fn from_input(input: Self::Input) -> std::result::Result<Self, String> {
		Ok(Self { input })
	}

	async fn execute(
		self,
		context: Arc<crate::context::CoreContext>,
	) -> std::result::Result<Self::Output, ActionError> {
		if !self.input.confirm {
			return Err(ActionError::InvalidInput(
				"Reset must be confirmed".to_string(),
			));
		}

		info!("Starting data reset operation");

		let data_dir = &context.data_dir;

		if !data_dir.exists() {
			warn!("Data directory does not exist: {:?}", data_dir);
			return Ok(ResetDataOutput {
				success: false,
				message: "Data directory does not exist".to_string(),
			});
		}

		info!("Resetting data directory: {:?}", data_dir);

		// Stop networking to release any file handles
		if let Some(networking) = context.get_networking().await {
			info!("Stopping networking service");
			if let Err(e) = networking.shutdown().await {
				warn!("Failed to shutdown networking: {}", e);
			}
		}

		// Close all libraries
		let library_manager = context.libraries().await;
		let libraries = library_manager.list().await;
		for library in libraries {
			info!("Closing library: {}", library.id());
			if let Err(e) = library_manager.close_library(library.id()).await {
				warn!("Failed to close library {}: {}", library.id(), e);
			}
		}

		// Give services time to shut down
		tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

		// Delete all files and directories in the data directory
		info!("Removing contents of data directory");
		match std::fs::read_dir(data_dir) {
			Ok(entries) => {
				for entry in entries.flatten() {
					let path = entry.path();
					info!("Removing: {:?}", path);
					let result = if path.is_dir() {
						std::fs::remove_dir_all(&path)
					} else {
						std::fs::remove_file(&path)
					};

					if let Err(e) = result {
						error!("Failed to remove {:?}: {}", path, e);
						return Err(ActionError::Internal(format!(
							"Failed to remove {:?}: {}",
							path, e
						)));
					}
				}
			}
			Err(e) => {
				error!("Failed to read data directory: {}", e);
				return Err(ActionError::Internal(format!(
					"Failed to read data directory: {}",
					e
				)));
			}
		}

		info!("Data reset completed successfully");

		Ok(ResetDataOutput {
			success: true,
			message: "All data has been reset. Please restart the app.".to_string(),
		})
	}

	fn action_kind(&self) -> &'static str {
		"core.reset"
	}
}

crate::register_core_action!(ResetDataAction, "core.reset");
