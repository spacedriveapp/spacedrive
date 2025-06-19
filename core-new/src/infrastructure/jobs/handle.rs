//! Job handle for controlling running jobs

use super::{
    error::{JobError, JobResult},
    output::JobOutput,
    progress::Progress,
    types::{JobId, JobStatus},
};
use std::sync::Arc;
use sd_task_system::TaskHandle;
use tokio::sync::{broadcast, watch, Mutex};

/// Handle to a running job
pub struct JobHandle {
    pub(crate) id: JobId,
    pub(crate) task_handle: Arc<Mutex<Option<TaskHandle<JobError>>>>,
    pub(crate) status_rx: watch::Receiver<JobStatus>,
    pub(crate) progress_rx: broadcast::Receiver<Progress>,
    pub(crate) output: Arc<Mutex<Option<JobOutput>>>,
}

impl Clone for JobHandle {
    fn clone(&self) -> Self {
        Self {
            id: self.id,
            task_handle: self.task_handle.clone(),
            status_rx: self.status_rx.clone(),
            progress_rx: self.progress_rx.resubscribe(),
            output: self.output.clone(),
        }
    }
}

impl JobHandle {
    /// Get the job ID
    pub fn id(&self) -> JobId {
        self.id
    }
    
    /// Get the current status
    pub fn status(&self) -> JobStatus {
        *self.status_rx.borrow()
    }
    
    /// Subscribe to status updates
    pub fn subscribe_status(&self) -> watch::Receiver<JobStatus> {
        self.status_rx.clone()
    }
    
    /// Subscribe to progress updates
    pub fn subscribe_progress(&self) -> broadcast::Receiver<Progress> {
        self.progress_rx.resubscribe()
    }
    
    /// Wait for the job to complete
    pub async fn wait(&self) -> JobResult<JobOutput> {
        // Wait for terminal status
        let mut status_rx = self.status_rx.clone();
        while !status_rx.borrow().is_terminal() {
            status_rx.changed().await
                .map_err(|_| JobError::Other("Status channel closed".into()))?;
        }
        
        // Check final status
        let final_status = *status_rx.borrow();
        match final_status {
            JobStatus::Completed => {
                // Get output
                let output = self.output.lock().await.clone()
                    .unwrap_or_default();
                Ok(output)
            }
            JobStatus::Failed => Err(JobError::ExecutionFailed("Job failed".into())),
            JobStatus::Cancelled => Err(JobError::Interrupted),
            _ => unreachable!("Non-terminal status after wait"),
        }
    }
    
    /// Pause the job
    pub async fn pause(&self) -> JobResult<()> {
        // For now, these operations need to be implemented through JobManager
        // since the TaskHandle is stored there, not in JobHandle
        todo!("Job control operations will be implemented through JobManager")
    }
    
    /// Resume the job
    pub async fn resume(&self) -> JobResult<()> {
        todo!("Job control operations will be implemented through JobManager")
    }
    
    /// Cancel the job
    pub async fn cancel(&self) -> JobResult<()> {
        todo!("Job control operations will be implemented through JobManager")
    }
    
    /// Force abort the job
    pub async fn force_abort(&self) -> JobResult<()> {
        todo!("Job control operations will be implemented through JobManager")
    }
}

/// Update events from a job
#[derive(Debug)]
pub enum JobUpdate {
    /// Status changed
    StatusChanged(JobStatus),
    /// Progress update
    Progress(Progress),
    /// Job completed
    Completed(JobOutput),
    /// Job failed
    Failed(JobError),
}

impl JobHandle {
    /// Subscribe to all job updates
    pub fn subscribe(&self) -> JobUpdateStream {
        JobUpdateStream {
            handle: self.clone(),
            status_rx: self.status_rx.clone(),
            progress_rx: self.progress_rx.resubscribe(),
        }
    }
}

/// Stream of job updates
pub struct JobUpdateStream {
    handle: JobHandle,
    status_rx: watch::Receiver<JobStatus>,
    progress_rx: broadcast::Receiver<Progress>,
}

impl JobUpdateStream {
    /// Get the next update
    pub async fn next(&mut self) -> Option<JobUpdate> {
        tokio::select! {
            // Status changes
            Ok(_) = self.status_rx.changed() => {
                let status = *self.status_rx.borrow();
                
                match status {
                    JobStatus::Completed => {
                        let output = self.handle.output.lock().await.clone()
                            .unwrap_or_default();
                        Some(JobUpdate::Completed(output))
                    }
                    JobStatus::Failed => {
                        Some(JobUpdate::Failed(JobError::ExecutionFailed("Job failed".into())))
                    }
                    _ => Some(JobUpdate::StatusChanged(status)),
                }
            }
            
            // Progress updates
            Ok(progress) = self.progress_rx.recv() => {
                Some(JobUpdate::Progress(progress))
            }
            
            else => None,
        }
    }
}