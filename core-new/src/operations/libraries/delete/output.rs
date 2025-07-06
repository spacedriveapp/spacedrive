//! Library delete operation output types

use crate::infrastructure::actions::output::ActionOutputTrait;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Output from library delete action dispatch
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LibraryDeleteOutput {
    pub library_id: Uuid,
    pub name: String,
}

impl LibraryDeleteOutput {
    pub fn new(library_id: Uuid, name: String) -> Self {
        Self {
            library_id,
            name,
        }
    }
}

impl ActionOutputTrait for LibraryDeleteOutput {
    fn to_json(&self) -> serde_json::Value {
        serde_json::to_value(self).unwrap_or(serde_json::Value::Null)
    }
    
    fn display_message(&self) -> String {
        format!(
            "Deleted library '{}' with ID {}",
            self.name, self.library_id
        )
    }
    
    fn output_type(&self) -> &'static str {
        "library.delete.completed"
    }
}