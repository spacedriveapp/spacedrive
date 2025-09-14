use clap::Args;
use uuid::Uuid;

use sd_core::{
    infra::job::types::JobStatus,
    ops::jobs::{
        info::query::JobInfoQuery,
        list::query::JobListQuery,
    },
};

#[derive(Args, Debug)]
pub struct JobListArgs {
    #[arg(long)]
    pub status: Option<String>,
}

impl JobListArgs {
    pub fn to_query(&self, library_id: Uuid) -> JobListQuery {
        JobListQuery {
            status: self.status.as_deref().and_then(|s| s.parse::<JobStatus>().ok()),
        }
    }
}

#[derive(Args, Debug)]
pub struct JobInfoArgs {
    pub job_id: Uuid,
}

impl JobInfoArgs {
    pub fn to_query(&self) -> JobInfoQuery {
        JobInfoQuery {
            job_id: self.job_id,
        }
    }
}

