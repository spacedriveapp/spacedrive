//! Integration test for job pause/resume functionality

use sd_core_new::{
    infrastructure::{
        database::entities,
        jobs::types::{JobId, JobStatus},
    },
    location::{create_location, LocationCreateArgs, IndexMode},
    Core,
};
use sea_orm::{ActiveModelTrait, EntityTrait, QueryFilter, ColumnTrait};
use std::time::Duration;
use tempfile::TempDir;
use tokio::time::sleep;

#[tokio::test]
async fn test_pause_and_resume_indexing_job() -> Result<(), Box<dyn std::error::Error>> {
    // Setup test environment
    let temp_dir = TempDir::new()?;
    let core = Core::new_with_config(temp_dir.path().to_path_buf()).await?;
    
    // Create library
    let library = core
        .libraries
        .create_library("Test Pause Resume Library", None, core.context.clone())
        .await?;
    
    // Create test location directory with many files
    let test_location_dir = temp_dir.path().join("test_location");
    tokio::fs::create_dir_all(&test_location_dir).await?;
    
    // Create many test files to ensure job runs long enough
    for i in 0..100 {
        let file_path = test_location_dir.join(format!("test_file_{}.txt", i));
        tokio::fs::write(&file_path, format!("Test content {}", i)).await?;
    }
    
    // Register device
    let db = library.db();
    let device = core.device.to_device()?;
    
    let device_record = match entities::device::Entity::find()
        .filter(entities::device::Column::Uuid.eq(device.id))
        .one(db.conn())
        .await?
    {
        Some(existing) => existing,
        None => {
            let device_model: entities::device::ActiveModel = device.into();
            device_model.insert(db.conn()).await?
        }
    };
    
    // Create location to trigger indexing
    let location_args = LocationCreateArgs {
        path: test_location_dir.clone(),
        name: Some("Test Location".to_string()),
        index_mode: IndexMode::Deep,
    };
    
    let _location_db_id = create_location(
        library.clone(),
        &core.events,
        location_args,
        device_record.id,
    )
    .await?;
    
    // Get the indexing job that was created
    let job_manager = library.jobs();
    
    // Wait a bit for job to be created and start
    sleep(Duration::from_millis(200)).await;
    
    // Get running jobs
    let running_jobs = job_manager.list_jobs(Some(JobStatus::Running)).await?;
    assert!(!running_jobs.is_empty(), "Should have a running indexing job");
    
    let job_info = &running_jobs[0];
    let job_id = JobId(job_info.id);
    
    // Wait a bit for job to start processing
    sleep(Duration::from_millis(500)).await;
    
    // Pause the job
    job_manager.pause_job(job_id).await?;
    
    // Wait for pause to take effect
    sleep(Duration::from_millis(200)).await;
    
    // Check job is paused
    let job_info = job_manager.get_job_info(job_id.0).await?.unwrap();
    assert_eq!(job_info.status, JobStatus::Paused, "Job should be paused");
    
    // Record progress when paused
    let paused_progress = job_info.progress;
    assert!(paused_progress > 0.0, "Should have made some progress");
    assert!(paused_progress < 100.0, "Should not be complete");
    
    // Wait a bit to ensure no progress is made while paused
    sleep(Duration::from_millis(500)).await;
    
    // Check progress hasn't changed
    let job_info = job_manager.get_job_info(job_id.0).await?.unwrap();
    assert_eq!(job_info.progress, paused_progress, "Progress should not change while paused");
    
    // Resume the job
    job_manager.resume_job(job_id).await?;
    
    // Wait for job to complete
    let mut retries = 0;
    loop {
        let job_info = job_manager.get_job_info(job_id.0).await?.unwrap();
        match job_info.status {
            JobStatus::Completed => {
                assert!(job_info.progress >= 99.0, "Job should be complete");
                break;
            }
            JobStatus::Failed => {
                panic!("Job failed: {:?}", job_info.error_message);
            }
            _ => {
                if retries > 100 {
                    panic!("Job did not complete in time");
                }
                retries += 1;
                sleep(Duration::from_millis(100)).await;
            }
        }
    }
    
    // Verify files were indexed
    use sea_orm::PaginatorTrait;
    let indexed_count = entities::entry::Entity::find()
        .count(db.conn())
        .await?;
    
    assert!(indexed_count > 0, "Files should be indexed");
    
    Ok(())
}

#[tokio::test]
async fn test_pause_paused_job_error() -> Result<(), Box<dyn std::error::Error>> {
    // Setup test environment
    let temp_dir = TempDir::new()?;
    let core = Core::new_with_config(temp_dir.path().to_path_buf()).await?;
    
    // Create library
    let library = core
        .libraries
        .create_library("Test Pause Error Library", None, core.context.clone())
        .await?;
    
    // Create test location
    let test_location_dir = temp_dir.path().join("test_location");
    tokio::fs::create_dir_all(&test_location_dir).await?;
    tokio::fs::write(test_location_dir.join("test.txt"), "content").await?;
    
    // Register device
    let db = library.db();
    let device = core.device.to_device()?;
    let device_model: entities::device::ActiveModel = device.into();
    let device_record = device_model.insert(db.conn()).await?;
    
    // Create location
    let location_args = LocationCreateArgs {
        path: test_location_dir.clone(),
        name: Some("Test Location".to_string()),
        index_mode: IndexMode::Deep,
    };
    
    create_location(
        library.clone(),
        &core.events,
        location_args,
        device_record.id,
    )
    .await?;
    
    // Get the job
    let job_manager = library.jobs();
    sleep(Duration::from_millis(200)).await;
    let running_jobs = job_manager.list_jobs(Some(JobStatus::Running)).await?;
    let job_id = JobId(running_jobs[0].id);
    
    // Pause the job
    job_manager.pause_job(job_id).await?;
    sleep(Duration::from_millis(100)).await;
    
    // Try to pause again - should fail
    let result = job_manager.pause_job(job_id).await;
    assert!(result.is_err(), "Should not be able to pause an already paused job");
    assert!(result.unwrap_err().to_string().contains("Cannot pause job in Paused state"));
    
    Ok(())
}

#[tokio::test]
async fn test_resume_running_job_error() -> Result<(), Box<dyn std::error::Error>> {
    // Setup test environment
    let temp_dir = TempDir::new()?;
    let core = Core::new_with_config(temp_dir.path().to_path_buf()).await?;
    
    // Create library
    let library = core
        .libraries
        .create_library("Test Resume Error Library", None, core.context.clone())
        .await?;
    
    // Create test location with multiple files
    let test_location_dir = temp_dir.path().join("test_location");
    tokio::fs::create_dir_all(&test_location_dir).await?;
    
    for i in 0..10 {
        let file_path = test_location_dir.join(format!("test_file_{}.txt", i));
        tokio::fs::write(&file_path, format!("Test content {}", i)).await?;
    }
    
    // Register device
    let db = library.db();
    let device = core.device.to_device()?;
    let device_model: entities::device::ActiveModel = device.into();
    let device_record = device_model.insert(db.conn()).await?;
    
    // Create location
    let location_args = LocationCreateArgs {
        path: test_location_dir.clone(),
        name: Some("Test Location".to_string()),
        index_mode: IndexMode::Deep,
    };
    
    create_location(
        library.clone(),
        &core.events,
        location_args,
        device_record.id,
    )
    .await?;
    
    // Get the running job
    let job_manager = library.jobs();
    sleep(Duration::from_millis(200)).await;
    let running_jobs = job_manager.list_jobs(Some(JobStatus::Running)).await?;
    let job_id = JobId(running_jobs[0].id);
    
    // Try to resume a running job - should fail
    let result = job_manager.resume_job(job_id).await;
    assert!(result.is_err(), "Should not be able to resume a running job");
    assert!(result.unwrap_err().to_string().contains("Cannot resume job in Running state"));
    
    Ok(())
}