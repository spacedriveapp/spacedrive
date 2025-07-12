//! Helper functions for common daemon operations

/// Helper functions for daemon operations
pub struct DaemonHelpers;

impl DaemonHelpers {

	/// Set up file logging for the daemon
	pub fn setup_file_logging(
		log_file: &std::path::Path,
	) -> Result<(), Box<dyn std::error::Error>> {
		use std::fs::OpenOptions;
		use tracing_subscriber::{fmt, prelude::*, EnvFilter};

		// Create log file directory if it doesn't exist
		if let Some(parent) = log_file.parent() {
			std::fs::create_dir_all(parent)?;
		}

		// Open log file for appending
		let file = OpenOptions::new()
			.create(true)
			.append(true)
			.open(log_file)?;

		// Set up file logging with both console and file output
		let file_layer = fmt::layer()
			.with_writer(file)
			.with_ansi(false) // No color codes in file
			.with_target(true)
			.with_thread_ids(true)
			.with_line_number(true);

		let console_layer = fmt::layer()
			.with_writer(std::io::stderr) // Console output to stderr
			.with_target(false); // Less verbose for console

		// Use info level for daemon by default, can be overridden with RUST_LOG
		let filter = EnvFilter::try_from_default_env()
			.unwrap_or_else(|_| EnvFilter::new("info,sd_core_new=debug"));

		// Set up comprehensive logging for the entire daemon process
		tracing_subscriber::registry()
			.with(filter)
			.with(file_layer)
			.with(console_layer)
			.init();

		tracing::info!(
			"Daemon logging initialized, writing to: {}",
			log_file.display()
		);
		tracing::info!("All Core application logs will be captured");
		Ok(())
	}
}