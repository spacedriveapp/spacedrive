//! Library demo using full core lifecycle

use sd_core::{infra::db::entities, Core};
use sea_orm::{ActiveModelTrait, ActiveValue::NotSet, EntityTrait, PaginatorTrait, Set};
use std::path::PathBuf;
use uuid::Uuid;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
	// Initialize logging
	tracing_subscriber::fmt()
		.with_env_filter("sd_core=debug")
		.init();

	println!("=== Spacedrive Core Lifecycle Demo ===\n");

	// 1. Initialize core with custom data directory
	println!("1. Initializing Spacedrive Core...");
	let data_dir = PathBuf::from("./data/spacedrive-demo-data");
	let core = Core::new(data_dir.clone()).await?;
	println!("   ✓ Core initialized with data directory: {:?}", data_dir);
	println!("   ✓ Device UUID: {}", core.device.device_id()?);

	// 2. Check application config
	{
		let config = core.config();
		let app_config = config.read().await;
		println!("\n2. Application Configuration:");
		println!("   - Data directory: {:?}", app_config.data_dir);
		println!("   - Log level: {}", app_config.log_level);
		println!(
			"   - Networking enabled: {}",
			app_config.services.networking_enabled
		);
		println!("   - Theme: {}", app_config.preferences.theme);
	}

	// 3. Subscribe to events
	println!("\n3. Setting up event listener...");
	let mut events = core.events.subscribe();
	tokio::spawn(async move {
		while let Ok(event) = events.recv().await {
			println!("   [EVENT] {:?}", event);
		}
	});

	// 4. Check for existing libraries
	println!("\n4. Checking for existing libraries...");
	let libraries = core.libraries.list().await;
	println!("   Found {} open libraries", libraries.len());

	if libraries.is_empty() {
		// 5. Create a new library
		println!("\n5. Creating new library...");
		let library = core
			.libraries
			.create_library("Lifecycle Demo Library", None, core.context.clone())
			.await?;
		println!("   ✓ Library created: {}", library.name().await);
		println!("   ✓ ID: {}", library.id());
		println!("   ✓ Path: {}", library.path().display());

		// 6. Add some test data
		println!("\n6. Adding test data...");
		let db = library.db();
		let device = core.device.to_device()?;

		// Register device
		let device_model = entities::device::ActiveModel {
			id: NotSet,
			uuid: Set(device.id),
			name: Set(device.name.clone()),
			slug: Set(device.name.clone()),
			os: Set(device.os.to_string()),
			os_version: Set(None),
			hardware_model: Set(device.hardware_model),
			cpu_model: Set(None),
			cpu_architecture: Set(None),
			cpu_cores_physical: Set(None),
			cpu_cores_logical: Set(None),
			cpu_frequency_mhz: Set(None),
			memory_total_bytes: Set(None),
			form_factor: Set(None),
			manufacturer: Set(None),
			gpu_models: Set(None),
			boot_disk_type: Set(None),
			boot_disk_capacity_bytes: Set(None),
			swap_total_bytes: Set(None),
			network_addresses: Set(serde_json::json!([])),
			is_online: Set(true),
			last_seen_at: Set(chrono::Utc::now()),
			capabilities: Set(serde_json::json!({
				"indexing": true,
				"p2p": true,
				"cloud": false
			})),
			created_at: Set(device.created_at),
			updated_at: Set(device.updated_at),
			sync_enabled: Set(false),
		};
		let inserted_device = device_model.insert(db.conn()).await?;
		println!("   ✓ Device registered");

		// Create entry for location root
		let current_path = std::env::current_dir()?;
		let entry = entities::entry::ActiveModel {
			id: NotSet,
			uuid: Set(Some(Uuid::new_v4())),
			parent_id: Set(None), // Location root has no parent
			name: Set(current_path
				.file_name()
				.and_then(|n| n.to_str())
				.unwrap_or("Current Directory")
				.to_string()),
			kind: Set(1), // 1 = Directory
			extension: Set(None),
			metadata_id: Set(None),
			content_id: Set(None),
			size: Set(0),
			aggregate_size: Set(0),
			child_count: Set(0),
			file_count: Set(0),
			created_at: Set(chrono::Utc::now()),
			modified_at: Set(chrono::Utc::now()),
			accessed_at: Set(None),
			indexed_at: Set(None),
			permissions: Set(None),
			volume_id: Set(None),
			inode: Set(None),
		};
		let entry_record = entry.insert(db.conn()).await?;

		// Add location
		let location = entities::location::ActiveModel {
			id: NotSet,
			uuid: Set(Uuid::new_v4()),
			device_id: Set(inserted_device.id),
			volume_id: Set(None),
			entry_id: Set(Some(entry_record.id)),
			name: Set(Some("Current Directory".to_string())),
			index_mode: Set("shallow".to_string()),
			scan_state: Set("pending".to_string()),
			last_scan_at: Set(None),
			error_message: Set(None),
			total_file_count: Set(0),
			total_byte_size: Set(0),
			job_policies: Set(None),
			created_at: Set(chrono::Utc::now()),
			updated_at: Set(chrono::Utc::now()),
		};
		location.insert(db.conn()).await?;
		println!("   ✓ Location added");
	} else {
		// Show existing libraries
		println!("\n5. Existing libraries:");
		for library in &libraries {
			println!("   - {} ({})", library.name().await, library.id());

			// Show some stats
			let db = library.db();
			let entry_count = entities::entry::Entity::find().count(db.conn()).await?;
			let location_count = entities::location::Entity::find().count(db.conn()).await?;
			println!(
				"     Entries: {}, Locations: {}",
				entry_count, location_count
			);
		}
	}

	// 7. Demonstrate graceful shutdown
	println!("\n7. Press Ctrl+C to trigger graceful shutdown...");
	tokio::signal::ctrl_c().await?;

	println!("\n8. Shutting down...");
	core.shutdown().await?;
	println!("   ✓ Core shutdown complete");

	println!("\nLifecycle demo completed!");
	println!("\nData stored at: {:?}", data_dir);
	println!("   Run again to see library auto-loading in action!");

	Ok(())
}
