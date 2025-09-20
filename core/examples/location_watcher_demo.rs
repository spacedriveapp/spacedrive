//! Location Watcher Demo
//!
//! This example demonstrates how to use the location watcher to monitor
//! file system changes in real-time.

use sd_core::{infra::event::Event, Core};
use std::path::PathBuf;
use tokio::time::{sleep, Duration};
use tracing::info;
use uuid::Uuid;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
	// Initialize logging
	tracing_subscriber::fmt::init();

	info!("Starting Location Watcher Demo");

	// Initialize core
	let core = Core::new().await?;
	info!("Core initialized successfully");

	// Create a test library
	let library = core
		.libraries
		.create_library("Watcher Demo Library", None, core.context.clone())
		.await?;

	let library_id = library.id();
	info!("Created demo library: {}", library_id);

	// Add a location to watch
	let watch_dir = PathBuf::from("./data/spacedrive_watcher_demo");
	tokio::fs::create_dir_all(&watch_dir).await?;

	let location_id = Uuid::new_v4();
	core.add_watched_location(location_id, library_id, watch_dir.clone(), true)
		.await?;
	info!("Added watched location: {}", watch_dir.display());

	// Subscribe to events
	let mut event_subscriber = core.events.subscribe();

	// Spawn event listener
	let events_handle = tokio::spawn(async move {
		info!("Event listener started");

		while let Ok(event) = event_subscriber.recv().await {
			match event {
				Event::EntryCreated {
					library_id,
					entry_id,
				} => {
					info!(
						"File created - Library: {}, Entry: {}",
						library_id, entry_id
					);
				}
				Event::EntryModified {
					library_id,
					entry_id,
				} => {
					info!(
						"✏️  File modified - Library: {}, Entry: {}",
						library_id, entry_id
					);
				}
				Event::EntryDeleted {
					library_id,
					entry_id,
				} => {
					info!(
						"🗑️  File deleted - Library: {}, Entry: {}",
						library_id, entry_id
					);
				}
				Event::EntryMoved {
					library_id,
					entry_id,
					old_path,
					new_path,
				} => {
					info!(
						"File moved - Library: {}, Entry: {}, {} -> {}",
						library_id, entry_id, old_path, new_path
					);
				}
				_ => {} // Ignore other events for this demo
			}
		}
	});

	// Simulate file operations
	info!("Starting file operations simulation...");

	// Create a test file
	let test_file = watch_dir.join("test_file.txt");
	tokio::fs::write(&test_file, "Hello, Spacedrive!").await?;
	info!("Created test file: {}", test_file.display());
	sleep(Duration::from_millis(200)).await;

	// Modify the file
	tokio::fs::write(&test_file, "Hello, Spacedrive! Modified content.").await?;
	info!("Modified test file");
	sleep(Duration::from_millis(200)).await;

	// Create a directory
	let test_dir = watch_dir.join("test_directory");
	tokio::fs::create_dir(&test_dir).await?;
	info!("Created test directory: {}", test_dir.display());
	sleep(Duration::from_millis(200)).await;

	// Create a file in the directory
	let nested_file = test_dir.join("nested_file.txt");
	tokio::fs::write(&nested_file, "Nested file content").await?;
	info!("Created nested file: {}", nested_file.display());
	sleep(Duration::from_millis(200)).await;

	// Rename the file
	let renamed_file = test_dir.join("renamed_file.txt");
	tokio::fs::rename(&nested_file, &renamed_file).await?;
	info!(
		"Renamed file: {} -> {}",
		nested_file.display(),
		renamed_file.display()
	);
	sleep(Duration::from_millis(200)).await;

	// Delete the file
	tokio::fs::remove_file(&renamed_file).await?;
	info!("Deleted file: {}", renamed_file.display());
	sleep(Duration::from_millis(200)).await;

	// Delete the directory
	tokio::fs::remove_dir(&test_dir).await?;
	info!("Deleted directory: {}", test_dir.display());
	sleep(Duration::from_millis(200)).await;

	// Delete the original test file
	tokio::fs::remove_file(&test_file).await?;
	info!("Deleted test file: {}", test_file.display());

	// Give some time for all events to be processed
	sleep(Duration::from_secs(2)).await;

	// Display current watched locations
	let watched_locations = core.get_watched_locations().await;
	info!("Currently watching {} locations:", watched_locations.len());
	for location in watched_locations {
		info!(
			"  - {} ({}): {} [{}]",
			location.id,
			location.library_id,
			location.path.display(),
			if location.enabled {
				"enabled"
			} else {
				"disabled"
			}
		);
	}

	// Clean up
	core.remove_watched_location(location_id).await?;
	info!("Removed watched location");

	// Clean up directory
	if watch_dir.exists() {
		tokio::fs::remove_dir_all(&watch_dir).await?;
		info!("Cleaned up demo directory");
	}

	// Stop event listener
	events_handle.abort();

	// Shutdown core
	core.shutdown().await?;
	info!("Demo completed successfully");

	Ok(())
}
