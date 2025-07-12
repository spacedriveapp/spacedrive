//! Library export operation output

use crate::infrastructure::actions::output::ActionOutputTrait;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LibraryExportOutput {
    pub library_id: Uuid,
    pub library_name: String,
    pub export_path: PathBuf,
    pub exported_files: Vec<String>,
}

impl ActionOutputTrait for LibraryExportOutput {
    fn to_json(&self) -> serde_json::Value {
        serde_json::to_value(self).unwrap_or(serde_json::Value::Null)
    }

    fn display_message(&self) -> String {
        format!(
            "Exported library '{}' to {} ({} files)",
            self.library_name,
            self.export_path.display(),
            self.exported_files.len()
        )
    }

    fn output_type(&self) -> &'static str {
        "library.export.output"
    }
}