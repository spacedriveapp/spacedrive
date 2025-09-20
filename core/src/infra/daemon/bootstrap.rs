use std::path::PathBuf;
use std::sync::Arc;
use tracing::info;

use crate::infra::daemon::rpc::RpcServer;
use crate::Core;

/// Start a daemon server with a single Core instance
pub async fn start_default_server(
	socket_path: PathBuf,
	data_dir: PathBuf,
	enable_networking: bool,
) -> Result<(), Box<dyn std::error::Error>> {
	// Initialize basic tracing with file logging first
	initialize_tracing_with_file_logging(&data_dir)?;

	// Create a single Core instance
	let core = Arc::new(
		Core::new_with_config(data_dir.clone())
			.await
			.map_err(|e| format!("Failed to create core: {}", e))?,
	);

	let core = if enable_networking {
		Core::init_networking_shared(core)
			.await
			.map_err(|e| format!("Failed to initialize networking: {}", e))?
	} else {
		core
	};

	info!("Starting Spacedrive daemon");
	info!("Data directory: {:?}", data_dir);
	info!("Socket path: {:?}", socket_path);
	info!("Networking enabled: {}", enable_networking);

	let mut server = RpcServer::new(socket_path, core.clone());

	// Start the server, which will initialize event streaming
	server.start().await
}

/// Initialize tracing with file logging to {data_dir}/logs/daemon.log
fn initialize_tracing_with_file_logging(
	data_dir: &PathBuf,
) -> Result<(), Box<dyn std::error::Error>> {
	use crate::infra::event::log_emitter::LogEventLayer;
	use std::sync::Once;
	use tracing_appender::rolling::{RollingFileAppender, Rotation};
	use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

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
		let file_appender = RollingFileAppender::new(Rotation::DAILY, logs_dir, "daemon.log");

		// Set up layered subscriber with stdout, file output, and log event streaming layer
		if let Err(e) = tracing_subscriber::registry()
			.with(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(env_filter)))
			.with(
				fmt::layer()
					.with_target(true)
					.with_thread_ids(true)
					.with_writer(std::io::stdout),
			)
			.with(
				fmt::layer()
					.with_target(true)
					.with_thread_ids(true)
					.with_ansi(false) // No ANSI colors in log files
					.with_writer(file_appender),
			)
			.with(LogEventLayer::new())
			.try_init()
		{
			result = Err(format!("Failed to initialize tracing: {}", e).into());
		}
	});

	result
}
