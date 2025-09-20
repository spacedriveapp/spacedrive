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
	// Initialize basic tracing with file logging first
	initialize_tracing_with_file_logging(&data_dir)?;

	let instances = Arc::new(CoreInstanceManager::new(
		data_dir.clone(),
		enable_networking,
	));

	info!("Starting Spacedrive daemon");
	info!("Data directory: {:?}", data_dir);
	info!("Socket path: {:?}", socket_path);
	info!("Networking enabled: {}", enable_networking);

	let mut server = RpcServer::new(socket_path, instances.clone());

	// Start the server, which will initialize the core and set up event streaming
	server.start().await
}

/// Initialize tracing with file logging to {data_dir}/logs/daemon.log
fn initialize_tracing_with_file_logging(data_dir: &PathBuf) -> Result<(), Box<dyn std::error::Error>> {
	use std::sync::Once;
    use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, fmt, EnvFilter};
    use crate::infra::event::log_emitter::LogEventLayer;
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

        // Set up layered subscriber with stdout, file output, and log event streaming layer
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
            .with(LogEventLayer::new())
			.try_init()
		{
			result = Err(format!("Failed to initialize tracing: {}", e).into());
		}
	});

	result
}

/// Set up log event streaming by initializing core and registering log event layer
async fn setup_log_event_streaming(instances: &Arc<CoreInstanceManager>) -> Result<(), Box<dyn std::error::Error>> {
	info!("Initializing core instance for log streaming...");

	// Initialize the default core instance to get access to the event bus
	let core = instances.get_default().await
		.map_err(|e| format!("Failed to initialize core for log streaming: {}", e))?;

	info!("Core instance initialized, setting up tracing log event layer...");

	// Now set up the log event layer with the core's event bus
	setup_tracing_log_event_layer(core.events.clone())?;

	info!("Starting test event emission...");

	// Emit some test events to verify the system is working
	emit_test_log_events(&core.events).await;

	Ok(())
}

/// Set up the tracing log event layer to forward logs to the event bus
fn setup_tracing_log_event_layer(event_bus: Arc<crate::infra::event::EventBus>) -> Result<(), Box<dyn std::error::Error>> {
	use crate::infra::event::log_emitter::LogEventLayer;
	use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, Registry};

	// We need to rebuild the subscriber with the log event layer
	// This is a bit tricky since tracing is already initialized
	// For now, let's just emit manual events and improve this later

	// TODO: Properly integrate LogEventLayer with existing tracing subscriber
	// This requires restructuring the tracing initialization to be done after core creation

	Ok(())
}

/// Emit test log events to demonstrate the log streaming functionality
async fn emit_test_log_events(event_bus: &Arc<crate::infra::event::EventBus>) {
	use crate::infra::event::Event;
	use chrono::Utc;

	info!("Emitting test log events to event bus");

	// Emit a series of test log events
	let events = vec![
		Event::LogMessage {
			timestamp: Utc::now(),
			level: "INFO".to_string(),
			target: "sd_core::daemon".to_string(),
			message: "Spacedrive daemon started successfully".to_string(),
			job_id: None,
			library_id: None,
		},
		Event::LogMessage {
			timestamp: Utc::now(),
			level: "INFO".to_string(),
			target: "sd_core::event".to_string(),
			message: "Log event streaming initialized".to_string(),
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

	for (i, event) in events.into_iter().enumerate() {
		info!("Emitting test event {}: {:?}", i + 1, event);
		event_bus.emit(event);
		// Small delay between events
		tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
	}

	// Emit periodic heartbeat events every 10 seconds for testing
	tokio::spawn({
		let event_bus = event_bus.clone();
		async move {
			let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(10));
			loop {
				interval.tick().await;
				let heartbeat_event = Event::LogMessage {
					timestamp: Utc::now(),
					level: "DEBUG".to_string(),
					target: "sd_core::daemon".to_string(),
					message: "Daemon heartbeat".to_string(),
					job_id: None,
					library_id: None,
				};
				info!("Emitting heartbeat event: {:?}", heartbeat_event);
				event_bus.emit(heartbeat_event);
			}
		}
	});

	info!("Test log events setup complete");
}
