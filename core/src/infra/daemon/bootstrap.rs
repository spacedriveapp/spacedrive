use std::path::PathBuf;
use std::sync::Arc;
use tracing::{info, warn};

use crate::infra::daemon::rpc::RpcServer;
use crate::Core;

/// Start a daemon server with a single Core instance
pub async fn start_default_server(
	socket_addr: String,
	data_dir: PathBuf,
	enable_networking: bool,
) -> Result<(), Box<dyn std::error::Error>> {
	// Initialize basic tracing with file logging first
	initialize_tracing_with_file_logging(&data_dir)?;

	// Create a single Core instance
	let mut core = Core::new(data_dir.clone())
		.await
		.map_err(|e| format!("Failed to create core: {}", e))?;

	// Initialize networking if enabled
	if enable_networking {
		core.init_networking()
			.await
			.map_err(|e| format!("Failed to initialize networking: {}", e))?;
	}

	let core = Arc::new(core);

	info!("Starting Spacedrive daemon");
	info!("Data directory: {:?}", data_dir);
	info!("Socket address: {}", socket_addr);
	info!("Networking enabled: {}", enable_networking);

	// Log file descriptor limits for debugging
	#[cfg(unix)]
	{
		use std::process::Command;
		if let Ok(output) = Command::new("sh").arg("-c").arg("ulimit -n").output() {
			if let Ok(limit_str) = String::from_utf8(output.stdout) {
				if let Ok(limit) = limit_str.trim().parse::<u64>() {
					info!("System file descriptor limit: {}", limit);
					if limit < 10000 {
						warn!("File descriptor limit is low ({}), consider increasing with 'ulimit -n 65536'", limit);
					}
				}
			}
		}
	}

	let mut server = RpcServer::new(socket_addr, core.clone());

	// Start the server, which will initialize event streaming
	server.start().await
}

/// Initialize tracing with file logging to {data_dir}/logs/daemon.log
/// Supports multi-stream logging with per-stream filters
fn initialize_tracing_with_file_logging(
	data_dir: &PathBuf,
) -> Result<(), Box<dyn std::error::Error>> {
	use crate::config::AppConfig;
	use crate::infra::event::log_emitter::LogEventLayer;
	use std::sync::Once;
	use tracing_appender::rolling::{RollingFileAppender, Rotation};
	use tracing_subscriber::{
		fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter, Layer,
	};

	static INIT: Once = Once::new();
	let mut result: Result<(), Box<dyn std::error::Error>> = Ok(());

	INIT.call_once(|| {
		// Ensure logs directory exists
		let logs_dir = data_dir.join("logs");
		if let Err(e) = std::fs::create_dir_all(&logs_dir) {
			result = Err(format!("Failed to create logs directory: {}", e).into());
			return;
		}

		// Load config to get logging streams
		let config = match AppConfig::load_from(data_dir) {
			Ok(c) => c,
			Err(e) => {
				warn!(
					"Failed to load config for logging streams: {}, using defaults",
					e
				);
				AppConfig::default_with_dir(data_dir.clone())
			}
		};

		// Set up main environment filter (for stdout and main daemon.log)
		let main_filter =
			std::env::var("RUST_LOG").unwrap_or_else(|_| config.logging.main_filter.clone());

		// Create main daemon.log file appender
		let main_file_appender = RollingFileAppender::new(Rotation::DAILY, &logs_dir, "daemon.log");

		// Start building the subscriber with stdout and main file layers
		let mut layers = Vec::new();

		// Stdout layer with main filter
		layers.push(
			fmt::layer()
				.with_target(true)
				.with_thread_ids(true)
				.with_writer(std::io::stdout)
				.with_filter(
					EnvFilter::try_from_default_env()
						.unwrap_or_else(|_| EnvFilter::new(&main_filter)),
				)
				.boxed(),
		);

		// Main daemon.log file layer with main filter
		layers.push(
			fmt::layer()
				.with_target(true)
				.with_thread_ids(true)
				.with_ansi(false)
				.with_writer(main_file_appender)
				.with_filter(EnvFilter::new(&main_filter))
				.boxed(),
		);

		// Add custom log streams
		for stream in config.logging.streams.iter().filter(|s| s.enabled) {
			info!(
				"Configuring log stream: {} -> {} (filter: {})",
				stream.name, stream.file_name, stream.filter
			);

			let stream_appender =
				RollingFileAppender::new(Rotation::DAILY, &logs_dir, &stream.file_name);

			match EnvFilter::try_new(&stream.filter) {
				Ok(filter) => {
					layers.push(
						fmt::layer()
							.with_target(true)
							.with_thread_ids(true)
							.with_ansi(false)
							.with_writer(stream_appender)
							.with_filter(filter)
							.boxed(),
					);
					info!("Log stream '{}' configured successfully", stream.name);
				}
				Err(e) => {
					warn!(
						"Failed to parse filter for log stream '{}': {}. Skipping stream.",
						stream.name, e
					);
				}
			}
		}

		// Set up layered subscriber with all streams plus the log event streaming layer
		if let Err(e) = tracing_subscriber::registry()
			.with(layers)
			.with(LogEventLayer::new())
			.try_init()
		{
			result = Err(format!("Failed to initialize tracing: {}", e).into());
		}
	});

	result
}
