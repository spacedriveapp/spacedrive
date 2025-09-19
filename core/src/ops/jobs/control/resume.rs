//! Job resume operation

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
pub struct JobResumeInput {
    pub job_id: Uuid,
}

impl JobResumeInput {
    pub fn new(job_id: Uuid) -> Self {
        Self { job_id }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobResumeOutput {
    pub job_id: Uuid,
    pub success: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobResumeAction {
    input: JobResumeInput,
}

impl JobResumeAction {
    pub fn new(input: JobResumeInput) -> Self {
        Self { input }
    }
}

impl LibraryAction for JobResumeAction {
    type Input = JobResumeInput;
    type Output = JobResumeOutput;

    fn from_input(input: JobResumeInput) -> Result<Self, String> {
        Ok(JobResumeAction::new(input))
    }

    fn action_kind(&self) -> &'static str {
        "jobs.resume"
    }

    async fn execute(
        self,
        library: Arc<crate::library::Library>,
        _context: Arc<CoreContext>,
    ) -> ActionResult<Self::Output> {
        let job_id = JobId::from(self.input.job_id);

        match library.jobs().resume_job(job_id).await {
            Ok(()) => Ok(JobResumeOutput {
                job_id: self.input.job_id,
                success: true,
            }),
            Err(e) => {
                // Return success=false instead of error for better UX
                eprintln!("Failed to resume job: {}", e);
                Ok(JobResumeOutput {
                    job_id: self.input.job_id,
                    success: false,
                })
            }
        }
    }
}

crate::register_library_action!(JobResumeAction, "jobs.resume");
