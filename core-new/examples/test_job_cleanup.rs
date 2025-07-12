use sd_core_new::{
    context::CoreContext,
    infrastructure::jobs::{Job, JobHandler, JobInfo, JobOutput, JobResult, Progress},
    library::{Library, LibraryConfig, LibraryManager},
};
use std::{path::PathBuf, sync::Arc, time::Duration};
use tokio::time::sleep;
use tracing::{info, Level};
use tracing_subscriber::FmtSubscriber;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct TestJob {
    duration_secs: u64,
}

#[sd_core_new::infrastructure::jobs::job]
impl Job for TestJob {
    const NAME: &'static str = "test_job";

    async fn run(
        mut self,
        ctx: sd_core_new::infrastructure::jobs::context::JobContext,
    ) -> JobResult<JobOutput> {
        info!("Test job started, will run for {} seconds", self.duration_secs);
        
        // Simulate work with progress updates
        for i in 0..self.duration_secs {
            ctx.progress(Progress::percentage(i as f32 / self.duration_secs as f32)).await;
            sleep(Duration::from_secs(1)).await;
        }
        
        ctx.progress(Progress::percentage(1.0)).await;
        info!("Test job completed");
        
        Ok(JobOutput::Empty)
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize tracing
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .finish();
    tracing::subscriber::set_global_default(subscriber)?;

    // Create temp directory for testing
    let temp_dir = tempfile::tempdir()?;
    let data_dir = temp_dir.path().to_path_buf();
    
    // Initialize core context
    let context = Arc::new(CoreContext::new(data_dir.clone()).await?);
    
    // Create a test library
    let library_manager = context.library_manager.clone();
    let library_config = LibraryConfig {
        name: "Test Library".to_string(),
        description: Some("Test library for job cleanup".to_string()),
    };
    
    let library = library_manager.create_library(library_config).await?;
    info!("Created library: {}", library.id);
    
    // Get the job manager
    let job_manager = library.job_manager.clone();
    
    // Dispatch a test job
    let test_job = TestJob { duration_secs: 3 };
    let handle = job_manager.dispatch(test_job).await?;
    let job_id = handle.id;
    
    info!("Dispatched job: {}", job_id);
    
    // Monitor the job status
    let mut last_status = None;
    for i in 0..10 {
        sleep(Duration::from_secs(1)).await;
        
        // Check if job is in running_jobs
        let running_jobs = job_manager.list_running_jobs().await;
        let in_memory = running_jobs.iter().any(|j| j.id == job_id.0);
        
        // Check database status
        let job_info = job_manager.get_job_info(job_id.0).await?;
        
        if let Some(info) = &job_info {
            let status_changed = last_status.as_ref() != Some(&info.status);
            if status_changed {
                info!(
                    "Job {} status: {:?}, progress: {:.1}%, in_memory: {}",
                    job_id,
                    info.status,
                    info.progress * 100.0,
                    in_memory
                );
                last_status = Some(info.status.clone());
            }
            
            // Check if job completed
            if matches!(info.status, sd_core_new::infrastructure::jobs::types::JobStatus::Completed) {
                info!("Job completed! Checking if removed from memory...");
                sleep(Duration::from_millis(100)).await; // Give cleanup task time to run
                
                let running_jobs_after = job_manager.list_running_jobs().await;
                let still_in_memory = running_jobs_after.iter().any(|j| j.id == job_id.0);
                
                if still_in_memory {
                    eprintln!("ERROR: Job {} is still in running_jobs map after completion!", job_id);
                } else {
                    info!("SUCCESS: Job {} was properly removed from running_jobs map", job_id);
                }
                break;
            }
        }
    }
    
    // Final check
    let final_running = job_manager.list_running_jobs().await;
    info!("Final running jobs count: {}", final_running.len());
    
    Ok(())
}