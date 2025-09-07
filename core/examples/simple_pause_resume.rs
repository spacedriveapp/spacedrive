//! Simple demonstration of pause/resume functionality

use sd_core_new::Core;
use std::time::Duration;
use tokio::time::sleep;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt::init();
    
    println!("=== Simple Pause/Resume Demo ===\n");
    
    // Create Core instance
    let core = Core::new().await?;
    
    // Get open libraries
    let libraries = core.libraries.get_open_libraries().await;
    if libraries.is_empty() {
        println!("No open libraries found. Please create and open a library first.");
        return Ok(());
    }
    
    // Use the first library
    let library = libraries[0].clone();
    println!("Using library: {}", library.id());
    
    // Get job manager
    let job_manager = library.jobs();
    
    // List running jobs
    println!("\nChecking for running jobs...");
    let running_jobs = job_manager.list_jobs(Some(sd_core_new::infrastructure::jobs::types::JobStatus::Running)).await?;
    
    if running_jobs.is_empty() {
        println!("No running jobs found.");
        println!("\nTo test pause/resume:");
        println!("1. Start an indexing job: spacedrive location add /path/to/folder");
        println!("2. Run this demo again while indexing is in progress");
        return Ok(());
    }
    
    // Get the first running job
    let job_info = &running_jobs[0];
    let job_id = sd_core_new::infrastructure::jobs::types::JobId(job_info.id);
    
    println!("\nFound running job:");
    println!("  ID: {}", job_info.id);
    println!("  Name: {}", job_info.name);
    println!("  Progress: {:.1}%", job_info.progress);
    
    // Pause the job
    println!("\nPausing job...");
    match job_manager.pause_job(job_id).await {
        Ok(_) => println!("✓ Job paused successfully"),
        Err(e) => {
            println!("✗ Failed to pause job: {}", e);
            return Ok(());
        }
    }
    
    // Check status
    sleep(Duration::from_millis(500)).await;
    let job_info = job_manager.get_job_info(job_id.0).await?.unwrap();
    println!("\nJob status after pause:");
    println!("  Status: {:?}", job_info.status);
    println!("  Progress: {:.1}%", job_info.progress);
    
    // Wait a bit
    println!("\nWaiting 3 seconds while paused...");
    sleep(Duration::from_secs(3)).await;
    
    // Check progress hasn't changed
    let job_info_after = job_manager.get_job_info(job_id.0).await?.unwrap();
    println!("\nProgress after waiting: {:.1}% (should be same)", job_info_after.progress);
    
    // Resume the job
    println!("\nResuming job...");
    match job_manager.resume_job(job_id).await {
        Ok(_) => println!("✓ Job resumed successfully"),
        Err(e) => {
            println!("✗ Failed to resume job: {}", e);
            return Ok(());
        }
    }
    
    // Monitor progress
    println!("\nMonitoring progress for 5 seconds...");
    for i in 0..5 {
        sleep(Duration::from_secs(1)).await;
        let job_info = job_manager.get_job_info(job_id.0).await?.unwrap();
        println!("  Progress: {:.1}% - Status: {:?}", job_info.progress, job_info.status);
        
        if matches!(job_info.status, sd_core_new::infrastructure::jobs::types::JobStatus::Completed) {
            println!("\n✓ Job completed!");
            break;
        }
    }
    
    println!("\n✨ Demo completed!");
    
    Ok(())
}