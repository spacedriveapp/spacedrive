//! Location add operation output types

use crate::infrastructure::actions::output::ActionOutputTrait;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use uuid::Uuid;

/// Output from location add action dispatch
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocationAddOutput {
    pub location_id: Uuid,
    pub path: PathBuf,
    pub name: Option<String>,
}

impl LocationAddOutput {
    pub fn new(location_id: Uuid, path: PathBuf, name: Option<String>) -> Self {
        Self {
            location_id,
            path,
            name,
        }
    }
}

impl ActionOutputTrait for LocationAddOutput {
    fn to_json(&self) -> serde_json::Value {
        serde_json::to_value(self).unwrap_or(serde_json::Value::Null)
    }
    
    fn display_message(&self) -> String {
        match &self.name {
            Some(name) => format!(
                "Added location '{}' with ID {} at {}",
                name, self.location_id, self.path.display()
            ),
            None => format!(
                "Added location with ID {} at {}",
                self.location_id, self.path.display()
            ),
        }
    }
    
    fn output_type(&self) -> &'static str {
        "location.add.completed"
    }
}