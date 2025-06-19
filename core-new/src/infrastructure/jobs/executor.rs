//! Job executor that wraps jobs for task system integration

use super::{
    context::{CheckpointHandler, JobContext},
    error::{JobError, JobResult},
    handle::JobHandle,
    output::JobOutput,
    progress::Progress,
    traits::{Job, JobHandler},
    types::{ErasedJob, JobId, JobMetrics},
};
use crate::library::Library;
use async_trait::async_trait;
use std::sync::Arc;
use sd_task_system::{ExecStatus, Interrupter, Task, TaskId};
use tokio::sync::{broadcast, mpsc, watch, Mutex};
use tracing::{debug, error, info};

/// Executor that wraps a job for task system execution
pub struct JobExecutor<J: JobHandler> {
    job: J,
    state: JobExecutorState,
}

pub struct JobExecutorState {
    pub job_id: JobId,
    pub library: Arc<Library>,
    pub status_tx: watch::Sender<super::types::JobStatus>,
    pub progress_tx: mpsc::UnboundedSender<Progress>,
    pub broadcast_tx: broadcast::Sender<Progress>,
    pub checkpoint_handler: Arc<dyn CheckpointHandler>,
    pub metrics: JobMetrics,
    pub output: Option<JobOutput>,
}

impl<J: JobHandler> JobExecutor<J> {
    pub fn new(
        job: J,
        job_id: JobId,
        library: Arc<Library>,
        status_tx: watch::Sender<super::types::JobStatus>,
        progress_tx: mpsc::UnboundedSender<Progress>,
        broadcast_tx: broadcast::Sender<Progress>,
        checkpoint_handler: Arc<dyn CheckpointHandler>,
    ) -> Self {
        Self {
            job,
            state: JobExecutorState {
                job_id,
                library,
                status_tx,
                progress_tx,
                broadcast_tx,
                checkpoint_handler,
                metrics: Default::default(),
                output: None,
            },
        }
    }
}

#[async_trait]
impl<J: JobHandler> Task<JobError> for JobExecutor<J> {
    fn id(&self) -> TaskId {
        TaskId::from(self.state.job_id.0)
    }
    
    fn with_priority(&self) -> bool {
        // TODO: Get from job priority
        false
    }
    
    async fn run(&mut self, interrupter: &Interrupter) -> Result<ExecStatus, JobError> {
        info!("Starting job {}: {}", self.state.job_id, J::NAME);
        
        // Update status to running
        let _ = self.state.status_tx.send(super::types::JobStatus::Running);
        
        // Create job context
        let ctx = JobContext {
            id: self.state.job_id,
            library: self.state.library.clone(),
            interrupter: interrupter,
            progress_tx: self.state.progress_tx.clone(),
            metrics: Arc::new(Mutex::new(self.state.metrics.clone())),
            checkpoint_handler: self.state.checkpoint_handler.clone(),
            child_handles: Arc::new(Mutex::new(Vec::new())),
        };
        
        // Forward progress to broadcast channel
        // For now, skip this as it requires proper channel setup
        // TODO: Implement progress forwarding properly
        
        // Check if we're resuming
        // TODO: Implement proper resume detection
        debug!("Starting job {}", self.state.job_id);
        
        // Store metrics reference for later update
        let metrics_ref = ctx.metrics.clone();
        
        // Run the job  
        let result = match self.job.run(ctx).await {
            Ok(output) => {
                self.state.output = Some(output.into());
                
                // Update metrics
                self.state.metrics = metrics_ref.lock().await.clone();
                
                // Update status
                let _ = self.state.status_tx.send(super::types::JobStatus::Completed);
                
                info!("Job {} completed successfully", self.state.job_id);
                Ok(ExecStatus::Done(sd_task_system::TaskOutput::Empty))
            }
            Err(e) => {
                if e.is_interrupted() {
                    debug!("Job {} interrupted", self.state.job_id);
                    
                    // Update status
                    let _ = self.state.status_tx.send(super::types::JobStatus::Cancelled);
                    
                    Ok(ExecStatus::Canceled)
                } else {
                    error!("Job {} failed: {}", self.state.job_id, e);
                    
                    // Update status
                    let _ = self.state.status_tx.send(super::types::JobStatus::Failed);
                    
                    Err(e)
                }
            }
        };
        
        // Clean up checkpoint if job completed
        if matches!(result, Ok(ExecStatus::Done(_))) {
            let _ = self.state.checkpoint_handler.delete_checkpoint(self.state.job_id).await;
        }
        
        result
    }
}

// Implement ErasedJob for the executor
impl<J: JobHandler> ErasedJob for JobExecutor<J> {
    fn as_task(&mut self) -> Box<dyn Task<JobError>> {
        // This is a bit of a hack to get around lifetime issues
        // In practice, we'd use unsafe or refactor the trait
        todo!("Implement proper type erasure")
    }
    
    fn serialize_state(&self) -> Result<Vec<u8>, JobError> {
        rmp_serde::to_vec(&self.job)
            .map_err(|e| JobError::serialization(format!("{}", e)))
    }
}