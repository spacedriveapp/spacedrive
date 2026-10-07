//! Library demo using full core lifecycle

use sd_core::{
	infra::{action::LibraryAction, db::entities},
	ops::sources::track::{TrackSourceAction, TrackSourceInput},
	Core,
};
use sea_orm::{EntityTrait, PaginatorTrait};
use std::path::PathBuf;

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

		// 6. Track the current directory as a source
		println!("\n6. Tracking the current directory as a source...");
		let tracked = TrackSourceAction::from_input(TrackSourceInput {
			path: std::env::current_dir()?,
			name: Some("Current Directory".to_string()),
			overrides: Default::default(),
		})?
		.execute(library.clone(), core.context.clone())
		.await?;
		println!("   ✓ Source tracked: {}", tracked.id);
	} else {
		// Show existing libraries
		println!("\n5. Existing libraries:");
		for library in &libraries {
			println!("   - {} ({})", library.name().await, library.id());

			// Show some stats
			let source_count = entities::source::Entity::find()
				.count(library.db().conn())
				.await?;
			println!("     Sources: {}", source_count);
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
