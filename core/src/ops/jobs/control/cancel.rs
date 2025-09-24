//! Job cancel operation

use crate::{
    context::CoreContext,
    infra::{
        action::{error::ActionResult, LibraryAction},
        job::types::JobId,
    },
};
use serde::{Deserialize, Serialize};
use specta::Type;
use std::sync::Arc;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct JobCancelInput {
    pub job_id: Uuid,
}

impl JobCancelInput {
    pub fn new(job_id: Uuid) -> Self {
        Self { job_id }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct JobCancelOutput {
    pub job_id: Uuid,
    pub success: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobCancelAction {
    input: JobCancelInput,
}

impl JobCancelAction {
    pub fn new(input: JobCancelInput) -> Self {
        Self { input }
    }
}

impl LibraryAction for JobCancelAction {
    type Input = JobCancelInput;
    type Output = JobCancelOutput;

    fn from_input(input: JobCancelInput) -> Result<Self, String> {
        Ok(JobCancelAction::new(input))
    }

    fn action_kind(&self) -> &'static str {
        "jobs.cancel"
    }

    async fn execute(
        self,
        library: Arc<crate::library::Library>,
        _context: Arc<CoreContext>,
    ) -> ActionResult<Self::Output> {
        let job_id = JobId::from(self.input.job_id);

        match library.jobs().cancel_job(job_id).await {
            Ok(()) => Ok(JobCancelOutput {
                job_id: self.input.job_id,
                success: true,
            }),
            Err(e) => {
                // Return success=false instead of error for better UX
                eprintln!("Failed to cancel job: {}", e);
                Ok(JobCancelOutput {
                    job_id: self.input.job_id,
                    success: false,
                })
            }
        }
    }
}

crate::register_library_action!(JobCancelAction, "jobs.cancel");
