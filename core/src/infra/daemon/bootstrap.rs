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

/// Initialize tracing with optional file logging to {data_dir}/logs/
/// File logging is controlled by config.daemon_logging.enabled
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
		// Load config to get logging settings
		let config = match AppConfig::load_from(data_dir) {
			Ok(c) => c,
			Err(e) => {
				eprintln!("Failed to load config for logging: {}, using defaults", e);
				AppConfig::default_with_dir(data_dir.clone())
			}
		};

		// Resolve logs directory path (absolute or relative to data_dir)
		let logs_dir = config.logs_dir();

		// Set up main environment filter (for stdout and main daemon.log)
		let main_filter =
			std::env::var("RUST_LOG").unwrap_or_else(|_| config.logging.main_filter.clone());

		// Start building the subscriber layers
		let mut layers = Vec::new();

		// Stdout layer - always enabled
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

		// Daemon file logging - conditional based on config
		if config.daemon_logging.enabled {
			// Ensure logs directory exists
			if let Err(e) = std::fs::create_dir_all(&logs_dir) {
				eprintln!("Failed to create logs directory {:?}: {}", logs_dir, e);
				// Continue without file logging rather than failing completely
			} else {
				// Create file appender based on rotation and naming settings
				let file_appender = if config.daemon_logging.enable_rotation {
					if config.daemon_logging.standard_naming {
						// Standard naming: daemon-YYYY-MM-DD.log
						// Use tracing_appender with custom prefix to get date before extension
						RollingFileAppender::new(Rotation::DAILY, &logs_dir, "daemon")
					} else {
						// Legacy naming: daemon.log.YYYY-MM-DD (breaks tooling)
						RollingFileAppender::new(Rotation::DAILY, &logs_dir, "daemon.log")
					}
				} else {
					// No rotation: single daemon.log file
					RollingFileAppender::new(Rotation::NEVER, &logs_dir, "daemon.log")
				};

				// Main daemon file layer with main filter
				layers.push(
					fmt::layer()
						.with_target(true)
						.with_thread_ids(true)
						.with_ansi(false)
						.with_writer(file_appender)
						.with_filter(EnvFilter::new(&main_filter))
						.boxed(),
				);

				// Add custom log streams only if daemon file logging is enabled
				for stream in config.logging.streams.iter().filter(|s| s.enabled) {
					// Note: Can't use info! here as tracing isn't initialized yet
					eprintln!(
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
							eprintln!("Log stream '{}' configured successfully", stream.name);
						}
						Err(e) => {
							eprintln!(
								"Failed to parse filter for log stream '{}': {}. Skipping stream.",
								stream.name, e
							);
						}
					}
				}
			}
		}

		// LogEventLayer - always enabled for client streaming regardless of file logging
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
