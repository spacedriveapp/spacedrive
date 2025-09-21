//! Location add operation output types

use crate::infra::action::output::ActionOutputTrait;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use uuid::Uuid;

/// Output from location add action dispatch
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct LocationAddOutput {
    pub location_id: Uuid,
    pub path: PathBuf,
    pub name: Option<String>,
    pub job_id: Option<Uuid>,
}

impl LocationAddOutput {
    pub fn new(location_id: Uuid, path: PathBuf, name: Option<String>) -> Self {
        Self {
            location_id,
            path,
            name,
            job_id: None,
        }
    }

    pub fn with_job_id(mut self, job_id: Uuid) -> Self {
        self.job_id = Some(job_id);
        self
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