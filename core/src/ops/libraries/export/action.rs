//! Library export action handler

use super::input::LibraryExportInput;
use crate::{
	context::CoreContext,
	infra::action::{error::ActionError, LibraryAction},
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LibraryExportAction {
	input: LibraryExportInput,
}

impl LibraryExportAction {
	/// Create a new library export action
	pub fn new(input: LibraryExportInput) -> Self {
		Self { input }
	}
}

// Implement LibraryAction
impl LibraryAction for LibraryExportAction {
	type Input = LibraryExportInput;
	type Output = super::output::LibraryExportOutput;

	fn from_input(input: LibraryExportInput) -> Result<Self, String> {
		Ok(LibraryExportAction::new(input))
	}

	async fn execute(
		self,
		library: std::sync::Arc<crate::library::Library>,
		_context: Arc<CoreContext>,
	) -> Result<Self::Output, ActionError> {
		// Ensure parent directory exists
		if let Some(parent) = self.input.export_path.parent() {
			if !parent.exists() {
				return Err(ActionError::Validation {
					field: "export_path".to_string(),
					message: "Export directory does not exist".to_string(),
				});
			}
		}

		// Create export directory
		let export_dir = &self.input.export_path;
		tokio::fs::create_dir_all(&export_dir).await.map_err(|e| {
			ActionError::Internal(format!("Failed to create export directory: {}", e))
		})?;

		// Export library config
		let config = library.config().await;
		let config_path = export_dir.join("library.json");
		let config_json = serde_json::to_string_pretty(&config)
			.map_err(|e| ActionError::Internal(format!("Failed to serialize config: {}", e)))?;
		tokio::fs::write(&config_path, config_json)
			.await
			.map_err(|e| ActionError::Internal(format!("Failed to write config: {}", e)))?;

		// Export database (as SQL dump for portability)
		// TODO: Implement actual database export
		let db_export_path = export_dir.join("database.sql");
		tokio::fs::write(&db_export_path, "-- Database export not yet implemented")
			.await
			.map_err(|e| {
				ActionError::Internal(format!("Failed to write database export: {}", e))
			})?;

		let mut exported_files = vec![
			config_path.to_string_lossy().to_string(),
			db_export_path.to_string_lossy().to_string(),
		];

		// Optionally export thumbnails
		if self.input.include_thumbnails {
			let thumbnails_src = library.path().join("thumbnails");
			if thumbnails_src.exists() {
				// TODO: Copy thumbnails directory
				exported_files.push("thumbnails/".to_string());
			}
		}

		// Optionally export previews
		if self.input.include_previews {
			let previews_src = library.path().join("previews");
			if previews_src.exists() {
				// TODO: Copy previews directory
				exported_files.push("previews/".to_string());
			}
		}

		Ok(super::output::LibraryExportOutput {
			library_id: library.id(),
			library_name: config.name.clone(),
			export_path: self.input.export_path,
			exported_files,
		})
	}

	fn action_kind(&self) -> &'static str {
		"library.export"
	}
}
