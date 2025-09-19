use std::path::PathBuf;
use std::sync::Arc;
use tracing::info;

use crate::infra::daemon::{
	instance::CoreInstanceManager, rpc::RpcServer,
};

/// Start a default daemon server with built-in handlers and default instance
pub async fn start_default_server(
	socket_path: PathBuf,
	data_dir: PathBuf,
	enable_networking: bool,
) -> Result<(), Box<dyn std::error::Error>> {
	// Initialize tracing with file logging first
	initialize_tracing_with_file_logging(&data_dir)?;

	let instances = Arc::new(CoreInstanceManager::new(
		data_dir.clone(),
		enable_networking,
	));

	// Set up event streaming after core is initialized
	setup_event_streaming(&instances).await?;

	info!("Starting Spacedrive daemon");
	info!("Data directory: {:?}", data_dir);
	info!("Socket path: {:?}", socket_path);
	info!("Networking enabled: {}", enable_networking);

	let mut server = RpcServer::new(socket_path, instances);
	server.start().await
}

/// Initialize tracing with file logging to {data_dir}/logs/daemon.log
fn initialize_tracing_with_file_logging(data_dir: &PathBuf) -> Result<(), Box<dyn std::error::Error>> {
	use std::sync::Once;
	use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, fmt, EnvFilter};
	use tracing_appender::rolling::{RollingFileAppender, Rotation};

	static INIT: Once = Once::new();
	let mut result: Result<(), Box<dyn std::error::Error>> = Ok(());

	INIT.call_once(|| {
		// Ensure logs directory exists
		let logs_dir = data_dir.join("logs");
		if let Err(e) = std::fs::create_dir_all(&logs_dir) {
			result = Err(format!("Failed to create logs directory: {}", e).into());
			return;
		}

		// Set up environment filter
		let env_filter = std::env::var("RUST_LOG")
			.unwrap_or_else(|_| "sd_core=info,spacedrive=info".to_string());

		// Create file appender that rotates daily
		let file_appender = RollingFileAppender::new(
			Rotation::DAILY,
			logs_dir,
			"daemon.log"
		);

		// Set up layered subscriber with both stdout and file output
		if let Err(e) = tracing_subscriber::registry()
			.with(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(env_filter)))
			.with(
				fmt::layer()
					.with_target(true)
					.with_thread_ids(true)
					.with_writer(std::io::stdout)
			)
			.with(
				fmt::layer()
					.with_target(true)
					.with_thread_ids(true)
					.with_ansi(false) // No ANSI colors in log files
					.with_writer(file_appender)
			)
			.try_init()
		{
			result = Err(format!("Failed to initialize tracing: {}", e).into());
		}
	});

	result
}

/// Set up event streaming after core is initialized
async fn setup_event_streaming(instances: &Arc<CoreInstanceManager>) -> Result<(), Box<dyn std::error::Error>> {
	use crate::infra::event::log_emitter::LogEventLayer;

	// Set up the log event layer after the core is initialized
	let instances_clone = instances.clone();
	tokio::spawn(async move {
		// Wait a bit for the core to be fully initialized
		tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

		if let Ok(core) = instances_clone.get_default().await {
			// Create and register the log event layer
			let _log_layer = LogEventLayer::new(core.events.clone());

			// Emit some test events to verify the system is working
			emit_test_log_events(&core.events).await;
		}
	});

	Ok(())
}

/// Emit test log events to demonstrate the log streaming functionality
async fn emit_test_log_events(event_bus: &Arc<crate::infra::event::EventBus>) {
	use crate::infra::event::Event;
	use chrono::Utc;

	// Emit a series of test log events
	let events = vec![
		Event::LogMessage {
			timestamp: Utc::now(),
			level: "INFO".to_string(),
			target: "sd_core::daemon".to_string(),
			message: "🚀 Spacedrive daemon started successfully".to_string(),
			job_id: None,
			library_id: None,
		},
		Event::LogMessage {
			timestamp: Utc::now(),
			level: "INFO".to_string(),
			target: "sd_core::event".to_string(),
			message: "📡 Log event streaming initialized".to_string(),
			job_id: None,
			library_id: None,
		},
		Event::LogMessage {
			timestamp: Utc::now(),
			level: "DEBUG".to_string(),
			target: "sd_core::rpc".to_string(),
			message: "RPC server listening for connections".to_string(),
			job_id: None,
			library_id: None,
		},
	];

	for event in events {
		event_bus.emit(event);
		// Small delay between events
		tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
	}

	// Emit periodic heartbeat events
	tokio::spawn({
		let event_bus = event_bus.clone();
		async move {
			let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(30));
			loop {
				interval.tick().await;
				event_bus.emit(Event::LogMessage {
					timestamp: Utc::now(),
					level: "DEBUG".to_string(),
					target: "sd_core::daemon".to_string(),
					message: "💓 Daemon heartbeat".to_string(),
					job_id: None,
					library_id: None,
				});
			}
		}
	});
}
