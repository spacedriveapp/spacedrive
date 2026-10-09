use std::path::PathBuf;
use std::sync::Arc;
use tracing::{info, warn};

use crate::infra::daemon::rpc::RpcServer;
use crate::Core;

/// Start a daemon server with a single Core instance. Without
/// `default_sources`, the launch pass a client starts leaves out the system
/// volume, the home folder and whole-drive maps.
pub async fn start_default_server(
	socket_addr: String,
	data_dir: PathBuf,
	enable_networking: bool,
	default_sources: bool,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
	// Initialize basic tracing with file logging first
	initialize_tracing_with_file_logging(&data_dir)?;

	// Before Core::new: library pools, watchers and the network endpoint open
	// descriptors under whatever limit the process has at that moment.
	#[cfg(unix)]
	raise_fd_limit();

	// Create a single Core instance
	let mut core = Core::new(data_dir.clone())
		.await
		.map_err(|e| format!("Failed to create core: {}", e))?;
	if !default_sources {
		core.context.startup_indexing.skip_default_sources();
	}

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

	let mut server = RpcServer::new(socket_addr, core.clone());

	// Start the server, which will initialize event streaming
	server.start().await
}

/// Raises the soft file descriptor limit to the hard limit.
///
/// Watchers, stores and peer connections each hold descriptors, and a shell or
/// launchd starts the daemon at 256 to 8192. The hard limit is the most a
/// process may grant itself without privileges, so that is the target. macOS
/// reports an unlimited hard limit but refuses a soft limit above
/// kern.maxfilesperproc, so an unlimited hard limit falls back to 65536 and
/// then to OPEN_MAX (10240), which macOS always accepts. The warning stays
/// when the limit is still low after the attempt, raised or not.
#[cfg(unix)]
fn raise_fd_limit() {
	const LOW_WATER: libc::rlim_t = 10000;

	let mut limit = libc::rlimit {
		rlim_cur: 0,
		rlim_max: 0,
	};
	if unsafe { libc::getrlimit(libc::RLIMIT_NOFILE, &mut limit) } != 0 {
		warn!(
			"Could not read the file descriptor limit: {}",
			std::io::Error::last_os_error()
		);
		return;
	}
	let before = limit.rlim_cur;
	let hard = if limit.rlim_max == libc::RLIM_INFINITY {
		"unlimited".to_string()
	} else {
		limit.rlim_max.to_string()
	};

	let mut targets = Vec::new();
	if limit.rlim_max != libc::RLIM_INFINITY {
		targets.push(limit.rlim_max);
	}
	targets.extend([65536, 10240]);

	for target in targets {
		if target <= before {
			break;
		}
		let raised = libc::rlimit {
			rlim_cur: target,
			rlim_max: limit.rlim_max,
		};
		if unsafe { libc::setrlimit(libc::RLIMIT_NOFILE, &raised) } == 0 {
			info!(
				"File descriptor limit raised from {} to {} (hard limit {})",
				before, target, hard
			);
			if target < LOW_WATER {
				warn!(
					"File descriptor limit is still low ({}); raise the hard limit or launchd NumberOfFiles",
					target
				);
			}
			return;
		}
	}

	if before < LOW_WATER {
		warn!(
			"File descriptor limit is low ({}) and could not be raised (hard limit {}); consider 'ulimit -n 65536' or a higher launchd NumberOfFiles",
			before, hard
		);
	} else {
		info!("File descriptor limit: {}", before);
	}
}

/// Initialize tracing with file logging to {data_dir}/logs/daemon.log
/// Supports multi-stream logging with per-stream filters
fn initialize_tracing_with_file_logging(
	data_dir: &PathBuf,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
	use crate::config::AppConfig;
	use crate::infra::event::log_emitter::LogEventLayer;
	use std::sync::Once;
	use tracing_appender::rolling::{RollingFileAppender, Rotation};
	use tracing_subscriber::{
		fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter, Layer,
	};

	static INIT: Once = Once::new();
	let mut result: Result<(), Box<dyn std::error::Error + Send + Sync>> = Ok(());

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

		// Set up layered subscriber with all streams plus the log event streaming layer.
		// If a tracing subscriber is already installed (e.g. when the daemon is embedded
		// inside sd-server which sets up its own basic subscriber first), fall back to
		// the existing one — losing the daemon's file logging is preferable to crashing.
		if let Err(e) = tracing_subscriber::registry()
			.with(layers)
			.with(LogEventLayer::new())
			.try_init()
		{
			eprintln!(
				"Note: daemon tracing setup skipped — a global subscriber is already \
				 installed by the host process. File logging to {}/daemon.log is disabled. \
				 Underlying error: {}",
				logs_dir.display(),
				e
			);
		}
	});

	result
}
