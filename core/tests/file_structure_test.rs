//! Test to map exact File structure at each indexing phase

use sd_core::{
	infra::{
		db::entities,
		event::{Event, EventSubscriber},
	},
	location::{create_location, IndexMode, LocationCreateArgs},
	Core,
};
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter};
use std::{sync::Arc, time::Duration};
use tempfile::TempDir;

#[tokio::test]
async fn map_file_structure_per_phase() -> Result<(), Box<dyn std::error::Error>> {
	tracing_subscriber::fmt::init();
	eprintln!("\nMAPPING FILE STRUCTURE AT EACH PHASE\n");
	eprintln!("{}", "=".repeat(80));

	// Setup
	let temp_dir = TempDir::new()?;
	let core = Core::new(temp_dir.path().to_path_buf()).await?;

	let library = core
		.libraries
		.create_library("Phase Test", None, core.context.clone())
		.await?;

	// Create test files
	let test_dir = temp_dir.path().join("test");
	tokio::fs::create_dir_all(&test_dir).await?;

	for i in 1..=3 {
		tokio::fs::write(
			test_dir.join(format!("file{}.txt", i)),
			format!("Content {}", i),
		)
		.await?;
	}

	eprintln!("Created test directory with 3 files\n");

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

	// Collect events
	let events_collected = Arc::new(tokio::sync::Mutex::new(Vec::new()));
	let events_clone = events_collected.clone();
	let mut subscriber = core.events.subscribe();

	tokio::spawn(async move {
		while let Ok(event) = subscriber.recv().await {
			events_clone.lock().await.push(event);
		}
	});

	tokio::time::sleep(Duration::from_millis(100)).await;

	eprintln!("Starting indexing...\n");

	let location_args = LocationCreateArgs {
		path: test_dir.clone(),
		name: Some("Test".to_string()),
		index_mode: IndexMode::Content,
	};

	create_location(
		library.clone(),
		&core.events,
		location_args,
		device_record.id,
	)
	.await?;

	// Wait for completion
	tokio::time::sleep(Duration::from_secs(5)).await;

	// Analyze
	let events = events_collected.lock().await;
	let mut batch_num = 0;

	eprintln!("RESOURCE EVENT FILE STRUCTURES:\n");

	for event in events.iter() {
		if let Event::ResourceChangedBatch {
			resource_type,
			resources,
			metadata,
		} = event
		{
			if resource_type == "file" {
				batch_num += 1;

				if let Some(files) = resources.as_array() {
					eprintln!("\n{}", "=".repeat(80));
					eprintln!("BATCH #{} ({} files)", batch_num, files.len());
					eprintln!("{}", "=".repeat(80));

					if let Some(file) = files.first() {
						eprintln!("\nSample File JSON:");
						eprintln!("{}\n", serde_json::to_string_pretty(&file).unwrap());

						eprintln!("Key Fields:");
						eprintln!(
							"   id:                  {}",
							file.get("id").unwrap_or(&serde_json::Value::Null)
						);
						eprintln!(
							"   name:                {}",
							file.get("name").unwrap_or(&serde_json::Value::Null)
						);

						if let Some(sd_path) = file.get("sd_path") {
							eprintln!("\n   sd_path:");
							if let Some(phys) = sd_path.get("Physical") {
								eprintln!("     Type: Physical");
								eprintln!(
									"     device_slug: {}",
									phys.get("device_slug").unwrap_or(&serde_json::Value::Null)
								);
								eprintln!(
									"     path: {}",
									phys.get("path").unwrap_or(&serde_json::Value::Null)
								);
							} else if let Some(content) = sd_path.get("Content") {
								eprintln!("     Type: Content");
								eprintln!(
									"     content_id: {}",
									content
										.get("content_id")
										.unwrap_or(&serde_json::Value::Null)
								);
							} else if let Some(cloud) = sd_path.get("Cloud") {
								eprintln!("     Type: Cloud");
								eprintln!(
									"     service: {}",
									cloud.get("service").unwrap_or(&serde_json::Value::Null)
								);
								eprintln!(
									"     path: {}",
									cloud.get("path").unwrap_or(&serde_json::Value::Null)
								);
							}
						}

						eprintln!(
							"\n   content_identity:    {}",
							if file
								.get("content_identity")
								.and_then(|v| v.as_object())
								.is_some()
							{
								"PRESENT"
							} else {
								"NULL"
							}
						);

						if let Some(ci) = file.get("content_identity") {
							if let Some(ci_obj) = ci.as_object() {
								eprintln!(
									"     uuid: {}",
									ci_obj.get("uuid").unwrap_or(&serde_json::Value::Null)
								);
								eprintln!(
									"     content_hash: {}",
									ci_obj
										.get("content_hash")
										.unwrap_or(&serde_json::Value::Null)
								);
							}
						}

						eprintln!(
							"\n   sidecars:            {} items",
							file.get("sidecars")
								.and_then(|s| s.as_array())
								.map(|a| a.len())
								.unwrap_or(0)
						);
					}
				}
			}
		}
	}

	eprintln!("\n{}", "=".repeat(80));
	eprintln!("EVENT SUMMARY: {} file batches emitted", batch_num);
	eprintln!("{}", "=".repeat(80));

	//  Now manually check what entries look like in database
	eprintln!("\n\nCHECKING DATABASE ENTRIES:\n");
	eprintln!("{}", "=".repeat(80));

	use sd_core::infra::db::entities::entry;
	use sea_orm::EntityTrait;

	let db_entries = entry::Entity::find().all(db.conn()).await?;

	eprintln!("Database has {} entries\n", db_entries.len());

	for entry in db_entries.iter().take(3) {
		if entry.kind == 0 {
			// File
			eprintln!("Entry ID: {}", entry.id);
			eprintln!("   UUID: {:?}", entry.uuid);
			eprintln!("   Name: {}", entry.name);
			eprintln!("   content_id (db FK): {:?}", entry.content_id);
			eprintln!();
		}
	}

	eprintln!("{}", "=".repeat(80));
	eprintln!("KEY INSIGHT:");
	eprintln!("{}", "=".repeat(80));
	eprintln!("\nEvent files use:");
	eprintln!("  - id = entry.uuid");
	eprintln!("  - sd_path = Content {{content_id}}");
	eprintln!("\nDirectory query SHOULD use:");
	eprintln!("  - id = entry.uuid (SAME)");
	eprintln!("  - sd_path = Physical {{path}} (DIFFERENT)");
	eprintln!("\n️  If IDs match, normalized cache should work!");
	eprintln!("️  If IDs don't match, we have a bigger problem.");
	eprintln!();

	Ok(())
}
