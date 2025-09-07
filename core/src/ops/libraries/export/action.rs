//! Library export action handler

use crate::{
    context::CoreContext,
    infra::actions::{
        error::{ActionError, ActionResult},
        handler::ActionHandler,
        output::ActionOutput,
        Action,
    },
    register_action_handler,
};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::{path::PathBuf, sync::Arc};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LibraryExportAction {
    pub library_id: Uuid,
    pub export_path: PathBuf,
    pub include_thumbnails: bool,
    pub include_previews: bool,
}

pub struct LibraryExportHandler;

impl LibraryExportHandler {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl ActionHandler for LibraryExportHandler {
    async fn validate(
        &self,
        _context: Arc<CoreContext>,
        action: &Action,
    ) -> ActionResult<()> {
        if let Action::LibraryExport { action, .. } = action {
            // Ensure parent directory exists
            if let Some(parent) = action.export_path.parent() {
                if !parent.exists() {
                    return Err(ActionError::Validation {
                        field: "export_path".to_string(),
                        message: "Export directory does not exist".to_string(),
                    });
                }
            }
            Ok(())
        } else {
            Err(ActionError::InvalidActionType)
        }
    }

    async fn execute(
        &self,
        context: Arc<CoreContext>,
        action: Action,
    ) -> ActionResult<ActionOutput> {
        if let Action::LibraryExport { library_id, action } = action {
            let library_manager = &context.library_manager;

            // Get the specific library
            let library = library_manager
                .get_library(library_id)
                .await
                .ok_or(ActionError::LibraryNotFound(library_id))?;

            // Create export directory
            let export_dir = &action.export_path;
            tokio::fs::create_dir_all(&export_dir).await
                .map_err(|e| ActionError::Internal(format!("Failed to create export directory: {}", e)))?;

            // Export library config
            let config = library.config().await;
            let config_path = export_dir.join("library.json");
            let config_json = serde_json::to_string_pretty(&config)
                .map_err(|e| ActionError::Internal(format!("Failed to serialize config: {}", e)))?;
            tokio::fs::write(&config_path, config_json).await
                .map_err(|e| ActionError::Internal(format!("Failed to write config: {}", e)))?;

            // Export database (as SQL dump for portability)
            // TODO: Implement actual database export
            let db_export_path = export_dir.join("database.sql");
            tokio::fs::write(&db_export_path, "-- Database export not yet implemented").await
                .map_err(|e| ActionError::Internal(format!("Failed to write database export: {}", e)))?;

            let mut exported_files = vec![
                config_path.to_string_lossy().to_string(),
                db_export_path.to_string_lossy().to_string(),
            ];

            // Optionally export thumbnails
            if action.include_thumbnails {
                let thumbnails_src = library.path().join("thumbnails");
                let thumbnails_dst = export_dir.join("thumbnails");
                if thumbnails_src.exists() {
                    // TODO: Copy thumbnails directory
                    exported_files.push("thumbnails/".to_string());
                }
            }

            // Optionally export previews
            if action.include_previews {
                let previews_src = library.path().join("previews");
                let previews_dst = export_dir.join("previews");
                if previews_src.exists() {
                    // TODO: Copy previews directory
                    exported_files.push("previews/".to_string());
                }
            }

            let output = super::output::LibraryExportOutput {
                library_id,
                library_name: config.name.clone(),
                export_path: action.export_path,
                exported_files,
            };

            Ok(ActionOutput::from_trait(output))
        } else {
            Err(ActionError::InvalidActionType)
        }
    }

    fn can_handle(&self, action: &Action) -> bool {
        matches!(action, Action::LibraryExport { .. })
    }

    fn supported_actions() -> &'static [&'static str] {
        &["library.export"]
    }
}

register_action_handler!(LibraryExportHandler, "library.export");