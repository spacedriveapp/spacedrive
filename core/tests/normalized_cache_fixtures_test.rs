//! Normalized Cache Fixtures Test
//!
//! Generates real event and query data for TypeScript normalized cache tests.
//! Uses high-level Core APIs to create authentic backend responses.

use sd_core::{
	infra::{db::entities, event::Event, job::types::JobStatus},
	library::Library,
	Core,
};
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter};
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
				// Collect ResourceChanged/Batch events for both FILE and LOCATION resources
				match &event {
					Event::ResourceChanged {
						resource_type,
						metadata,
						..
					} => {
						if resource_type == "file" || resource_type == "location" {
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
						if resource_type == "file" || resource_type == "location" {
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
						if resource_type == "file" || resource_type == "location" {
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

/// Wait for indexing job to complete
async fn wait_for_indexing_completion(
	library: &Arc<Library>,
) -> Result<(), Box<dyn std::error::Error>> {
	let mut job_seen = false;
	let timeout = Duration::from_secs(30);
	let start = tokio::time::Instant::now();
	let mut last_entry_count = 0;
	let mut stable_iterations = 0;

	while start.elapsed() < timeout {
		let running = library.jobs().list_jobs(Some(JobStatus::Running)).await?;
		let completed = library.jobs().list_jobs(Some(JobStatus::Completed)).await?;

		if !running.is_empty() {
			job_seen = true;
		}

		let current_entries = entities::entry::Entity::find()
			.count(library.db().conn())
			.await?;

		// If job finished and entries are stable
		if job_seen && running.is_empty() && !completed.is_empty() && current_entries > 0 {
			if current_entries == last_entry_count {
				stable_iterations += 1;
				if stable_iterations >= 3 {
					tracing::info!(
						total_entries = current_entries,
						"Indexing completed and stabilized"
					);
					return Ok(());
				}
			} else {
				stable_iterations = 0;
			}
			last_entry_count = current_entries;
		}

		if start.elapsed() > timeout {
			return Err(format!(
				"Indexing timeout after {:?} (entries: {})",
				timeout, current_entries
			)
			.into());
		}

		tokio::time::sleep(Duration::from_millis(100)).await;
	}

	Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn capture_event_fixtures_for_typescript() -> Result<(), Box<dyn std::error::Error>> {
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
	let test_dir = temp_dir.path().join("test_location");
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

	// Set up event collection FIRST (before creating location)
	let collector = EventCollector::new();
	collector.start(&library);

	// Give event collector a moment to subscribe
	tokio::time::sleep(Duration::from_millis(100)).await;

	// Register device in database
	let device = core.device.to_device()?;
	let device_id = device.id;
	let device_name = device.name.clone();
	let device_slug = device.slug.clone();

	let _device_record = match entities::device::Entity::find()
		.filter(entities::device::Column::Uuid.eq(device.id))
		.one(library.db().conn())
		.await?
	{
		Some(existing) => existing,
		None => {
			let device_model: entities::device::ActiveModel = device.into();
			device_model.insert(library.db().conn()).await?
		}
	};

	tracing::info!("Device registered, creating location via LocationAddAction");

	// Build the path scope (using device_slug from above)
	let test_location_path = sd_core::domain::SdPath::Physical {
		device_slug: device_slug.clone(),
		path: test_dir.clone().into(),
	};

	// Use the actual production LocationAddAction to get real ResourceChanged events
	use sd_core::{
		infra::action::LibraryAction,
		ops::locations::add::action::{LocationAddAction, LocationAddInput},
	};

	let location_input = LocationAddInput {
		path: test_location_path.clone(),
		name: Some("Test Location".to_string()),
		mode: sd_core::ops::indexing::IndexMode::Deep,
		job_policies: None,
	};

	let action = LocationAddAction::from_input(location_input)
		.map_err(|e| format!("Failed to create action: {}", e))?;

	let location_output = action
		.execute(library.clone(), core.context.clone())
		.await
		.map_err(|e| format!("Failed to execute action: {:?}", e))?;

	let location_id = location_output.location_id;

	tracing::info!(
		location_id = %location_id,
		"Location created via action, waiting for indexing to complete"
	);

	// Wait for indexing to complete
	wait_for_indexing_completion(&library).await?;

	// Give events time to settle and for entry->file mapping to complete
	// The resource manager maps entry events to file events asynchronously
	tracing::info!("Waiting for entry->file event mapping to complete...");
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
		path: test_location_path.clone(),
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
			"test_location_path": test_dir.to_string_lossy(),
		}
	});

	// Query locations list for location event test case
	use sd_core::ops::locations::list::{LocationsListQuery, LocationsListQueryInput};

	let locations_query = LocationsListQuery::from_input(LocationsListQueryInput)?;
	let locations_response = locations_query
		.execute(core.context.clone(), session.clone())
		.await?;

	tracing::info!(
		locations_count = locations_response.locations.len(),
		"Locations query response"
	);

	// Extract location events
	let location_events: Vec<_> = events
		.iter()
		.filter(
			|e| matches!(e, Event::ResourceChanged { resource_type, .. } if resource_type == "location"),
		)
		.filter_map(|e| serde_json::to_value(e).ok())
		.collect();

	tracing::info!(
		location_events_count = location_events.len(),
		"Location events captured"
	);

	// Create test cases with initial state, events, and expected outcomes

	// Test Case 1: Exact mode - only direct children should be added
	let test_case_exact = json!({
		"name": "directory_view_exact_mode",
		"description": "Directory view should only show direct children, filtering out subdirectory files",
		"query": {
			"wireMethod": "query:files.directory_listing",
			"input": {
				"path": test_location_path,
				"limit": null,
				"include_hidden": false,
				"sort_by": "name"
			},
			"resourceType": "file",
			"pathScope": test_location_path,
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
			"wireMethod": "query:files.media_listing",
			"input": {
				"path": test_location_path,
				"include_descendants": true,
				"media_types": null,
				"limit": 10000,
				"sort_by": "name"
			},
			"resourceType": "file",
			"pathScope": test_location_path,
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

	// Test Case 3: Location events (no path filtering)
	let test_case_location = json!({
		"name": "location_updates",
		"description": "Location list should update when locations are created or modified",
		"query": {
			"wireMethod": "query:locations.list",
			"input": null,
			"resourceType": "location",
			"pathScope": null,
			"includeDescendants": false
		},
		"initial_state": {
			"locations": []
		},
		"events": location_events,
		"expected_final_state": {
			"locations": locations_response.locations
		},
		"expected_location_count": locations_response.locations.len(),
		"expected_location_names": locations_response.locations.iter().map(|l| &l.name).collect::<Vec<_>>()
	});

	fixtures["test_cases"] = json!([test_case_exact, test_case_recursive, test_case_location]);

	// Write fixtures to file
	let fixtures_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
		.parent()
		.unwrap()
		.join("packages/ts-client/src/__fixtures__");
	std::fs::create_dir_all(&fixtures_dir)?;

	let fixtures_path = fixtures_dir.join("backend_events.json");
	std::fs::write(&fixtures_path, serde_json::to_string_pretty(&fixtures)?)?;

	tracing::info!(
		fixtures_path = %fixtures_path.display(),
		"Fixtures written successfully"
	);

	println!("\n=== FIXTURE GENERATION COMPLETE ===");
	println!("Test cases generated: 3");
	println!("  - directory_view_exact_mode (direct children only)");
	println!("  - media_view_recursive_mode (all descendants)");
	println!("  - location_updates (location resource events)");
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
	println!("  - Location events: {}", location_events.len());
	println!("Direct children: {}", direct_children.len());
	println!("Subdirectory files: {}", subdirectory_files.len());
	println!("Fixtures written to: {}", fixtures_path.display());

	Ok(())
}
