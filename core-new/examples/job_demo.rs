//! Job system demonstration

use sd_core_new::{
    infrastructure::jobs::{manager::JobManager, traits::Job},
    operations::{
        file_ops::copy_job::FileCopyJob,
        indexing::indexer_job::{IndexerJob, IndexMode},
    },
    shared::types::SdPath,
};
use tempfile::TempDir;
use uuid::Uuid;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter("sd_core_new=debug")
        .init();
    
    println!("=== Spacedrive Job System Demo ===\n");
    
    // Create temporary directory for demo
    let temp_dir = TempDir::new()?;
    let data_dir = temp_dir.path().to_path_buf();
    
    // Initialize job manager (since Core/Library aren't implemented yet)
    println!("1. Initializing Job Manager...");
    // Create jobs subdirectory
    tokio::fs::create_dir_all(&data_dir).await?;
    let job_manager = JobManager::new(data_dir.clone()).await?;
    println!("   ✓ Job manager initialized");
    
    // For demonstration, we'll create a mock library ID
    let library_id = Uuid::new_v4();
    let device_id = Uuid::new_v4();
    println!("\n2. Demo library ID: {}", library_id);
    println!("   Demo device ID: {}", device_id);
    
    // Create some test files
    println!("\n3. Creating test files...");
    let test_dir = temp_dir.path().join("test_files");
    tokio::fs::create_dir_all(&test_dir).await?;
    
    for i in 1..=5 {
        let file_path = test_dir.join(format!("file_{}.txt", i));
        let content = format!("This is test file number {}", i);
        tokio::fs::write(&file_path, content).await?;
        println!("   ✓ Created {}", file_path.display());
    }
    
    // Demo 1: Create Jobs (without dispatching since we need library integration)
    println!("\n4. Creating Demo Jobs...");
    let copy_dest = temp_dir.path().join("copied_files");
    tokio::fs::create_dir_all(&copy_dest).await?;
    
    let sources: Vec<SdPath> = (1..=3)
        .map(|i| {
            let path = test_dir.join(format!("file_{}.txt", i));
            SdPath::new(device_id, path)
        })
        .collect();
    
    let destination = SdPath::new(device_id, copy_dest);
    
    let copy_job = FileCopyJob::new(sources, destination);
    println!("   ✓ File copy job created: {}", FileCopyJob::NAME);
    
    // Demonstrate job serialization
    let serialized = rmp_serde::to_vec(&copy_job)?;
    println!("   ✓ Job serialized: {} bytes", serialized.len());
    
    // Demo 2: Indexer Job
    println!("\n5. Creating Indexer Job...");
    let indexer_job = IndexerJob::new(
        library_id,
        SdPath::new(device_id, test_dir.clone()),
        IndexMode::Deep,
    );
    
    println!("   ✓ Indexer job created: {}", IndexerJob::NAME);
    
    // Test indexer serialization  
    let indexer_serialized = rmp_serde::to_vec(&indexer_job)?;
    println!("   ✓ Indexer serialized: {} bytes", indexer_serialized.len());
    
    // Demo 3: Job Database Operations
    println!("\n6. Testing Job Database...");
    
    // List any existing jobs (should be empty)
    let all_jobs = job_manager.list_jobs(None).await?;
    println!("   ✓ Found {} existing jobs in database", all_jobs.len());
    
    // Demo 4: Job System Architecture
    println!("\n7. Job System Architecture:");
    println!("   • JobManager: ✅ Initialized and ready");
    println!("   • JobDatabase: ✅ SQLite storage created");
    println!("   • Job Types:");
    println!("     - {}: Resumable file copying", FileCopyJob::NAME);
    println!("     - {}: Multi-phase content indexing", IndexerJob::NAME);
    println!("   • Key Features:");
    println!("     - Automatic serialization/deserialization");
    println!("     - Type-safe progress reporting");
    println!("     - Database persistence");
    println!("     - Minimal boilerplate (~50 lines vs 500+)");
    
    // Demo 5: Next Steps
    println!("\n8. Integration Status:");
    println!("   ✅ Job system infrastructure complete");
    println!("   ✅ Job traits and types defined");
    println!("   ✅ Database schema created");
    println!("   ✅ Example jobs implemented");
    println!("   🔄 TODO: Integrate with Core and Library");
    println!("   🔄 TODO: Implement job dispatch and execution");
    println!("   🔄 TODO: Add derive macros for registration");
    
    // Shutdown
    println!("\n9. Shutting down...");
    job_manager.shutdown().await?;
    
    println!("\n✅ Job system demo completed!");
    
    Ok(())
}