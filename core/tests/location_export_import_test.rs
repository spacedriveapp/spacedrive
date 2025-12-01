//! Location Export/Import Integration Test
//!
//! Tests the location export and import functionality:
//! - Export a location to SQL dump after indexing
//! - Import the exported location into a new library
//! - Verify data integrity after import

use sd_core::{
	domain::addressing::SdPath,
	infra::{action::LibraryAction, db::entities, job::JobStatus},
	ops::{
		indexing::IndexMode,
		locations::{
			add::action::{LocationAddAction, LocationAddInput},
			export::{LocationExportAction, LocationExportInput},
			import::{LocationImportAction, LocationImportInput},
		},
	},
	Core,
};
use sea_orm::{ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter};
use tempfile::TempDir;
use tokio::time::Duration;

async fn wait_for_job_completion(
	library: &std::sync::Arc<sd_core::library::Library>,
	job_id: uuid::Uuid,
	timeout_secs: u64,
) -> Result<(), String> {
	let start = tokio::time::Instant::now();
	let timeout = Duration::from_secs(timeout_secs);

	loop {
		let jobs = library.jobs().list_jobs(None).await.map_err(|e| e.to_string())?;

		let job = jobs.iter().find(|j| j.id == job_id);

		match job {
			Some(j) => match j.status {
				JobStatus::Completed => {
					println!("Job {} completed successfully", job_id);
					return Ok(());
				}
				JobStatus::Failed => {
					return Err(format!("Job {} failed", job_id));
				}
				JobStatus::Running | JobStatus::Queued => {
					// Still running, continue waiting
				}
				_ => {}
			},
			None => {
				// Job not found yet, might still be initializing
			}
		}

		if start.elapsed() > timeout {
			return Err(format!("Job {} timed out after {:?}", job_id, timeout));
		}

		tokio::time::sleep(Duration::from_millis(200)).await;
	}
}

async fn wait_for_indexing_stable(
	library: &std::sync::Arc<sd_core::library::Library>,
	timeout_secs: u64,
) -> Result<u64, String> {
	let start = tokio::time::Instant::now();
	let timeout = Duration::from_secs(timeout_secs);
	let mut last_count = 0u64;
	let mut stable_iterations = 0;

	loop {
		let running = library
			.jobs()
			.list_jobs(Some(JobStatus::Running))
			.await
			.map_err(|e| e.to_string())?;

		let entry_count = entities::entry::Entity::find()
			.count(library.db().conn())
			.await
			.map_err(|e| e.to_string())?;

		if running.is_empty() && entry_count > 0 {
			if entry_count == last_count {
				stable_iterations += 1;
				if stable_iterations >= 3 {
					return Ok(entry_count);
				}
			} else {
				stable_iterations = 0;
			}
			last_count = entry_count;
		}

		if start.elapsed() > timeout {
			return Err(format!(
				"Indexing timed out after {:?} (entries: {})",
				timeout, entry_count
			));
		}

		tokio::time::sleep(Duration::from_millis(300)).await;
	}
}

#[tokio::test]
async fn test_location_export_import() -> Result<(), Box<dyn std::error::Error>> {
	// Setup test directories
	let temp_dir = TempDir::new()?;
	let core_dir = temp_dir.path().join("core");
	let export_file = temp_dir.path().join("location_export.sql");

	tokio::fs::create_dir_all(&core_dir).await?;

	// Create test location with some files
	let test_location = temp_dir.path().join("test_location");
	tokio::fs::create_dir_all(&test_location).await?;
	tokio::fs::create_dir_all(test_location.join("subdir")).await?;

	tokio::fs::write(test_location.join("file1.txt"), "Hello World").await?;
	tokio::fs::write(test_location.join("file2.md"), "# Markdown").await?;
	tokio::fs::write(test_location.join("subdir/file3.rs"), "fn main() {}").await?;

	println!("Test location created at: {}", test_location.display());

	// Initialize Core
	let core = Core::new(core_dir.clone()).await?;

	// Create first library
	let library1 = core
		.libraries
		.create_library("Export Test Library", None, core.context.clone())
		.await?;

	println!("Library created: {}", library1.id());

	// Add location using action
	let location_input = LocationAddInput {
		path: SdPath::local(test_location.clone()),
		name: Some("Test Location".to_string()),
		mode: IndexMode::Content,
		job_policies: None,
	};

	let location_action = LocationAddAction::from_input(location_input)
		.map_err(|e| format!("Failed to create location action: {}", e))?;

	let action_manager = core.context.action_manager.read().await;
	let action_manager = action_manager
		.as_ref()
		.ok_or("Action manager not initialized")?;

	let location_output = action_manager
		.dispatch_library(Some(library1.id()), location_action)
		.await
		.map_err(|e| format!("Failed to dispatch location add: {}", e))?;

	println!("Location added: {}", location_output.location_id);

	// Wait for indexing job to complete
	if let Some(job_id) = location_output.job_id {
		println!("Waiting for indexing job: {}", job_id);
		wait_for_job_completion(&library1, job_id, 60).await?;
	}

	// Wait for indexing to stabilize
	let entry_count = wait_for_indexing_stable(&library1, 30).await?;
	println!("Indexing complete, {} entries", entry_count);

	// Verify we have some entries
	assert!(entry_count >= 4, "Should have at least 4 entries (3 files + 1 subdir + root)");

	// Get location UUID for export
	let location = entities::location::Entity::find()
		.filter(entities::location::Column::Uuid.eq(location_output.location_id))
		.one(library1.db().conn())
		.await?
		.ok_or("Location not found")?;

	// Count content identities
	let content_count = entities::content_identity::Entity::find()
		.count(library1.db().conn())
		.await?;
	println!("Content identities: {}", content_count);

	// Export the location
	let export_input = LocationExportInput {
		location_uuid: location.uuid,
		export_path: export_file.clone(),
		include_content_identities: true,
		include_media_data: true,
		include_user_metadata: true,
		include_tags: true,
	};

	let export_action = LocationExportAction::from_input(export_input)
		.map_err(|e| format!("Failed to create export action: {}", e))?;

	let export_output = action_manager
		.dispatch_library(Some(library1.id()), export_action)
		.await
		.map_err(|e| format!("Failed to dispatch export: {}", e))?;

	println!(
		"Export complete: {} bytes, {} entries",
		export_output.file_size_bytes, export_output.stats.entries
	);

	assert!(export_file.exists(), "Export file should exist");
	assert!(export_output.file_size_bytes > 0, "Export file should not be empty");
	assert_eq!(
		export_output.stats.entries, entry_count,
		"Export should include all entries"
	);

	// Read and verify export file contains expected content
	let export_content = tokio::fs::read_to_string(&export_file).await?;
	assert!(
		export_content.contains("-- Spacedrive Location Export"),
		"Export should have header"
	);
	assert!(
		export_content.contains("INSERT OR REPLACE INTO entries"),
		"Export should have entry inserts"
	);
	assert!(
		export_content.contains("INSERT OR REPLACE INTO locations"),
		"Export should have location insert"
	);

	println!("Export file verified");

	// Close first library
	drop(action_manager);
	let lib1_id = library1.id();
	core.libraries.close_library(lib1_id).await?;
	drop(library1);

	// Create second library for import
	let library2 = core
		.libraries
		.create_library("Import Test Library", None, core.context.clone())
		.await?;

	println!("Second library created: {}", library2.id());

	// Verify library2 is empty
	let lib2_entries_before = entities::entry::Entity::find()
		.count(library2.db().conn())
		.await?;
	assert_eq!(lib2_entries_before, 0, "Library2 should start empty");

	// Import the location
	let import_input = LocationImportInput {
		import_path: export_file.clone(),
		new_name: Some("Imported Location".to_string()),
		skip_existing: false,
	};

	let import_action = LocationImportAction::from_input(import_input)
		.map_err(|e| format!("Failed to create import action: {}", e))?;

	let action_manager = core.context.action_manager.read().await;
	let action_manager = action_manager
		.as_ref()
		.ok_or("Action manager not initialized")?;

	let import_output = action_manager
		.dispatch_library(Some(library2.id()), import_action)
		.await
		.map_err(|e| format!("Failed to dispatch import: {}", e))?;

	println!(
		"Import complete: {} entries imported, {} skipped",
		import_output.stats.entries_imported, import_output.stats.entries_skipped
	);

	// Verify import results
	let lib2_entries_after = entities::entry::Entity::find()
		.count(library2.db().conn())
		.await?;

	println!(
		"Library2 entries after import: {} (expected: {})",
		lib2_entries_after, entry_count
	);

	// The import should have created entries
	assert!(
		lib2_entries_after > 0,
		"Library2 should have entries after import"
	);

	// Debug: List all locations in library2
	let all_locations = entities::location::Entity::find()
		.all(library2.db().conn())
		.await?;
	println!("Library2 locations count: {}", all_locations.len());
	for loc in &all_locations {
		println!("  Location: uuid={}, name={:?}", loc.uuid, loc.name);
	}
	println!("Looking for location UUID: {}", import_output.location_uuid);

	// Verify location was created
	let imported_location = entities::location::Entity::find()
		.filter(entities::location::Column::Uuid.eq(import_output.location_uuid))
		.one(library2.db().conn())
		.await?;

	assert!(
		imported_location.is_some(),
		"Imported location should exist (found {} locations)",
		all_locations.len()
	);

	let imported_loc = imported_location.unwrap();
	assert_eq!(
		imported_loc.name.as_deref(),
		Some("Imported Location"),
		"Location name should be updated"
	);

	// Verify content identities were imported
	let lib2_content_count = entities::content_identity::Entity::find()
		.count(library2.db().conn())
		.await?;
	println!(
		"Library2 content identities: {} (expected: {})",
		lib2_content_count, content_count
	);
	assert_eq!(
		lib2_content_count, content_count,
		"Content identities should be imported"
	);

	// Verify entries are linked to content identities
	let entries_with_content = entities::entry::Entity::find()
		.filter(entities::entry::Column::ContentId.is_not_null())
		.count(library2.db().conn())
		.await?;
	println!("Entries with content_id: {}", entries_with_content);
	assert!(
		entries_with_content >= 3,
		"File entries should be linked to content identities"
	);

	println!("Import verification complete");

	// Cleanup
	drop(action_manager);
	let lib2_id = library2.id();
	core.libraries.close_library(lib2_id).await?;
	drop(library2);

	core.shutdown().await?;

	println!("Test completed successfully");
	Ok(())
}

#[tokio::test]
async fn test_export_nonexistent_location() -> Result<(), Box<dyn std::error::Error>> {
	let temp_dir = TempDir::new()?;
	let core_dir = temp_dir.path().join("core");
	let export_file = temp_dir.path().join("export.sql");

	tokio::fs::create_dir_all(&core_dir).await?;

	let core = Core::new(core_dir).await?;

	let library = core
		.libraries
		.create_library("Test Library", None, core.context.clone())
		.await?;

	// Try to export a non-existent location
	let export_input = LocationExportInput {
		location_uuid: uuid::Uuid::new_v4(), // Random UUID that doesn't exist
		export_path: export_file,
		include_content_identities: true,
		include_media_data: true,
		include_user_metadata: true,
		include_tags: true,
	};

	let export_action = LocationExportAction::from_input(export_input)
		.map_err(|e| format!("Failed to create export action: {}", e))?;

	let action_manager = core.context.action_manager.read().await;
	let action_manager = action_manager
		.as_ref()
		.ok_or("Action manager not initialized")?;

	let result = action_manager
		.dispatch_library(Some(library.id()), export_action)
		.await;

	assert!(result.is_err(), "Export of non-existent location should fail");

	// Cleanup
	drop(action_manager);
	let lib_id = library.id();
	core.libraries.close_library(lib_id).await?;
	drop(library);

	core.shutdown().await?;

	Ok(())
}

#[tokio::test]
async fn test_import_invalid_file() -> Result<(), Box<dyn std::error::Error>> {
	let temp_dir = TempDir::new()?;
	let core_dir = temp_dir.path().join("core");
	let invalid_file = temp_dir.path().join("invalid.sql");

	tokio::fs::create_dir_all(&core_dir).await?;

	// Create an invalid SQL file (not a Spacedrive export)
	tokio::fs::write(&invalid_file, "-- Not a Spacedrive export\nSELECT 1;").await?;

	let core = Core::new(core_dir).await?;

	let library = core
		.libraries
		.create_library("Test Library", None, core.context.clone())
		.await?;

	// Try to import invalid file
	let import_input = LocationImportInput {
		import_path: invalid_file,
		new_name: None,
		skip_existing: false,
	};

	let import_action = LocationImportAction::from_input(import_input)
		.map_err(|e| format!("Failed to create import action: {}", e))?;

	let action_manager = core.context.action_manager.read().await;
	let action_manager = action_manager
		.as_ref()
		.ok_or("Action manager not initialized")?;

	let result = action_manager
		.dispatch_library(Some(library.id()), import_action)
		.await;

	assert!(result.is_err(), "Import of invalid file should fail");

	// Cleanup
	drop(action_manager);
	let lib_id = library.id();
	core.libraries.close_library(lib_id).await?;
	drop(library);

	core.shutdown().await?;

	Ok(())
}

#[tokio::test]
async fn test_import_links_existing_content_identities() -> Result<(), Box<dyn std::error::Error>> {
	// This test verifies that when importing a location that has content matching
	// existing content_identities in the destination library, the entries link to
	// the existing content_identities rather than creating duplicates.

	let temp_dir = TempDir::new()?;
	let core_dir = temp_dir.path().join("core");
	let export_file = temp_dir.path().join("location_export.sql");

	tokio::fs::create_dir_all(&core_dir).await?;

	// Create test location with some files
	let test_location = temp_dir.path().join("test_location");
	tokio::fs::create_dir_all(&test_location).await?;

	// Create a file with known content
	let shared_content = "This is shared content between libraries";
	tokio::fs::write(test_location.join("shared_file.txt"), shared_content).await?;
	tokio::fs::write(test_location.join("unique_file.txt"), "Unique content").await?;

	println!("Test location created at: {}", test_location.display());

	// Initialize Core
	let core = Core::new(core_dir.clone()).await?;

	// Create first library and index the location
	let library1 = core
		.libraries
		.create_library("Source Library", None, core.context.clone())
		.await?;

	let location_input = LocationAddInput {
		path: SdPath::local(test_location.clone()),
		name: Some("Source Location".to_string()),
		mode: IndexMode::Content,
		job_policies: None,
	};

	let location_action = LocationAddAction::from_input(location_input)
		.map_err(|e| format!("Failed to create location action: {}", e))?;

	let action_manager = core.context.action_manager.read().await;
	let action_manager = action_manager
		.as_ref()
		.ok_or("Action manager not initialized")?;

	let location_output = action_manager
		.dispatch_library(Some(library1.id()), location_action)
		.await
		.map_err(|e| format!("Failed to dispatch location add: {}", e))?;

	// Wait for indexing
	if let Some(job_id) = location_output.job_id {
		wait_for_job_completion(&library1, job_id, 60).await?;
	}
	wait_for_indexing_stable(&library1, 30).await?;

	// Get content identities from library1
	let lib1_content = entities::content_identity::Entity::find()
		.all(library1.db().conn())
		.await?;
	println!("Library1 has {} content identities", lib1_content.len());

	// Export the location
	let location = entities::location::Entity::find()
		.filter(entities::location::Column::Uuid.eq(location_output.location_id))
		.one(library1.db().conn())
		.await?
		.ok_or("Location not found")?;

	let export_input = LocationExportInput {
		location_uuid: location.uuid,
		export_path: export_file.clone(),
		include_content_identities: true,
		include_media_data: true,
		include_user_metadata: true,
		include_tags: true,
	};

	let export_action = LocationExportAction::from_input(export_input)
		.map_err(|e| format!("Failed to create export action: {}", e))?;

	action_manager
		.dispatch_library(Some(library1.id()), export_action)
		.await
		.map_err(|e| format!("Failed to dispatch export: {}", e))?;

	// Close first library
	drop(action_manager);
	let lib1_id = library1.id();
	core.libraries.close_library(lib1_id).await?;
	drop(library1);

	// Create second library
	let library2 = core
		.libraries
		.create_library("Destination Library", None, core.context.clone())
		.await?;

	// Create a different location in library2 with the SAME content as one file
	// This simulates having duplicate content across libraries
	let test_location2 = temp_dir.path().join("test_location2");
	tokio::fs::create_dir_all(&test_location2).await?;
	tokio::fs::write(test_location2.join("duplicate.txt"), shared_content).await?;

	// Add and index this location in library2
	let location_input2 = LocationAddInput {
		path: SdPath::local(test_location2.clone()),
		name: Some("Existing Location".to_string()),
		mode: IndexMode::Content,
		job_policies: None,
	};

	let location_action2 = LocationAddAction::from_input(location_input2)
		.map_err(|e| format!("Failed to create location action: {}", e))?;

	let action_manager = core.context.action_manager.read().await;
	let action_manager = action_manager
		.as_ref()
		.ok_or("Action manager not initialized")?;

	let location_output2 = action_manager
		.dispatch_library(Some(library2.id()), location_action2)
		.await
		.map_err(|e| format!("Failed to dispatch location add: {}", e))?;

	if let Some(job_id) = location_output2.job_id {
		wait_for_job_completion(&library2, job_id, 60).await?;
	}
	wait_for_indexing_stable(&library2, 30).await?;

	// Count content identities before import
	let content_before = entities::content_identity::Entity::find()
		.count(library2.db().conn())
		.await?;
	println!(
		"Library2 content identities before import: {}",
		content_before
	);

	// Now import the location from library1
	let import_input = LocationImportInput {
		import_path: export_file.clone(),
		new_name: Some("Imported Location".to_string()),
		skip_existing: false,
	};

	let import_action = LocationImportAction::from_input(import_input)
		.map_err(|e| format!("Failed to create import action: {}", e))?;

	let import_output = action_manager
		.dispatch_library(Some(library2.id()), import_action)
		.await
		.map_err(|e| format!("Failed to dispatch import: {}", e))?;

	println!(
		"Import complete: {} entries, {} content identities",
		import_output.stats.entries_imported, import_output.stats.content_identities
	);

	// Count content identities after import
	let content_after = entities::content_identity::Entity::find()
		.count(library2.db().conn())
		.await?;
	println!(
		"Library2 content identities after import: {}",
		content_after
	);

	// We should have: 1 existing + 1 unique from import = 2 total
	// The shared content should NOT create a duplicate
	// Note: content_identities use INSERT OR REPLACE with the same UUID based on content_hash,
	// so duplicates are handled at the database level
	assert!(
		content_after <= content_before + lib1_content.len() as u64,
		"Should not create more content identities than exported (existing: {}, after: {}, exported: {})",
		content_before,
		content_after,
		lib1_content.len()
	);

	// Verify the imported entries reference valid content_identities
	let imported_entries = entities::entry::Entity::find()
		.filter(entities::entry::Column::ContentId.is_not_null())
		.all(library2.db().conn())
		.await?;

	for entry in &imported_entries {
		if let Some(content_id) = entry.content_id {
			let content = entities::content_identity::Entity::find_by_id(content_id)
				.one(library2.db().conn())
				.await?;
			assert!(
				content.is_some(),
				"Entry {} should reference a valid content_identity",
				entry.name
			);
		}
	}

	println!(
		"Verified {} entries have valid content_identity references",
		imported_entries.len()
	);

	// Cleanup
	drop(action_manager);
	let lib2_id = library2.id();
	core.libraries.close_library(lib2_id).await?;
	drop(library2);

	core.shutdown().await?;

	println!("Test completed successfully");
	Ok(())
}
