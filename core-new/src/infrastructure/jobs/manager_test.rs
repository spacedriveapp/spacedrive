#[cfg(test)]
mod tests {
    use super::super::{
        context, error::JobResult, manager::JobManager, output, progress, registry, traits::*,
        types::{self, JobStatus},
    };
    use crate::{
        context::CoreContext,
        device::DeviceManager,
        infrastructure::events::EventBus,
        keys::library_key_manager::LibraryKeyManager,
        library::LibraryManager,
        volume::VolumeManager,
    };
    use std::sync::Arc;
    use tokio::time::{sleep, Duration};
    
    // Simple test job that counts slowly
    #[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
    struct SlowCountingJob {
        target: u32,
        current: u32,
    }
    
    impl Job for SlowCountingJob {
        const NAME: &'static str = "SlowCountingJob";
        
        fn schema() -> types::JobSchema {
            types::JobSchema {
                name: Self::NAME.to_string(),
                description: "A test job that counts slowly".to_string(),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "target": { "type": "integer" }
                    }
                }),
            }
        }
    }
    
    #[async_trait::async_trait]
    impl JobHandler for SlowCountingJob {
        async fn run(&mut self, ctx: context::JobContext<'_>) -> JobResult<output::JobOutput> {
            while self.current < self.target {
                // Check for interruption
                ctx.check_interrupt().await?;
                
                // Simulate work
                sleep(Duration::from_millis(100)).await;
                
                self.current += 1;
                
                // Report progress
                let progress = progress::Progress::percentage(self.current as f32 / self.target as f32);
                ctx.progress(progress);
            }
            
            Ok(output::JobOutput::None)
        }
        
        async fn on_pause(&mut self, _ctx: &context::JobContext<'_>) -> JobResult<()> {
            println!("Job paused at count: {}", self.current);
            Ok(())
        }
        
        async fn on_resume(&mut self, _ctx: &context::JobContext<'_>) -> JobResult<()> {
            println!("Job resumed at count: {}", self.current);
            Ok(())
        }
    }
    
    #[tokio::test]
    async fn test_pause_resume_workflow() -> JobResult<()> {
        // Register the test job
        registry::REGISTRY.register::<SlowCountingJob>();
        
        // Create a minimal test setup
        let temp_dir = tempfile::TempDir::new().unwrap();
        let data_dir = temp_dir.path().to_path_buf();
        
        // Create minimal context components
        let events = Arc::new(EventBus::new(1000));
        let device_manager = Arc::new(DeviceManager::new(data_dir.clone()).await.unwrap());
        let library_manager = Arc::new(LibraryManager::new(data_dir.clone()).await.unwrap());
        let volume_manager = Arc::new(VolumeManager::new(events.clone()));
        let library_key_manager = Arc::new(LibraryKeyManager::new().await.unwrap());
        
        let context = Arc::new(CoreContext::new(
            events,
            device_manager,
            library_manager,
            volume_manager,
            library_key_manager,
        ));
        
        // Create job manager
        let manager = JobManager::new(data_dir, context, uuid::Uuid::new_v4()).await?;
        
        // Create and dispatch job
        let job = SlowCountingJob {
            target: 20,
            current: 0,
        };
        
        let handle = manager.dispatch(job).await?;
        let job_id = handle.id();
        
        // Let it run for a bit
        sleep(Duration::from_millis(500)).await;
        
        // Check it's running and made progress
        let status1 = handle.status();
        assert_eq!(status1, JobStatus::Running);
        
        // Pause the job
        manager.pause_job(job_id).await?;
        sleep(Duration::from_millis(200)).await;
        
        // Check it's paused
        let status2 = handle.status();
        assert_eq!(status2, JobStatus::Paused);
        
        // Get progress when paused
        let job_info1 = manager.get_job_info(job_id.0).await?.unwrap();
        let paused_progress = job_info1.progress;
        assert!(paused_progress > 0.0);
        assert!(paused_progress < 100.0);
        
        // Wait and verify no progress while paused
        sleep(Duration::from_millis(500)).await;
        let job_info2 = manager.get_job_info(job_id.0).await?.unwrap();
        assert_eq!(job_info2.progress, paused_progress);
        
        // Resume the job
        manager.resume_job(job_id).await?;
        
        // Wait for completion
        let output = handle.wait().await?;
        
        // Verify completed
        let final_status = handle.status();
        assert_eq!(final_status, JobStatus::Completed);
        
        Ok(())
    }
    
    #[tokio::test]
    async fn test_pause_paused_job_error() -> JobResult<()> {
        registry::REGISTRY.register::<SlowCountingJob>();
        
        let temp_dir = tempfile::TempDir::new().unwrap();
        let data_dir = temp_dir.path().to_path_buf();
        
        // Create minimal context
        let events = Arc::new(EventBus::new(1000));
        let device_manager = Arc::new(DeviceManager::new(data_dir.clone()).await.unwrap());
        let library_manager = Arc::new(LibraryManager::new(data_dir.clone()).await.unwrap());
        let volume_manager = Arc::new(VolumeManager::new(events.clone()));
        let library_key_manager = Arc::new(LibraryKeyManager::new().await.unwrap());
        
        let context = Arc::new(CoreContext::new(
            events,
            device_manager,
            library_manager,
            volume_manager,
            library_key_manager,
        ));
        
        let manager = JobManager::new(data_dir, context, uuid::Uuid::new_v4()).await?;
        
        let job = SlowCountingJob { target: 10, current: 0 };
        let handle = manager.dispatch(job).await?;
        let job_id = handle.id();
        
        // Pause the job
        sleep(Duration::from_millis(200)).await;
        manager.pause_job(job_id).await?;
        
        // Try to pause again
        let result = manager.pause_job(job_id).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Cannot pause job in Paused state"));
        
        Ok(())
    }
    
    #[tokio::test]
    async fn test_resume_running_job_error() -> JobResult<()> {
        registry::REGISTRY.register::<SlowCountingJob>();
        
        let temp_dir = tempfile::TempDir::new().unwrap();
        let data_dir = temp_dir.path().to_path_buf();
        
        // Create minimal context
        let events = Arc::new(EventBus::new(1000));
        let device_manager = Arc::new(DeviceManager::new(data_dir.clone()).await.unwrap());
        let library_manager = Arc::new(LibraryManager::new(data_dir.clone()).await.unwrap());
        let volume_manager = Arc::new(VolumeManager::new(events.clone()));
        let library_key_manager = Arc::new(LibraryKeyManager::new().await.unwrap());
        
        let context = Arc::new(CoreContext::new(
            events,
            device_manager,
            library_manager,
            volume_manager,
            library_key_manager,
        ));
        
        let manager = JobManager::new(data_dir, context, uuid::Uuid::new_v4()).await?;
        
        let job = SlowCountingJob { target: 10, current: 0 };
        let handle = manager.dispatch(job).await?;
        let job_id = handle.id();
        
        // Wait for it to start
        sleep(Duration::from_millis(200)).await;
        
        // Try to resume a running job
        let result = manager.resume_job(job_id).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Cannot resume job in Running state"));
        
        Ok(())
    }
}