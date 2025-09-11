//! File delete operation output types

use crate::infra::action::output::ActionOutputTrait;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Output from file delete action dispatch
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileDeleteOutput {
    pub job_id: Uuid,
    pub targets_count: usize,
}

impl FileDeleteOutput {
    pub fn new(job_id: Uuid, targets_count: usize) -> Self {
        Self {
            job_id,
            targets_count,
        }
    }
}

impl ActionOutputTrait for FileDeleteOutput {
    fn to_json(&self) -> serde_json::Value {
        serde_json::to_value(self).unwrap_or(serde_json::Value::Null)
    }

    fn display_message(&self) -> String {
        format!(
            "Dispatched file delete job {} for {} file(s)",
            self.job_id, self.targets_count
        )
    }

    fn output_type(&self) -> &'static str {
        "file.delete.dispatched"
    }
}