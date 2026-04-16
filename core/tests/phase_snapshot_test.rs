//! Comprehensive phase snapshot test
//!
//! This test indexes Desktop with Content mode and captures:
//! - All events received during each phase
//! - Directory listing query results after each phase
//! - Saves snapshots to disk for manual inspection

use sd_core::{
	infra::{db::entities, event::Event},
	location::{create_location, IndexMode, LocationCreateArgs},
	Core,
};
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter};
use std::{sync::Arc, time::Duration};
use tempfile::TempDir;

#[tokio::test]
async fn capture_phase_snapshots() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
	tracing_subscriber::fmt::init();
	eprintln!("\nPHASE SNAPSHOT TEST - Indexing Desktop\n");
	eprintln!("{}", "=".repeat(80));

	// Setup
	let temp_dir = TempDir::new()?;
	let core = Core::new(temp_dir.path().to_path_buf()).await?;

	let library = core
		.libraries
		.create_library("Phase Snapshot Test", None, core.context.clone())
		.await?;

	eprintln!("Created test library");

	// Get Desktop path
	let desktop_path = dirs::desktop_dir().expect("Could not find Desktop directory");

	eprintln!("Will index: {:?}", desktop_path);

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

	// Create output directory for snapshots in temp
	let temp_snapshot = TempDir::new()?;
	let snapshot_dir = temp_snapshot.path().to_path_buf();
	eprintln!("Snapshots will be saved to: {:?}\n", snapshot_dir);

	// Collect all events
	let events_collected = Arc::new(tokio::sync::Mutex::new(Vec::new()));
	let events_clone = events_collected.clone();
	let mut subscriber = core.events.subscribe();

	tokio::spawn(async move {
		while let Ok(event) = subscriber.recv().await {
			events_clone.lock().await.push(event);
		}
	});

	tokio::time::sleep(Duration::from_millis(100)).await;

	eprintln!("Starting Content mode indexing of Desktop...\n");

	let location_args = LocationCreateArgs {
		path: desktop_path.clone(),
		name: Some("Desktop".to_string()),
		index_mode: IndexMode::Content,
	};

	let _location_db_id = create_location(
		library.clone(),
		&core.events,
		location_args,
		device_record.id,
	)
	.await?;

	// Monitor job completion
	eprintln!("Waiting for indexing job to complete...\n");

	let mut indexing_started = false;
	let mut processing_complete = false;
	let mut content_phase_complete = false;
	let mut job_completed = false;

	let mut phase_events: Vec<(String, Vec<Event>)> = Vec::new();
	let mut current_phase_events: Vec<Event> = Vec::new();
	let mut current_phase_name = String::from("Initial");

	let mut last_processed_event_count = 0;

	// Poll for up to 2 minutes
	for iteration in 0..120 {
		tokio::time::sleep(Duration::from_secs(1)).await;

		let events = events_collected.lock().await;
		let all_events_count = events.len();

		// Process only new events since last check
		let new_events: Vec<Event> = if all_events_count > last_processed_event_count {
			events[last_processed_event_count..].to_vec()
		} else {
			vec![]
		};

		last_processed_event_count = all_events_count;
		drop(events); // Release lock

		for event in new_events {
			match &event {
				Event::IndexingStarted { .. } if !indexing_started => {
					eprintln!("Phase: Discovery Started");
					indexing_started = true;

					// Save previous phase
					if !current_phase_events.is_empty() {
						phase_events
							.push((current_phase_name.clone(), current_phase_events.clone()));
					}
					current_phase_name = "Discovery".to_string();
					current_phase_events.clear();
				}
				Event::JobProgress {
					job_type, message, ..
				} if job_type == "indexer" => {
					if let Some(msg) = message {
						if msg.contains("Processing entries") && !processing_complete {
							eprintln!("Phase: Processing");
							processing_complete = true;

							// Save previous phase
							phase_events
								.push((current_phase_name.clone(), current_phase_events.clone()));
							current_phase_name = "Processing".to_string();
							current_phase_events.clear();
						} else if msg.contains("Generating content identities")
							&& !content_phase_complete
						{
							eprintln!("Phase: Content Identification");
							content_phase_complete = true;

							// Save previous phase
							phase_events
								.push((current_phase_name.clone(), current_phase_events.clone()));
							current_phase_name = "Content".to_string();
							current_phase_events.clear();
						}
					}
				}
				Event::IndexingCompleted { .. } => {
					if !job_completed {
						eprintln!("Indexing Complete\n");
						job_completed = true;

						// Save final phase
						phase_events
							.push((current_phase_name.clone(), current_phase_events.clone()));
					}
				}
				Event::JobCompleted { job_type, .. } => {
					if job_type == "indexer" && !job_completed {
						eprintln!("Job Complete\n");
						job_completed = true;

						// Save final phase
						if !current_phase_events.is_empty() {
							phase_events
								.push((current_phase_name.clone(), current_phase_events.clone()));
						}
					}
				}
				_ => {}
			}

			current_phase_events.push(event);
		}

		if job_completed {
			break;
		}
	}

	eprintln!("Captured {} phases\n", phase_events.len());

	// Save phase events to disk with detailed structure
	for (phase_name, phase_event_list) in &phase_events {
		let mut phase_data = serde_json::json!({
			"phase_name": phase_name,
			"total_events": phase_event_list.len(),
			"file_batches": [],
			"all_files": [],
		});

		let mut all_files_in_phase = vec![];

		for event in phase_event_list {
			if let Event::ResourceChangedBatch {
				resource_type,
				resources,
				metadata,
			} = event
			{
				if resource_type == "file" {
					if let Some(files_array) = resources.as_array() {
						// Save batch metadata
						phase_data["file_batches"].as_array_mut().unwrap().push(
							serde_json::json!({
								"batch_size": files_array.len(),
								"sample_file": files_array.first(),
							}),
						);

						// Collect all files
						for file in files_array {
							all_files_in_phase.push(file.clone());
						}
					}
				}
			}
		}

		phase_data["all_files"] = serde_json::json!(all_files_in_phase);
		phase_data["total_files"] = serde_json::json!(all_files_in_phase.len());

		let filename = snapshot_dir.join(format!("phase_{}.json", phase_name));
		let json = serde_json::to_string_pretty(&phase_data)?;
		std::fs::write(&filename, json)?;
		eprintln!(
			"Saved phase {} with {} total files across {} batches",
			phase_name,
			all_files_in_phase.len(),
			phase_data["file_batches"].as_array().unwrap().len()
		);
	}

	// Query directory listing and save snapshot
	eprintln!("\nQuerying final directory listing state...\n");

	use sd_core::infra::db::entities::entry;

	// Just load all entries and construct Files to see what query would return
	let entries_with_content = entry::Entity::find()
		.filter(entry::Column::Kind.eq(0)) // Files only
		.all(db.conn())
		.await?;

	eprintln!(
		"Found {} file entries in database",
		entries_with_content.len()
	);

	// Save ALL entries to check ID matching
	let all_entries: Vec<_> = entries_with_content
		.iter()
		.map(|e| {
			serde_json::json!({
				"entry_id": e.id,
				"entry_uuid": e.uuid,
				"name": e.name,
				"content_id_fk": e.content_id,
				"extension": e.extension,
			})
		})
		.collect();

	let entries_file = snapshot_dir.join("db_entries_all.json");
	std::fs::write(&entries_file, serde_json::to_string_pretty(&all_entries)?)?;
	eprintln!("Saved {} database entries", all_entries.len());

	eprintln!("\n{}", "=".repeat(80));
	eprintln!("TEST COMPLETE");
	eprintln!("{}", "=".repeat(80));
	eprintln!("\nSnapshots saved to: {:?}", snapshot_dir);
	eprintln!("\nFiles created:");
	for entry in std::fs::read_dir(&snapshot_dir)? {
		let entry = entry?;
		eprintln!("   - {}", entry.file_name().to_string_lossy());
	}
	eprintln!();

	Ok(())
}
