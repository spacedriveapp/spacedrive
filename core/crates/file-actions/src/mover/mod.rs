use std::path::PathBuf;

use serde::{Deserialize, Serialize};

pub mod tasks;

use heavy_lifting::{
    job::{Job, JobContext, JobError},
    report::ReportInputMetadata,
    task::Task,
};

use self::tasks::MoveTask;

#[derive(Debug, Serialize, Deserialize)]
pub struct MoveJob {
    sources: Vec<PathBuf>,
    target_dir: PathBuf,
}

impl MoveJob {
    pub fn new(sources: Vec<PathBuf>, target_dir: impl Into<PathBuf>) -> Self {
        Self {
            sources,
            target_dir: target_dir.into(),
        }
    }

    async fn create_move_tasks(
        &self,
        ctx: &impl JobContext,
    ) -> Result<Vec<Box<dyn Task<Error = JobError>>>, JobError> {
        let mut tasks: Vec<Box<dyn Task<Error = JobError>>> = Vec::new();

        ctx.progress_msg(format!("Moving {} files to {}", self.sources.len(), self.target_dir.display())).await;

        for source in &self.sources {
            let target = self.target_dir.join(source.file_name().unwrap());
            tasks.push(Box::new(MoveTask::new(source.clone(), target)));
        }

        Ok(tasks)
    }
}

#[async_trait::async_trait]
impl Task for MoveJob {
    type Error = JobError;

    fn name(&self) -> &'static str {
        "move"
    }

    fn metadata(&self) -> ReportInputMetadata {
        ReportInputMetadata::Mover {
            sources: self.sources.clone(),
            target_dir: self.target_dir.clone(),
        }
    }

    async fn run(
        &self,
        ctx: &impl JobContext,
    ) -> Result<Vec<Box<dyn Task<Error = JobError>>>, JobError> {
        let tasks = self.create_move_tasks(ctx).await?;
        ctx.progress_count(tasks.len() as u64).await;
        Ok(tasks)
    }
}
