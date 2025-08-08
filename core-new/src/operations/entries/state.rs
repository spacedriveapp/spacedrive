use serde::Serialize;
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "type", content = "data")]
pub enum EntryState {
    Available,
    Processing { job_id: Uuid },
    Syncing { job_id: Uuid },
    Validating { job_id: Uuid },
    Offline,
    Archived,
    Error { message: String },
}