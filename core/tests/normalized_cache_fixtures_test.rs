//! Normalized Cache Fixtures Test
//!
//! Generates real event and query data for TypeScript normalized cache tests.
//! Uses high-level Core APIs to create authentic backend responses.
//!
//! ## Fixture Generation
//!
//! By default, fixtures are written to the temp directory (following testing conventions).
//! To update the source fixtures used by TypeScript tests, run with:
//!
//! ```bash
//! SD_REGENERATE_FIXTURES=1 cargo test normalized_cache_fixtures_test --nocapture
//! ```

use sd_core::{
	infra::{action::LibraryAction, event::Event, job::types::JobId},
	library::Library,
	ops::sources::track::{TrackSourceAction, TrackSourceInput},
	Core,
};
use serde_json::json;
use std::{path::PathBuf, sync::Arc, time::Duration};
use tempfile::TempDir;
use tokio::sync::Mutex;

/// Event collector for capturing real backend events
struct EventCollector {
	events: Arc<Mutex<Vec<Event>>>,
}

impl EventCollector {
	fn new() -> Self {
		Self {
			events: Arc::new(Mutex::new(Vec::new())),
		}
	}

	/// Start collecting events from event bus
	fn start(&self, library: &Arc<Library>) {
		let events = self.events.clone();
		let mut subscriber = library.event_bus().subscribe();

		tokio::spawn(async move {
			while let Ok(event) = subscriber.recv().await {
				// Collect ResourceChanged/Batch events for FILE resources
				match &event {
					Event::ResourceChanged {
						resource_type,
						metadata,
						..
					} => {
						if resource_type == "file" {
							tracing::info!(
								"Collected ResourceChanged event for {}, has_metadata={}",
								resource_type,
								metadata.is_some()
							);
							events.lock().await.push(event);
						}
					}
					Event::ResourceChangedBatch {
						resource_type,
						metadata,
						..
					} => {
						if resource_type == "file" {
							let has_paths = metadata
								.as_ref()
								.map(|m| !m.affected_paths.is_empty())
								.unwrap_or(false);
							tracing::info!(
								"Collected ResourceChangedBatch event for {}, has_affected_paths={}",
								resource_type,
								has_paths
							);
							events.lock().await.push(event);
						}
					}
					Event::ResourceDeleted { resource_type, .. } => {
						if resource_type == "file" {
							events.lock().await.push(event);
						}
					}
					Event::JobStarted { .. }
					| Event::JobCompleted { .. }
					| Event::JobFailed { .. } => {
						events.lock().await.push(event);
					}
					_ => {}
				}
			}
		});
	}

	async fn get_events(&self) -> Vec<Event> {
		self.events.lock().await.clone()
	}
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn capture_event_fixtures_for_typescript(
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
	// Initialize tracing
	let _ = tracing_subscriber::fmt()
		.with_env_filter(
			tracing_subscriber::EnvFilter::try_from_default_env()
				.unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("sd_core=debug")),
		)
		.try_init();

	let temp_dir = TempDir::new()?;
	let core = Core::new(temp_dir.path().to_path_buf()).await?;

	// Create test directory structure
	let test_dir = temp_dir.path().join("test_source");
	std::fs::create_dir_all(&test_dir)?;

	// Create direct children (root level files)
	std::fs::write(test_dir.join("direct_child1.txt"), "This is a direct child")?;
	std::fs::write(test_dir.join("direct_child2.txt"), "Another direct child")?;

	// Create subdirectory with files
	std::fs::create_dir_all(test_dir.join("subfolder"))?;
	std::fs::write(
		test_dir.join("subfolder/grandchild1.txt"),
		"This is a grandchild",
	)?;
	std::fs::write(
		test_dir.join("subfolder/grandchild2.txt"),
		"Another grandchild",
	)?;

	// Create nested subdirectory
	std::fs::create_dir_all(test_dir.join("subfolder/nested"))?;
	std::fs::write(
		test_dir.join("subfolder/nested/deep_file.txt"),
		"Deep nested file",
	)?;

	tracing::info!(
		test_dir = %test_dir.display(),
		"Created test directory structure"
	);

	// Create library
	let library = core
		.libraries
		.create_library("Fixture Test", None, core.context.clone())
		.await?;

	// Set up event collection FIRST (before tracking the source)
	let collector = EventCollector::new();
	collector.start(&library);

	// Give event collector a moment to subscribe
	tokio::time::sleep(Duration::from_millis(100)).await;

	let device = core.device.to_device()?;
	let device_id = device.id;
	let device_name = device.name.clone();
	let device_slug = device.slug.clone();

	let source_path = sd_core::domain::SdPath::Physical {
		device_slug: device_slug.clone(),
		path: test_dir.clone().into(),
	};

	// The production action, so the events are the ones the frontend receives
	let track_output = TrackSourceAction::from_input(TrackSourceInput {
		path: test_dir.clone(),
		name: Some("Test Source".to_string()),
		unfiltered: false,
	})
	.map_err(|e| format!("Failed to create action: {}", e))?
	.execute(library.clone(), core.context.clone())
	.await
	.map_err(|e| format!("Failed to execute action: {:?}", e))?;

	tracing::info!(
		source_id = %track_output.id,
		"Source tracked, waiting for its walk to complete"
	);

	// A walk that already finished has left the running set
	let walk_id = track_output.job_id.ok_or("tracking dispatched no walk")?;
	if let Some(walk) = library.jobs().get_job(JobId(walk_id)).await {
		walk.wait().await?;
	}

	// Content identification runs behind the walk and announces the files it
	// hashes; give it and the event bus time to settle
	tokio::time::sleep(Duration::from_secs(2)).await;

	// Get collected events
	let events = collector.get_events().await;

	tracing::info!(total_events = events.len(), "Collected events");

	// Log what types we got
	for event in &events {
		match event {
			Event::ResourceChanged {
				resource_type,
				metadata,
				..
			} => {
				tracing::info!(
					"Event: ResourceChanged type={}, has_metadata={}",
					resource_type,
					metadata.is_some()
				);
			}
			Event::ResourceChangedBatch {
				resource_type,
				metadata,
				..
			} => {
				let path_count = metadata
					.as_ref()
					.map(|m| m.affected_paths.len())
					.unwrap_or(0);
				tracing::info!(
					"Event: ResourceChangedBatch type={}, affected_paths={}",
					resource_type,
					path_count
				);
			}
			Event::JobCompleted { job_type, .. } => {
				tracing::info!("Event: JobCompleted type={}", job_type);
			}
			_ => {}
		}
	}

	// Query the directory using the actual LibraryQuery (same as frontend)
	use sd_core::{
		infra::query::LibraryQuery,
		ops::files::query::directory_listing::{
			DirectoryListingInput, DirectoryListingQuery, DirectorySortBy,
		},
	};

	// Create session context with library (using device_id and device_name from above)
	let base_session =
		sd_core::infra::api::SessionContext::device_session(device_id, device_name.clone());
	let session = base_session.with_library(library.id());

	// Execute the actual directory listing query (same as frontend)
	let query_input = DirectoryListingInput {
		path: source_path.clone(),
		folders_first: Some(false),
		limit: None,
		include_hidden: Some(false),
		sort_by: DirectorySortBy::Name,
	};

	let query = DirectoryListingQuery::from_input(query_input)?;
	let directory_response = query.execute(core.context.clone(), session.clone()).await?;

	tracing::info!(
		total_files_in_response = directory_response.files.len(),
		"Directory query executed successfully"
	);

	// Separate into direct children and subdirectory files
	let direct_children: Vec<_> = directory_response
		.files
		.iter()
		.filter(|f| f.name.starts_with("direct_child"))
		.cloned()
		.collect();

	let subdirectory_files: Vec<_> = directory_response
		.files
		.iter()
		.filter(|f| f.name.contains("grandchild") || f.name.contains("deep_file"))
		.cloned()
		.collect();

	tracing::info!(
		direct_children = direct_children.len(),
		subdirectory_files = subdirectory_files.len(),
		"File distribution in query response"
	);

	// Extract fixtures with complete test cases
	let mut fixtures = json!({
		"test_cases": [],
		"events": {},
		"metadata": {
			"generated_at": chrono::Utc::now().to_rfc3339(),
			"device_slug": device_slug,
			"source_path": test_dir.to_string_lossy(),
		}
	});

	// Create test cases with initial state, events, and expected outcomes

	// Test Case 1: Exact mode - only direct children should be added
	let test_case_exact = json!({
		"name": "directory_view_exact_mode",
		"description": "Directory view should only show direct children, filtering out subdirectory files",
		"query": {
			"query": "files.directory_listing",
			"input": {
				"path": source_path,
				"limit": null,
				"include_hidden": false,
				"sort_by": "name"
			},
			"resourceType": "file",
			"pathScope": source_path,
			"includeDescendants": false
		},
		"initial_state": {
			"files": []
		},
		"events": events.iter().filter_map(|e| {
			if matches!(e, Event::ResourceChangedBatch { resource_type, .. } if resource_type == "file") {
				serde_json::to_value(e).ok()
			} else {
				None
			}
		}).collect::<Vec<_>>(),
		"expected_final_state": {
			"files": direct_children
		},
		"expected_file_count": direct_children.len(),
		"expected_file_names": direct_children.iter().map(|f| &f.name).collect::<Vec<_>>()
	});

	// Test Case 2: Recursive mode - all descendants should be included
	let test_case_recursive = json!({
		"name": "media_view_recursive_mode",
		"description": "Media view should show all files recursively including subdirectories",
		"query": {
			"query": "files.media_listing",
			"input": {
				"path": source_path,
				"include_descendants": true,
				"media_types": null,
				"limit": 10000,
				"sort_by": "name"
			},
			"resourceType": "file",
			"pathScope": source_path,
			"includeDescendants": true
		},
		"initial_state": {
			"files": []
		},
		"events": events.iter().filter_map(|e| {
			if matches!(e, Event::ResourceChangedBatch { resource_type, .. } if resource_type == "file") {
				serde_json::to_value(e).ok()
			} else {
				None
			}
		}).collect::<Vec<_>>(),
		"expected_final_state": {
			"files": directory_response.files
		},
		"expected_file_count": directory_response.files.len(),
		"expected_file_names": directory_response.files.iter().map(|f| &f.name).collect::<Vec<_>>()
	});

	fixtures["test_cases"] = json!([test_case_exact, test_case_recursive]);

	// Write fixtures to temp directory (follows testing conventions)
	let temp_fixtures_path = temp_dir.path().join("backend_events.json");
	let fixtures_json = serde_json::to_string_pretty(&fixtures)?;
	std::fs::write(&temp_fixtures_path, &fixtures_json)?;

	tracing::info!(
		fixtures_path = %temp_fixtures_path.display(),
		"Fixtures written to temp directory"
	);

	// Only copy to source if explicitly requested (similar to snapshot system)
	if std::env::var("SD_REGENERATE_FIXTURES").is_ok() {
		let source_fixtures_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
			.parent()
			.unwrap()
			.join("packages/ts-client/src/__fixtures__");
		std::fs::create_dir_all(&source_fixtures_dir)?;

		let source_fixtures_path = source_fixtures_dir.join("backend_events.json");
		std::fs::copy(&temp_fixtures_path, &source_fixtures_path)?;

		tracing::info!(
			source_path = %source_fixtures_path.display(),
			"Fixtures copied to source tree (SD_REGENERATE_FIXTURES=1)"
		);
		println!("\n=== FIXTURES COPIED TO SOURCE ===");
		println!("Source path: {}", source_fixtures_path.display());
	} else {
		println!("\n=== FIXTURE GENERATION COMPLETE ===");
		println!("Note: Fixtures written to temp directory only.");
		println!("To update source fixtures, run with: SD_REGENERATE_FIXTURES=1");
	}

	println!("\nTest cases generated: 2");
	println!("  - directory_view_exact_mode (direct children only)");
	println!("  - media_view_recursive_mode (all descendants)");
	println!("Total events captured: {}", events.len());
	println!(
		"  - File events: {}",
		events
			.iter()
			.filter(
				|e| matches!(e, Event::ResourceChangedBatch { resource_type, .. } if resource_type == "file")
			)
			.count()
	);
	println!("Direct children: {}", direct_children.len());
	println!("Subdirectory files: {}", subdirectory_files.len());
	println!("Temp fixtures: {}", temp_fixtures_path.display());

	Ok(())
}
