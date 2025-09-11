//! Action execution receipts

use crate::infra::job::handle::JobHandle;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Receipt returned from action execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionReceipt {
    /// Unique identifier for the action execution
    pub action_id: Uuid,

    /// Optional job handle if the action created a background job
    #[serde(skip)]
    pub job_handle: Option<JobHandle>,

    /// Optional result payload (for immediate actions)
    pub result_payload: Option<serde_json::Value>,

    /// Whether the action completed immediately or is running in background
    pub is_immediate: bool,
}

impl ActionReceipt {
    /// Create a new receipt for an immediate action
    pub fn immediate(action_id: Uuid, result_payload: Option<serde_json::Value>) -> Self {
        Self {
            action_id,
            job_handle: None,
            result_payload,
            is_immediate: true,
        }
    }

    /// Create a new receipt for a job-based action
    pub fn job_based(action_id: Uuid, job_handle: JobHandle) -> Self {
        Self {
            action_id,
            job_handle: Some(job_handle),
            result_payload: None,
            is_immediate: false,
        }
    }

    /// Create a new receipt for a hybrid action (immediate with optional job)
    pub fn hybrid(
        action_id: Uuid,
        result_payload: Option<serde_json::Value>,
        job_handle: Option<JobHandle>,
    ) -> Self {
        let is_immediate = job_handle.is_none();
        Self {
            action_id,
            job_handle,
            result_payload,
            is_immediate,
        }
    }
}