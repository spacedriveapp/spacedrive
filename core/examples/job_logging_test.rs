//! Simple test for job logging functionality

use sd_core::{
	config::{AppConfig, JobLoggingConfig},
	infra::{db::entities, event::Event},
	location::{create_location, IndexMode, LocationCreateArgs},
	Core,
};
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter};
use std::path::PathBuf;
use tokio::time::{sleep, Duration};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
	// Initialize logging
	tracing_subscriber::fmt()
		.with_env_filter("sd_core=debug")
		.init();

	println!("Job Logging Test\n");

	// 1. Initialize Core with job logging
	println!("1. Setting up with job logging enabled...");
	let data_dir = PathBuf::from("./data/job-logging-test");

	// Configure with job logging
	{
		let mut config = AppConfig::load_from(&data_dir)
			.unwrap_or_else(|_| AppConfig::default_with_dir(data_dir.clone()));

		config.job_logging = JobLoggingConfig {
			enabled: true,
			log_directory: "job_logs".to_string(),
			max_file_size: 10 * 1024 * 1024,
			include_debug: true,
			log_ephemeral_jobs: false,
		};

		config.save()?;
		println!("   Job logging enabled");
	}

	let core = Core::new(data_dir.clone()).await?;
	let job_logs_dir = data_dir.join("job_logs");
	println!("   Job logs directory: {:?}", job_logs_dir);

	// 2. Create library
	println!("\n2. Creating library...");
	let library = if core.libraries.list().await.is_empty() {
		core.libraries
			.create_library("Test Library", None, core.context.clone())
			.await?
	} else {
		core.libraries.list().await.into_iter().next().unwrap()
	};
	println!("   Library ready");

	// 3. Create a small test location
	println!("\n3. Creating test location...");
	let test_path = PathBuf::from("./test-data");

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

	// Create location
	let location_args = LocationCreateArgs {
		path: test_path.clone(),
		name: Some("Test Data".to_string()),
		index_mode: IndexMode::Deep,
	};

	let _location_db_id = create_location(
		library.clone(),
		&core.events,
		location_args,
		device_record.id,
	)
	.await?;

	println!("   Location created, job dispatched");

	// 4. Monitor for a short time
	println!("\n4. Monitoring job progress...");
	let mut event_rx = core.events.subscribe();
	let start = std::time::Instant::now();
	let timeout = Duration::from_secs(10);

	while start.elapsed() < timeout {
		tokio::select! {
			Ok(event) = event_rx.recv() => {
				match event {
					Event::JobProgress { job_id, message, .. } => {
						if let Some(msg) = message {
							println!("   Job {}: {}", job_id, msg);
						}
					}
					Event::IndexingCompleted { .. } => {
						println!("   Indexing completed!");
						break;
					}
					_ => {}
				}
			}
			_ = sleep(Duration::from_millis(100)) => {}
		}
	}

	// 5. Check job logs
	println!("\n5. Checking job logs...");
	if let Ok(mut entries) = tokio::fs::read_dir(&job_logs_dir).await {
		let mut count = 0;
		while let Ok(Some(entry)) = entries.next_entry().await {
			if let Some(name) = entry.file_name().to_str() {
				if name.ends_with(".log") {
					count += 1;
					let log_path = job_logs_dir.join(name);
					if let Ok(contents) = tokio::fs::read_to_string(&log_path).await {
						println!("\n   Log file: {}", name);
						println!("   Size: {} bytes", contents.len());
						println!("   Lines: {}", contents.lines().count());

						// Show first few lines
						println!("\n   First 10 lines:");
						for (i, line) in contents.lines().take(10).enumerate() {
							println!("   {}: {}", i + 1, line);
						}

						if contents.lines().count() > 10 {
							println!("   ... {} more lines", contents.lines().count() - 10);
						}
					}
				}
			}
		}

		if count == 0 {
			println!("    No job logs found");
		} else {
			println!("\n   Found {} job log file(s)", count);
		}
	}

	// 6. Shutdown
	println!("\n6. Shutting down...");
	core.shutdown().await?;

	println!("\nTest complete!");
	println!("Data: {:?}", data_dir);
	println!("Logs: {:?}", job_logs_dir);

	Ok(())
}
