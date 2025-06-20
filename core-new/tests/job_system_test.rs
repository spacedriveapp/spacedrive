// //! Integration tests for the job system

// use sd_core_new::{
//     infrastructure::jobs::{
//         manager::JobManager,
//         traits::{Job, JobHandler},
//         types::{JobId, JobStatus},
//         context::JobContext,
//         error::{JobError, JobResult},
//         progress::Progress,
//         output::JobOutput,
//         prelude::JobProgress,
//     },
//     operations::{
//         file_ops::copy_job::FileCopyJob,
//         indexing::indexer_job::{IndexerJob, IndexMode},
//     },
//     shared::types::{SdPath, SdPathBatch},
// };
// use serde::{Deserialize, Serialize};
// use std::{
//     path::PathBuf,
//     time::Duration,
// };
// use tempfile::TempDir;
// use uuid::Uuid;

// // Simple test job for testing basic functionality
// #[derive(Debug, Serialize, Deserialize)]
// struct TestJob {
//     name: String,
//     sleep_ms: u64,
//     should_fail: bool,
//     counter: u32,
// }

// #[derive(Debug, Clone, Serialize, Deserialize)]
// struct TestProgress {
//     current: u32,
//     total: u32,
//     message: String,
// }

// impl JobProgress for TestProgress {}

// impl Job for TestJob {
//     const NAME: &'static str = "test_job";
//     const RESUMABLE: bool = true;
//     const DESCRIPTION: Option<&'static str> = Some("Simple test job");
// }

// #[async_trait::async_trait]
// impl JobHandler for TestJob {
//     type Output = TestOutput;

//     async fn run(&mut self, ctx: JobContext<'_>) -> JobResult<Self::Output> {
//         ctx.log(format!("Starting test job: {}", self.name));

//         if self.should_fail {
//             return Err(JobError::execution("Test failure"));
//         }

//         // Simulate work with progress updates
//         for i in 0..5 {
//             ctx.check_interrupt().await?;

//             self.counter += 1;

//             ctx.progress(Progress::structured(TestProgress {
//                 current: i + 1,
//                 total: 5,
//                 message: format!("Processing step {}", i + 1),
//             }));

//             if self.sleep_ms > 0 {
//                 tokio::time::sleep(Duration::from_millis(self.sleep_ms)).await;
//             }

//             // Checkpoint every 2 steps
//             if i % 2 == 1 {
//                 ctx.checkpoint().await?;
//             }
//         }

//         ctx.log("Test job completed successfully");

//         Ok(TestOutput {
//             name: self.name.clone(),
//             final_counter: self.counter,
//         })
//     }
// }

// #[derive(Debug, Serialize, Deserialize)]
// struct TestOutput {
//     name: String,
//     final_counter: u32,
// }

// impl From<TestOutput> for JobOutput {
//     fn from(output: TestOutput) -> Self {
//         JobOutput::custom(output)
//     }
// }

// impl TestJob {
//     fn new(name: String, sleep_ms: u64, should_fail: bool) -> Self {
//         Self {
//             name,
//             sleep_ms,
//             should_fail,
//             counter: 0,
//         }
//     }
// }

// #[tokio::test]
// async fn test_job_manager_initialization() {
//     let temp_dir = TempDir::new().unwrap();
//     let data_dir = temp_dir.path().to_path_buf();

//     // Initialize job manager
//     let job_manager = JobManager::new(data_dir.clone()).await.unwrap();

//     // Verify database was created
//     assert!(data_dir.join("jobs.db").exists());

//     // Test basic operations
//     let jobs = job_manager.list_jobs(None).await.unwrap();
//     assert!(jobs.is_empty());

//     // Shutdown cleanly
//     job_manager.shutdown().await.unwrap();
// }

// #[tokio::test]
// async fn test_job_serialization() {
//     // Test FileCopyJob serialization
//     let device_id = Uuid::new_v4();
//     let sources = vec![
//         SdPath::new(device_id, PathBuf::from("/test/file1.txt")),
//         SdPath::new(device_id, PathBuf::from("/test/file2.txt")),
//     ];
//     let destination = SdPath::new(device_id, PathBuf::from("/dest"));

//     let sources_batch = SdPathBatch::new(sources);
//     let copy_job = FileCopyJob::new(sources_batch, destination);

//     // Serialize and deserialize
//     let serialized = rmp_serde::to_vec(&copy_job).unwrap();
//     let deserialized: FileCopyJob = rmp_serde::from_slice(&serialized).unwrap();

//     assert_eq!(copy_job.sources.paths.len(), deserialized.sources.paths.len());

//     // Test IndexerJob serialization
//     let indexer_job = IndexerJob::new(
//         Uuid::new_v4(),
//         SdPath::new(device_id, PathBuf::from("/index/path")),
//         IndexMode::Deep,
//     );

//     let serialized = rmp_serde::to_vec(&indexer_job).unwrap();
//     let deserialized: IndexerJob = rmp_serde::from_slice(&serialized).unwrap();

//     // Verify key fields are preserved
//     assert_eq!(indexer_job.location_id, deserialized.location_id);

//     // Test TestJob serialization
//     let test_job = TestJob::new("test".to_string(), 100, false);
//     let serialized = rmp_serde::to_vec(&test_job).unwrap();
//     let deserialized: TestJob = rmp_serde::from_slice(&serialized).unwrap();

//     assert_eq!(test_job.name, deserialized.name);
//     assert_eq!(test_job.sleep_ms, deserialized.sleep_ms);
//     assert_eq!(test_job.should_fail, deserialized.should_fail);
// }

// #[tokio::test]
// async fn test_job_database_operations() {
//     let temp_dir = TempDir::new().unwrap();
//     let job_manager = JobManager::new(temp_dir.path().to_path_buf()).await.unwrap();

//     // Test listing empty jobs
//     let jobs = job_manager.list_jobs(None).await.unwrap();
//     assert!(jobs.is_empty());

//     // Test queued jobs (empty initially)
//     let queued = job_manager.list_jobs(Some(JobStatus::Queued)).await.unwrap();
//     assert!(queued.is_empty());

//     // Test job status filtering
//     let running_jobs = job_manager.list_jobs(Some(JobStatus::Running)).await.unwrap();
//     assert!(running_jobs.is_empty());

//     let completed_jobs = job_manager.list_jobs(Some(JobStatus::Completed)).await.unwrap();
//     assert!(completed_jobs.is_empty());

//     job_manager.shutdown().await.unwrap();
// }

// #[tokio::test]
// async fn test_job_constants_and_metadata() {
//     // Test job constants are properly defined
//     assert_eq!(FileCopyJob::NAME, "file_copy");
//     assert_eq!(FileCopyJob::RESUMABLE, true);

//     assert_eq!(IndexerJob::NAME, "indexer");
//     assert_eq!(IndexerJob::RESUMABLE, true);

//     assert_eq!(TestJob::NAME, "test_job");
//     assert_eq!(TestJob::RESUMABLE, true);

//     // Test job schemas
//     let copy_schema = FileCopyJob::schema();
//     assert_eq!(copy_schema.name, "file_copy");
//     assert_eq!(copy_schema.version, 1);

//     let indexer_schema = IndexerJob::schema();
//     assert_eq!(indexer_schema.name, "indexer");
//     assert_eq!(indexer_schema.version, 1);

//     let test_schema = TestJob::schema();
//     assert_eq!(test_schema.name, "test_job");
//     assert_eq!(test_schema.version, 1);
// }

// #[tokio::test]
// async fn test_job_progress_types() {
//     use sd_core_new::infrastructure::jobs::progress::Progress;

//     // Test percentage progress
//     let percentage = Progress::percentage(0.75);
//     match percentage {
//         Progress::Percentage(percent) => {
//             assert_eq!(percent, 0.75);
//         }
//         _ => panic!("Expected percentage progress"),
//     }

//     // Test structured progress
//     let test_progress = TestProgress {
//         current: 3,
//         total: 10,
//         message: "Test message".to_string(),
//     };

//     let structured = Progress::structured(test_progress.clone());
//     match structured {
//         Progress::Structured(data) => {
//             let deserialized: TestProgress = serde_json::from_value(data).unwrap();
//             assert_eq!(deserialized.current, test_progress.current);
//             assert_eq!(deserialized.total, test_progress.total);
//             assert_eq!(deserialized.message, test_progress.message);
//         }
//         _ => panic!("Expected structured progress"),
//     }
// }

// #[tokio::test]
// async fn test_job_error_types() {
//     // Test different error types
//     let io_error = std::io::Error::new(std::io::ErrorKind::NotFound, "File not found");
//     let job_error = JobError::from(io_error);

//     match job_error {
//         JobError::Io(e) => {
//             assert_eq!(e.kind(), std::io::ErrorKind::NotFound);
//         }
//         _ => panic!("Expected IO error"),
//     }

//     let execution_error = JobError::execution("Execution error message");
//     match execution_error {
//         JobError::ExecutionFailed(msg) => {
//             assert_eq!(msg, "Execution error message");
//         }
//         _ => panic!("Expected execution error"),
//     }

//     let interrupted_error = JobError::Interrupted;
//     match interrupted_error {
//         JobError::Interrupted => {
//             // Expected
//         }
//         _ => panic!("Expected interrupted error"),
//     }
// }

// #[tokio::test]
// async fn test_job_output_types() {
//     // Test different output types
//     let copied_output = JobOutput::FileCopy {
//         copied_count: 95,
//         total_bytes: 1024 * 1024,
//     };

//     match copied_output {
//         JobOutput::FileCopy { copied_count, total_bytes } => {
//             assert_eq!(copied_count, 95);
//             assert_eq!(total_bytes, 1024 * 1024);
//         }
//         _ => panic!("Expected file copy output"),
//     }

//     let indexed_output = JobOutput::Indexed {
//         total_files: 500,
//         total_dirs: 50,
//         total_bytes: 10 * 1024 * 1024,
//     };

//     match indexed_output {
//         JobOutput::Indexed { total_files, total_dirs, total_bytes } => {
//             assert_eq!(total_files, 500);
//             assert_eq!(total_dirs, 50);
//             assert_eq!(total_bytes, 10 * 1024 * 1024);
//         }
//         _ => panic!("Expected indexed output"),
//     }

//     let custom_data = serde_json::json!({
//         "test": "value",
//         "number": 42
//     });

//     let custom_output = JobOutput::Custom(custom_data.clone());

//     match custom_output {
//         JobOutput::Custom(data) => {
//             assert_eq!(data, custom_data);
//         }
//         _ => panic!("Expected custom output"),
//     }
// }

// #[tokio::test]
// async fn test_job_id_generation() {
//     // Test that JobIds are unique
//     let id1 = JobId::new();
//     let id2 = JobId::new();

//     assert_ne!(id1, id2);

//     // Test string conversion
//     let id_str = id1.to_string();
//     assert!(!id_str.is_empty());

//     // Test that IDs are valid UUIDs
//     let parsed = Uuid::parse_str(&id_str);
//     assert!(parsed.is_ok());
// }

// #[tokio::test]
// async fn test_job_status_transitions() {
//     // Test status equality and display
//     assert_eq!(JobStatus::Queued, JobStatus::Queued);
//     assert_ne!(JobStatus::Queued, JobStatus::Running);

//     // Test string conversion
//     assert_eq!(JobStatus::Queued.to_string(), "queued");
//     assert_eq!(JobStatus::Running.to_string(), "running");
//     assert_eq!(JobStatus::Completed.to_string(), "completed");
//     assert_eq!(JobStatus::Failed.to_string(), "failed");
//     assert_eq!(JobStatus::Cancelled.to_string(), "cancelled");
//     assert_eq!(JobStatus::Paused.to_string(), "paused");
// }

// #[tokio::test]
// async fn test_job_context_functionality() {
//     // This test verifies JobContext methods work correctly
//     // Since JobContext requires a full job execution environment,
//     // we test that the types and structures are correct

//     let temp_dir = TempDir::new().unwrap();
//     let job_manager = JobManager::new(temp_dir.path().to_path_buf()).await.unwrap();

//     // Test that the job manager can be created and shut down
//     job_manager.shutdown().await.unwrap();

//     // Test that job context structures are properly defined
//     // (Full context testing would require running actual jobs)
//     assert!(true); // Placeholder for context method tests
// }

// #[tokio::test]
// async fn test_job_system_concurrency() {
//     // Test that multiple JobManagers can be created independently
//     let temp_dir1 = TempDir::new().unwrap();
//     let temp_dir2 = TempDir::new().unwrap();

//     let manager1 = JobManager::new(temp_dir1.path().to_path_buf()).await.unwrap();
//     let manager2 = JobManager::new(temp_dir2.path().to_path_buf()).await.unwrap();

//     // Both should work independently
//     let jobs1 = manager1.list_jobs(None).await.unwrap();
//     let jobs2 = manager2.list_jobs(None).await.unwrap();

//     assert!(jobs1.is_empty());
//     assert!(jobs2.is_empty());

//     // Shutdown both
//     manager1.shutdown().await.unwrap();
//     manager2.shutdown().await.unwrap();
// }

// #[tokio::test]
// async fn test_job_system_persistence() {
//     let temp_dir = TempDir::new().unwrap();
//     let data_dir = temp_dir.path().to_path_buf();

//     // Create manager, verify database
//     let manager1 = JobManager::new(data_dir.clone()).await.unwrap();
//     assert!(data_dir.join("jobs.db").exists());
//     manager1.shutdown().await.unwrap();

//     // Create new manager with same directory - should reuse database
//     let manager2 = JobManager::new(data_dir.clone()).await.unwrap();
//     let jobs = manager2.list_jobs(None).await.unwrap();
//     assert!(jobs.is_empty()); // Should start empty but database should exist

//     manager2.shutdown().await.unwrap();
// }
