//! Core types for the job system

use serde::{Deserialize, Serialize};
use std::fmt;
use uuid::Uuid;

/// Unique identifier for a job
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct JobId(pub Uuid);

impl JobId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl fmt::Display for JobId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<Uuid> for JobId {
    fn from(uuid: Uuid) -> Self {
        Self(uuid)
    }
}

impl From<JobId> for Uuid {
    fn from(id: JobId) -> Self {
        id.0
    }
}

/// Current status of a job
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JobStatus {
    /// Job is waiting to be executed
    Queued,
    /// Job is currently running
    Running,
    /// Job has been paused
    Paused,
    /// Job completed successfully
    Completed,
    /// Job failed with an error
    Failed,
    /// Job was cancelled
    Cancelled,
}

impl JobStatus {
    pub fn is_terminal(&self) -> bool {
        matches!(self, Self::Completed | Self::Failed | Self::Cancelled)
    }

    pub fn is_active(&self) -> bool {
        matches!(self, Self::Running | Self::Paused)
    }
}

impl fmt::Display for JobStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Queued => write!(f, "Queued"),
            Self::Running => write!(f, "Running"),
            Self::Paused => write!(f, "Paused"),
            Self::Completed => write!(f, "Completed"),
            Self::Failed => write!(f, "Failed"),
            Self::Cancelled => write!(f, "Cancelled"),
        }
    }
}

/// Priority level for job execution
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct JobPriority(pub i32);

impl JobPriority {
    pub const LOW: Self = Self(-1);
    pub const NORMAL: Self = Self(0);
    pub const HIGH: Self = Self(1);
    pub const CRITICAL: Self = Self(10);
}

impl Default for JobPriority {
    fn default() -> Self {
        Self::NORMAL
    }
}

/// Metrics collected during job execution
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct JobMetrics {
    pub bytes_processed: u64,
    pub items_processed: u64,
    pub warnings_count: u32,
    pub non_critical_errors_count: u32,
    pub duration_ms: Option<u64>,
}

/// Schema definition for a job type
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobSchema {
    pub name: &'static str,
    pub resumable: bool,
    pub version: u32,
    pub description: Option<&'static str>,
}

/// Registration information for a job type
#[derive(Clone)]
pub struct JobRegistration {
    pub name: &'static str,
    pub schema_fn: fn() -> JobSchema,
    pub create_fn: fn(serde_json::Value) -> Result<Box<dyn ErasedJob>, serde_json::Error>,
    pub deserialize_fn: fn(&[u8]) -> Result<Box<dyn ErasedJob>, rmp_serde::decode::Error>,
}

/// Type-erased job for dynamic dispatch
pub trait ErasedJob: Send + Sync + std::fmt::Debug + 'static {
    fn create_executor(
        self: Box<Self>,
        job_id: JobId,
        library: std::sync::Arc<crate::library::Library>,
        status_tx: tokio::sync::watch::Sender<JobStatus>,
        progress_tx: tokio::sync::mpsc::UnboundedSender<crate::infrastructure::jobs::progress::Progress>,
        broadcast_tx: tokio::sync::broadcast::Sender<crate::infrastructure::jobs::progress::Progress>,
        checkpoint_handler: std::sync::Arc<dyn crate::infrastructure::jobs::context::CheckpointHandler>,
        networking: Option<std::sync::Arc<tokio::sync::RwLock<crate::networking::NetworkingCore>>>,
    ) -> Box<dyn sd_task_system::Task<crate::infrastructure::jobs::error::JobError>>;

    fn serialize_state(&self) -> Result<Vec<u8>, crate::infrastructure::jobs::error::JobError>;
}

/// Information about a job (for display/querying)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobInfo {
    pub id: Uuid,
    pub name: String,
    pub status: JobStatus,
    pub progress: f32,
    pub started_at: chrono::DateTime<chrono::Utc>,
    pub completed_at: Option<chrono::DateTime<chrono::Utc>>,
    pub error_message: Option<String>,
    pub parent_job_id: Option<Uuid>,
}