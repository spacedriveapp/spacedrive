use clap::Args;
use uuid::Uuid;

use sd_core::{
    infra::job::types::JobStatus,
    ops::jobs::{
        info::query::JobInfoQueryInput,
        list::query::JobListInput,
    },
};

#[derive(Args, Debug)]
pub struct JobListArgs {
    #[arg(long)]
    pub status: Option<String>,
}

impl JobListArgs {
    pub fn to_input(&self, _library_id: Uuid) -> JobListInput {
        JobListInput {
            status: self.status.as_deref().and_then(|s| s.parse::<JobStatus>().ok()),
        }
    }
}

#[derive(Args, Debug)]
pub struct JobInfoArgs {
    pub job_id: Uuid,
}

impl JobInfoArgs {
    pub fn to_input(&self) -> JobInfoQueryInput {
        JobInfoQueryInput {
            job_id: self.job_id,
        }
    }
}

#[derive(Args, Debug, Clone)]
pub struct JobMonitorArgs {
    /// Monitor a specific job by ID
    #[arg(long)]
    pub job_id: Option<Uuid>,

    /// Filter by job status
    #[arg(long)]
    pub status: Option<String>,

    /// Refresh interval in seconds
    #[arg(long, default_value = "1")]
    pub refresh: u64,

    /// Use simple progress bars instead of TUI
    #[arg(long)]
    pub simple: bool,
}

#[derive(Args, Debug)]
pub struct JobControlArgs {
    /// Job ID to control
    pub job_id: Uuid,
}

