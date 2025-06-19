//! Job manager for scheduling and executing jobs

use super::{
    context::CheckpointHandler,
    database::{self, JobDb},
    error::{JobError, JobResult},
    executor::JobExecutor,
    handle::JobHandle,
    output::JobOutput,
    progress::Progress,
    registry::REGISTRY,
    traits::{Job, JobHandler},
    types::{JobId, JobPriority, JobStatus, JobInfo},
};
use crate::{
    library::Library,
    infrastructure::events::{Event, EventBus},
};
use async_trait::async_trait;
use chrono::Utc;
use sea_orm::{ActiveModelTrait, ActiveValue::Set, DatabaseConnection, EntityTrait};
use std::{collections::HashMap, path::PathBuf, sync::Arc};
use sd_task_system::{TaskSystem, TaskHandle};
use tokio::sync::{broadcast, mpsc, watch, Mutex, RwLock};
use tracing::{debug, error, info, warn};

/// Manages job execution for a library
pub struct JobManager {
    library: RwLock<Option<Arc<Library>>>,
    db: Arc<JobDb>,
    dispatcher: Arc<TaskSystem<JobError>>,
    running_jobs: Arc<RwLock<HashMap<JobId, RunningJob>>>,
    shutdown_tx: watch::Sender<bool>,
    event_bus: Option<Arc<EventBus>>,
}

struct RunningJob {
    handle: JobHandle,
    task_handle: TaskHandle<JobError>,
    status_tx: watch::Sender<JobStatus>,
    latest_progress: Arc<Mutex<Option<Progress>>>,
}

impl JobManager {
    /// Create a new job manager
    pub async fn new(data_dir: PathBuf) -> JobResult<Self> {
        // Initialize job database
        let job_db_path = data_dir.join("jobs.db");
        let db = database::init_database(&job_db_path).await?;
        
        // Create task system
        let dispatcher = TaskSystem::new();
        
        let (shutdown_tx, _) = watch::channel(false);
        
        let manager = Self {
            library: RwLock::new(None),
            db: Arc::new(JobDb::new(db)),
            dispatcher: Arc::new(dispatcher),
            running_jobs: Arc::new(RwLock::new(HashMap::new())),
            shutdown_tx,
            event_bus: None,
        };
        
        Ok(manager)
    }
    
    /// Set the library reference after creation
    pub async fn set_library(&self, library: Arc<Library>) {
        *self.library.write().await = Some(library);
        // Resume any interrupted jobs after library is set
        if let Err(e) = self.resume_interrupted_jobs().await {
            error!("Failed to resume interrupted jobs: {}", e);
        }
    }
    
    /// Dispatch a job for execution
    pub async fn dispatch<J>(&self, job: J) -> JobResult<JobHandle>
    where
        J: Job + JobHandler,
    {
        self.dispatch_with_priority(job, JobPriority::NORMAL).await
    }
    
    /// Dispatch a job with specific priority
    pub async fn dispatch_with_priority<J>(
        &self,
        job: J,
        priority: JobPriority,
    ) -> JobResult<JobHandle>
    where
        J: Job + JobHandler,
    {
        let job_id = JobId::new();
        info!("Dispatching job {}: {}", job_id, J::NAME);
        
        // Serialize job state
        let state = rmp_serde::to_vec(&job)
            .map_err(|e| JobError::serialization(format!("{}", e)))?;
        
        // Create database record
        let job_model = database::jobs::ActiveModel {
            id: Set(job_id.to_string()),
            name: Set(J::NAME.to_string()),
            state: Set(state),
            status: Set(JobStatus::Queued.to_string()),
            priority: Set(priority.0),
            progress_type: Set(None),
            progress_data: Set(None),
            parent_job_id: Set(None),
            created_at: Set(Utc::now()),
            started_at: Set(None),
            completed_at: Set(None),
            paused_at: Set(None),
            error_message: Set(None),
            warnings: Set(None),
            non_critical_errors: Set(None),
            metrics: Set(None),
        };
        
        job_model.insert(self.db.conn()).await?;
        
        // Create channels
        let (status_tx, status_rx) = watch::channel(JobStatus::Queued);
        let (progress_tx, progress_rx) = mpsc::unbounded_channel();
        let (broadcast_tx, broadcast_rx) = broadcast::channel(100);
        
        // Create storage for latest progress
        let latest_progress = Arc::new(Mutex::new(None));
        
        // Create progress forwarding task to bridge mpsc -> broadcast
        let broadcast_tx_clone = broadcast_tx.clone();
        let latest_progress_clone = latest_progress.clone();
        tokio::spawn(async move {
            let mut progress_rx: mpsc::UnboundedReceiver<Progress> = progress_rx;
            while let Some(progress) = progress_rx.recv().await {
                // Store latest progress
                *latest_progress_clone.lock().await = Some(progress.clone());
                
                // Forward progress from mpsc to broadcast
                // Ignore errors if no one is listening
                let _ = broadcast_tx_clone.send(progress);
            }
        });
        
        // Get library reference
        let library = self.library.read().await
            .as_ref()
            .ok_or_else(|| JobError::invalid_state("Library not initialized"))?
            .clone();
        
        // Create executor
        let executor = JobExecutor::new(
            job,
            job_id,
            library,
            status_tx.clone(),
            progress_tx,
            broadcast_tx,
            Arc::new(DbCheckpointHandler {
                db: self.db.clone(),
            }),
        );
        
        // Create handle
        let handle = JobHandle {
            id: job_id,
            task_handle: Arc::new(Mutex::new(None)),
            status_rx,
            progress_rx: broadcast_rx,
            output: Arc::new(Mutex::new(None)),
        };
        
        // Dispatch to task system
        let task_handle = self.dispatcher.dispatch(executor).await;
        
        match task_handle {
            Ok(handle_result) => {
                // We don't store the task handle in JobHandle anymore
                // since it's already stored in RunningJob
                
                // Track running job
                self.running_jobs.write().await.insert(
                    job_id,
                    RunningJob {
                        handle: handle.clone(),
                        task_handle: handle_result,
                        status_tx,
                        latest_progress: latest_progress.clone(),
                    },
                );
                
                Ok(handle)
            }
            Err(e) => {
                Err(JobError::task_system(format!("{:?}", e)))
            }
        }
    }
    
    /// Get a handle to a running job
    pub async fn get_job(&self, id: JobId) -> Option<JobHandle> {
        self.running_jobs.read().await
            .get(&id)
            .map(|j| j.handle.clone())
    }
    
    /// List currently running jobs from memory (for live monitoring)
    pub async fn list_running_jobs(&self) -> Vec<JobInfo> {
        let running_jobs = self.running_jobs.read().await;
        let mut job_infos = Vec::new();
        
        info!("list_running_jobs: Found {} jobs in running_jobs map", running_jobs.len());
        
        for (job_id, running_job) in running_jobs.iter() {
            let handle = &running_job.handle;
            let status = handle.status();
            
            info!("Job {}: status = {:?}", job_id, status);
            
            // Only include active jobs (running or paused)
            if status.is_active() {
                // Get latest progress
                let progress_percentage = if let Some(progress) = running_job.latest_progress.lock().await.as_ref() {
                    progress.as_percentage().unwrap_or(0.0)
                } else {
                    0.0
                };
                
                // Create JobInfo from in-memory state
                let job_info = JobInfo {
                    id: job_id.0,
                    name: format!("Job {}", job_id), // Use job ID as name for now
                    status,
                    progress: progress_percentage,
                    started_at: chrono::Utc::now(), // TODO: Get actual start time
                    completed_at: None,
                    error_message: None,
                    parent_job_id: None,
                };
                
                job_infos.push(job_info);
                info!("Added active job {} to result with progress {:.1}%", job_id, progress_percentage * 100.0);
            } else {
                info!("Skipping job {} with status {:?} (not active)", job_id, status);
            }
        }
        
        info!("Returning {} active jobs", job_infos.len());
        job_infos
    }
    
    /// List all jobs with a specific status
    pub async fn list_jobs(&self, status: Option<JobStatus>) -> JobResult<Vec<JobInfo>> {
        use sea_orm::QueryFilter;
        
        let mut query = database::jobs::Entity::find();
        
        if let Some(status) = status {
            use sea_orm::ColumnTrait;
            query = query.filter(database::jobs::Column::Status.eq(status.to_string()));
        }
        
        let jobs = query.all(self.db.conn()).await?;
        
        Ok(jobs.into_iter()
            .filter_map(|j| {
                let id = j.id.parse::<Uuid>().ok()?;
                let status = match j.status.as_str() {
                    "queued" => JobStatus::Queued,
                    "running" => JobStatus::Running,
                    "paused" => JobStatus::Paused,
                    "completed" => JobStatus::Completed,
                    "failed" => JobStatus::Failed,
                    "cancelled" => JobStatus::Cancelled,
                    _ => return None,
                };
                
                // Parse progress from bytes if available
                let progress = if let Some(progress_data) = &j.progress_data {
                    rmp_serde::from_slice::<Progress>(progress_data)
                        .ok()
                        .and_then(|p| p.as_percentage())
                        .unwrap_or(0.0)
                } else {
                    0.0
                };
                
                Some(JobInfo {
                    id,
                    name: j.name,
                    status,
                    progress,
                    started_at: j.started_at.unwrap_or(j.created_at),
                    completed_at: j.completed_at,
                    error_message: j.error_message,
                    parent_job_id: j.parent_job_id.and_then(|s| s.parse::<Uuid>().ok()),
                })
            })
            .collect())
    }
    
    /// Get detailed information about a specific job
    pub async fn get_job_info(&self, id: Uuid) -> JobResult<Option<JobInfo>> {
        let job = database::jobs::Entity::find_by_id(id.to_string())
            .one(self.db.conn())
            .await?;
            
        Ok(job.and_then(|j| {
            let id = j.id.parse::<Uuid>().ok()?;
            let status = match j.status.as_str() {
                "queued" => JobStatus::Queued,
                "running" => JobStatus::Running,
                "paused" => JobStatus::Paused,
                "completed" => JobStatus::Completed,
                "failed" => JobStatus::Failed,
                "cancelled" => JobStatus::Cancelled,
                _ => return None,
            };
            
            let progress = if let Some(progress_data) = &j.progress_data {
                rmp_serde::from_slice::<Progress>(progress_data)
                    .ok()
                    .and_then(|p| p.as_percentage())
                    .unwrap_or(0.0)
            } else {
                0.0
            };
            
            Some(JobInfo {
                id,
                name: j.name,
                status,
                progress,
                started_at: j.started_at.unwrap_or(j.created_at),
                completed_at: j.completed_at,
                error_message: j.error_message,
                parent_job_id: j.parent_job_id.and_then(|s| s.parse::<Uuid>().ok()),
            })
        }))
    }
    
    /// Resume interrupted jobs from the last run
    async fn resume_interrupted_jobs(&self) -> JobResult<()> {
        info!("Checking for interrupted jobs to resume");
        
        use sea_orm::{QueryFilter, ColumnTrait};
        let interrupted = database::jobs::Entity::find()
            .filter(
                database::jobs::Column::Status.is_in([
                    JobStatus::Running.to_string(),
                    JobStatus::Paused.to_string(),
                ])
            )
            .all(self.db.conn())
            .await?;
        
        for job_record in interrupted {
            if let Ok(job_id) = job_record.id.parse::<Uuid>().map(JobId) {
                info!("Resuming job {}: {}", job_id, job_record.name);
                
                // Create job from saved state
                match REGISTRY.create_job(&job_record.name, serde_json::json!({})) {
                    Ok(mut erased_job) => {
                        // TODO: Properly restore job state and dispatch
                        warn!("Job resumption not fully implemented yet");
                    }
                    Err(e) => {
                        error!("Failed to resume job {}: {}", job_id, e);
                    }
                }
            }
        }
        
        Ok(())
    }
    
    /// Shutdown the job manager
    pub async fn shutdown(&self) -> JobResult<()> {
        info!("Shutting down job manager");
        
        // Signal shutdown
        let _ = self.shutdown_tx.send(true);
        
        // Wait for all jobs to complete or pause
        let job_ids: Vec<JobId> = self.running_jobs.read().await.keys().copied().collect();
        for id in job_ids {
            debug!("Waiting for job {} to stop", id);
            // The task system will handle graceful shutdown
        }
        
        Ok(())
    }
}

/// Checkpoint handler that uses the job database
struct DbCheckpointHandler {
    db: Arc<JobDb>,
}

#[async_trait]
impl CheckpointHandler for DbCheckpointHandler {
    async fn save_checkpoint(&self, job_id: JobId, data: Option<Vec<u8>>) -> JobResult<()> {
        use database::checkpoint;
        
        let checkpoint = checkpoint::ActiveModel {
            job_id: Set(job_id.to_string()),
            checkpoint_data: Set(data.unwrap_or_default()),
            created_at: Set(Utc::now()),
        };
        
        // Insert or update
        match checkpoint.clone().insert(self.db.conn()).await {
            Ok(model) => model,
            Err(_) => checkpoint.update(self.db.conn()).await?,
        };
        
        Ok(())
    }
    
    async fn load_checkpoint(&self, job_id: JobId) -> JobResult<Option<Vec<u8>>> {
        use database::checkpoint;
        
        let checkpoint = checkpoint::Entity::find_by_id(job_id.to_string())
            .one(self.db.conn())
            .await?;
        
        Ok(checkpoint.map(|c| c.checkpoint_data))
    }
    
    async fn delete_checkpoint(&self, job_id: JobId) -> JobResult<()> {
        use database::checkpoint;
        
        checkpoint::Entity::delete_by_id(job_id.to_string())
            .exec(self.db.conn())
            .await?;
        
        Ok(())
    }
}

use uuid::Uuid;