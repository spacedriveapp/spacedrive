//! Job pause operation

use crate::{
    context::CoreContext,
    infra::{
        action::{error::ActionResult, LibraryAction},
        job::types::JobId,
    },
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobPauseInput {
    pub job_id: Uuid,
}

impl JobPauseInput {
    pub fn new(job_id: Uuid) -> Self {
        Self { job_id }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobPauseOutput {
    pub job_id: Uuid,
    pub success: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobPauseAction {
    input: JobPauseInput,
}

impl JobPauseAction {
    pub fn new(input: JobPauseInput) -> Self {
        Self { input }
    }
}

impl LibraryAction for JobPauseAction {
    type Input = JobPauseInput;
    type Output = JobPauseOutput;

    fn from_input(input: JobPauseInput) -> Result<Self, String> {
        Ok(JobPauseAction::new(input))
    }

    fn action_kind(&self) -> &'static str {
        "jobs.pause"
    }

    async fn execute(
        self,
        library: Arc<crate::library::Library>,
        _context: Arc<CoreContext>,
    ) -> ActionResult<Self::Output> {
        let job_id = JobId::from(self.input.job_id);

        match library.jobs().pause_job(job_id).await {
            Ok(()) => Ok(JobPauseOutput {
                job_id: self.input.job_id,
                success: true,
            }),
            Err(e) => {
                // Return success=false instead of error for better UX
                eprintln!("Failed to pause job: {}", e);
                Ok(JobPauseOutput {
                    job_id: self.input.job_id,
                    success: false,
                })
            }
        }
    }
}

crate::register_library_action!(JobPauseAction, "jobs.pause");
