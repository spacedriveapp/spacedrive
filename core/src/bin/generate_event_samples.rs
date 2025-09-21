//! Generate JSON samples of Event enum variants for proper Swift type generation

use chrono::Utc;
use sd_core::infra::event::{Event, FileOperation, FsRawEventKind};
use sd_core::infra::job::output::JobOutput;
use serde_json;
use std::path::PathBuf;
use uuid::Uuid;

fn main() -> Result<(), Box<dyn std::error::Error>> {
	println!("🎯 Generating Event JSON samples for Swift type generation");

	let samples = vec![
		// Core lifecycle events
		Event::CoreStarted,
		Event::CoreShutdown,
		// Library events
		Event::LibraryCreated {
			id: Uuid::new_v4(),
			name: "Sample Library".to_string(),
			path: PathBuf::from("/sample/path"),
		},
		Event::LibraryOpened {
			id: Uuid::new_v4(),
			name: "Sample Library".to_string(),
			path: PathBuf::from("/sample/path"),
		},
		Event::LibraryClosed {
			id: Uuid::new_v4(),
			name: "Sample Library".to_string(),
		},
		Event::LibraryDeleted {
			id: Uuid::new_v4(),
			name: "Sample Library".to_string(),
			deleted_data: true,
		},
		// Entry events
		Event::EntryCreated {
			library_id: Uuid::new_v4(),
			entry_id: Uuid::new_v4(),
		},
		Event::EntryModified {
			library_id: Uuid::new_v4(),
			entry_id: Uuid::new_v4(),
		},
		Event::EntryDeleted {
			library_id: Uuid::new_v4(),
			entry_id: Uuid::new_v4(),
		},
		Event::EntryMoved {
			library_id: Uuid::new_v4(),
			entry_id: Uuid::new_v4(),
			old_path: "/old/path".to_string(),
			new_path: "/new/path".to_string(),
		},
		// Job events
		Event::JobQueued {
			job_id: "job-123".to_string(),
			job_type: "Indexing".to_string(),
		},
		Event::JobStarted {
			job_id: "job-123".to_string(),
			job_type: "Indexing".to_string(),
		},
		Event::JobProgress {
			job_id: "job-123".to_string(),
			job_type: "Indexing".to_string(),
			progress: 0.5,
			message: Some("Processing files...".to_string()),
			generic_progress: None, // Keep as None - this is dynamic content that varies by job type
		},
		// Add a second JobProgress sample with generic_progress data for completeness
		Event::JobProgress {
			job_id: "job-456".to_string(),
			job_type: "FileCopy".to_string(),
			progress: 0.75,
			message: Some("Copying files...".to_string()),
			generic_progress: Some(serde_json::json!({
				"percentage": 0.75,
				"message": "Copying files...",
				"phase": "Processing"
			})), // Minimal example - real data structure varies by job type
		},
		Event::JobCompleted {
			job_id: "job-123".to_string(),
			job_type: "Indexing".to_string(),
			output: JobOutput::Success,
		},
		Event::JobFailed {
			job_id: "job-123".to_string(),
			job_type: "Indexing".to_string(),
			error: "Sample error".to_string(),
		},
		Event::JobCancelled {
			job_id: "job-123".to_string(),
			job_type: "Indexing".to_string(),
		},
		Event::JobPaused {
			job_id: "job-123".to_string(),
		},
		Event::JobResumed {
			job_id: "job-123".to_string(),
		},
		// Indexing events
		Event::IndexingStarted {
			location_id: Uuid::new_v4(),
		},
		Event::IndexingProgress {
			location_id: Uuid::new_v4(),
			processed: 100,
			total: Some(200),
		},
		Event::IndexingCompleted {
			location_id: Uuid::new_v4(),
			total_files: 150,
			total_dirs: 20,
		},
		Event::IndexingFailed {
			location_id: Uuid::new_v4(),
			error: "Sample indexing error".to_string(),
		},
		// Device events
		Event::DeviceConnected {
			device_id: Uuid::new_v4(),
			device_name: "Sample Device".to_string(),
		},
		Event::DeviceDisconnected {
			device_id: Uuid::new_v4(),
		},
		// Raw filesystem events
		Event::FsRawChange {
			library_id: Uuid::new_v4(),
			kind: FsRawEventKind::Create {
				path: PathBuf::from("/sample/file.txt"),
			},
		},
		// Legacy events
		Event::LocationAdded {
			library_id: Uuid::new_v4(),
			location_id: Uuid::new_v4(),
			path: PathBuf::from("/sample/location"),
		},
		Event::LocationRemoved {
			library_id: Uuid::new_v4(),
			location_id: Uuid::new_v4(),
		},
		Event::FilesIndexed {
			library_id: Uuid::new_v4(),
			location_id: Uuid::new_v4(),
			count: 42,
		},
		Event::ThumbnailsGenerated {
			library_id: Uuid::new_v4(),
			count: 15,
		},
		Event::FileOperationCompleted {
			library_id: Uuid::new_v4(),
			operation: FileOperation::Copy,
			affected_files: 10,
		},
		Event::FilesModified {
			library_id: Uuid::new_v4(),
			paths: vec![PathBuf::from("/file1.txt"), PathBuf::from("/file2.txt")],
		},
		// Log events
		Event::LogMessage {
			timestamp: Utc::now(),
			level: "info".to_string(),
			target: "sd_core".to_string(),
			message: "Sample log message".to_string(),
			job_id: Some("job-123".to_string()),
			library_id: Some(Uuid::new_v4()),
		},
		// Custom events
		Event::Custom {
			event_type: "sample_custom".to_string(),
			data: serde_json::json!({"key": "value"}),
		},
	];

	// Write samples as a JSON array (not wrapped in an object)
	let output_path = "packages/event_samples.json";
	std::fs::write(output_path, serde_json::to_string_pretty(&samples)?)?;

	println!(
		"✅ Generated {} event samples to {}",
		samples.len(),
		output_path
	);
	println!("📄 Sample formats:");

	// Show a few examples
	for (i, sample) in samples.iter().take(3).enumerate() {
		println!("  {}. {}", i + 1, serde_json::to_string(sample)?);
	}

	Ok(())
}
