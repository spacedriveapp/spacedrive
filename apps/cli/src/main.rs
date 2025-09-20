use anyhow::Result;
use clap::{Parser, Subcommand};
use sd_core::client::CoreClient;
use std::path::Path;

mod context;
mod domains;
mod ui;
mod util;

use crate::context::{Context, OutputFormat};
use crate::domains::{
	file::{self, FileCmd},
	index::{self, IndexCmd},
	job::{self, JobCmd},
	library::{self, LibraryCmd},
	location::{self, LocationCmd},
	logs::{self, LogsCmd},
	network::{self, NetworkCmd},
	search::{self, SearchCmd},
	tag::{self, TagCmd},
};

// OutputFormat is defined in context.rs and shared across domains

/// Safely reset only Spacedrive v2 specific files and directories
/// This preserves any user data that might be in the data directory (like v1 backups)
fn reset_spacedrive_v2_data(data_dir: &Path) -> Result<()> {
	let mut removed_items = Vec::new();
	let mut errors = Vec::new();

	// List of specific Spacedrive v2 files and directories to remove
	let v2_items = [
		"spacedrive.json", // Main app config
		"device.json",     // Device config
		"libraries",       // All v2 libraries
		"logs",            // Application logs
		"job_logs",        // Job logs
	];

	for item in &v2_items {
		let path = data_dir.join(item);
		if path.exists() {
			let result = if path.is_dir() {
				std::fs::remove_dir_all(&path)
			} else {
				std::fs::remove_file(&path)
			};

			match result {
				Ok(()) => {
					removed_items.push(item.to_string());
					println!("   Removed: {}", item);
				}
				Err(e) => {
					errors.push(format!("Failed to remove {}: {}", item, e));
					println!("    Failed to remove {}: {}", item, e);
				}
			}
		} else {
			println!("    Not found: {}", item);
		}
	}

	if !removed_items.is_empty() {
		println!(
			"Reset complete. Removed {} items: {}",
			removed_items.len(),
			removed_items.join(", ")
		);
	} else {
		println!(" No Spacedrive v2 data found to reset.");
	}

	if !errors.is_empty() {
		println!(" {} errors occurred during reset:", errors.len());
		for error in &errors {
			println!("   • {}", error);
		}
		// Don't fail the entire operation for partial cleanup failures
	}

	Ok(())
}

#[derive(Parser, Debug)]
#[command(name = "spacedrive", about = "Spacedrive v2 CLI (daemon client)")]
struct Cli {
	/// Path to spacedrive data directory
	#[arg(long)]
	data_dir: Option<std::path::PathBuf>,

	/// Daemon instance name
	#[arg(long)]
	instance: Option<String>,

	/// Output format
	#[arg(long, value_enum, default_value = "human")]
	format: OutputFormat,

	#[command(subcommand)]
	command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
	/// Start the Spacedrive daemon
	Start {
		/// Automatically start networking
		#[arg(long)]
		enable_networking: bool,
		/// Run daemon in foreground (show logs)
		#[arg(long)]
		foreground: bool,
	},
	/// Stop the Spacedrive daemon
	Stop {
		/// Reset all data (requires confirmation)
		#[arg(long)]
		reset: bool,
	},
	/// Restart the Spacedrive daemon
	Restart {
		/// Automatically start networking after restart
		#[arg(long)]
		enable_networking: bool,
		/// Run daemon in foreground after restart (show logs)
		#[arg(long)]
		foreground: bool,
		/// Reset all data before restart (requires confirmation)
		#[arg(long)]
		reset: bool,
	},
	/// Core info
	Status,
	/// Libraries operations
	#[command(subcommand)]
	Library(LibraryCmd),
	/// File operations
	#[command(subcommand)]
	File(FileCmd),
	/// Indexing operations
	#[command(subcommand)]
	Index(IndexCmd),
	/// Location operations
	#[command(subcommand)]
	Location(LocationCmd),
	/// Networking and pairing
	#[command(subcommand)]
	Network(NetworkCmd),
	/// Job commands
	#[command(subcommand)]
	Job(JobCmd),
	/// View and follow logs
	#[command(subcommand)]
	Logs(LogsCmd),
	/// Search operations
	#[command(subcommand)]
	Search(SearchCmd),
	/// Tag operations
	#[command(subcommand)]
	Tag(TagCmd),
}

#[tokio::main]
async fn main() -> Result<()> {
	let cli = Cli::parse();
	let data_dir = cli.data_dir.unwrap_or(sd_core::config::default_data_dir()?);
	let instance = cli.instance;

	// Validate instance name for security
	if let Some(ref inst) = instance {
		sd_core::infra::daemon::instance::validate_instance_name(inst)
			.map_err(|e| anyhow::anyhow!("Invalid instance name: {}", e))?;
	}

	let socket_path = if let Some(inst) = &instance {
		data_dir
			.join("daemon")
			.join(format!("daemon-{}.sock", inst))
	} else {
		data_dir.join("daemon/daemon.sock")
	};

	match cli.command {
		Commands::Start {
			enable_networking,
			foreground,
		} => {
			crate::ui::print_compact_logo();
			println!("Starting daemon...");

			// Check if daemon is already running
			let client = CoreClient::new(socket_path.clone());
			match client
				.send_raw_request(&sd_core::infra::daemon::types::DaemonRequest::Ping)
				.await
			{
				Ok(sd_core::infra::daemon::types::DaemonResponse::Pong) => {
					println!("Daemon is already running");
					return Ok(());
				}
				_ => {} // Daemon not running, continue
			}

			// Start daemon using std::process::Command
			let current_exe = std::env::current_exe()?;
			let daemon_path = current_exe.parent().unwrap().join("daemon");
			let mut command = std::process::Command::new(daemon_path);

			// Pass networking flag if enabled (if daemon supports it)
			if enable_networking {
				println!("Note: Networking flag passed but daemon may not support it yet");
			}

			// Set working directory to current directory
			command.current_dir(std::env::current_dir()?);

			if foreground {
				// Foreground mode: inherit stdout/stderr so logs are visible
				println!("Starting daemon in foreground mode...");
				println!("Press Ctrl+C to stop the daemon");
				println!("═══════════════════════════════════════════════════════");

				match command.status() {
					Ok(status) => {
						if status.success() {
							println!("Daemon exited successfully");
						} else {
							return Err(anyhow::anyhow!("Daemon exited with error: {}", status));
						}
					}
					Err(e) => {
						return Err(anyhow::anyhow!("Failed to start daemon: {}", e));
					}
				}
			} else {
				// Background mode: redirect stdout/stderr to null
				command.stdout(std::process::Stdio::null());
				command.stderr(std::process::Stdio::null());

				match command.spawn() {
					Ok(child) => {
						println!("Daemon started (PID: {})", child.id());

						// Wait a moment for daemon to start up
						tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

						// Verify daemon is responding
						match client
							.send_raw_request(&sd_core::infra::daemon::types::DaemonRequest::Ping)
							.await
						{
							Ok(sd_core::infra::daemon::types::DaemonResponse::Pong) => {
								println!("Daemon is ready and responding");
								println!("Use 'sd logs follow' to view daemon logs");
							}
							_ => {
								println!("Warning: Daemon may not be fully initialized yet");
								println!("Use 'sd logs follow' to check daemon status");
							}
						}
					}
					Err(e) => {
						return Err(anyhow::anyhow!("Failed to start daemon: {}", e));
					}
				}
			}
		}
		Commands::Stop { reset } => {
			if reset {
				use crate::util::confirm::confirm_or_abort;
				confirm_or_abort(
					" This will permanently delete Spacedrive v2 data (libraries, settings, logs). Other files in the data directory will be preserved. Are you sure?",
					false
				)?;
			}

			println!("Stopping daemon...");
			let core = CoreClient::new(socket_path.clone());
			let stop_result = core
				.send_raw_request(&sd_core::infra::daemon::types::DaemonRequest::Shutdown)
				.await;

			match stop_result {
				Ok(_) => println!("Daemon stopped."),
				Err(_) => {
					if reset {
						println!(" Daemon was not running, proceeding with reset...");
					} else {
						println!(" Daemon was not running or already stopped.");
					}
				}
			}

			if reset {
				println!("Resetting Spacedrive v2 data...");
				reset_spacedrive_v2_data(&data_dir)?;
			}
		}
		Commands::Restart {
			enable_networking,
			foreground,
			reset,
		} => {
			if reset {
				use crate::util::confirm::confirm_or_abort;
				confirm_or_abort(
					" This will permanently delete Spacedrive v2 data (libraries, settings, logs) before restart. Other files in the data directory will be preserved. Are you sure?",
					false
				)?;
			}

			// First, try to stop the daemon if it's running
			println!("Stopping daemon...");
			let core = CoreClient::new(socket_path.clone());
			let stop_result = core
				.send_raw_request(&sd_core::infra::daemon::types::DaemonRequest::Shutdown)
				.await;

			match stop_result {
				Ok(_) => println!("Daemon stopped."),
				Err(_) => println!(" Daemon was not running or already stopped."),
			}

			// Reset data if requested
			if reset {
				println!("Resetting Spacedrive v2 data...");
				reset_spacedrive_v2_data(&data_dir)?;
			}

			// Wait a moment for cleanup
			tokio::time::sleep(std::time::Duration::from_millis(500)).await;

			// Start the daemon again
			println!("Starting daemon...");
			let mut cmd = std::process::Command::new(std::env::current_exe()?);
			cmd.arg("start");

			if enable_networking {
				cmd.arg("--enable-networking");
			}

			if foreground {
				cmd.arg("--foreground");
				// Run in foreground - this will block
				let status = cmd.status()?;
				if !status.success() {
					return Err(anyhow::anyhow!("Failed to start daemon"));
				}
			} else {
				// Run in background
				let child = cmd.spawn()?;
				println!("Daemon restarted (PID: {})", child.id());

				// Wait a moment and check if it's still running
				tokio::time::sleep(std::time::Duration::from_millis(1000)).await;

				// Try to connect to verify it started successfully
				let core = CoreClient::new(socket_path.clone());
				match core
					.send_raw_request(&sd_core::infra::daemon::types::DaemonRequest::Ping)
					.await
				{
					Ok(_) => println!("Daemon restart successful"),
					Err(e) => {
						println!(" Warning: Could not verify daemon status: {}", e);
						println!("Use 'sd status' to check daemon status");
					}
				}
			}
		}
		_ => {
			run_client_command(cli.command, cli.format, data_dir, socket_path).await?;
		}
	}

	Ok(())
}

async fn run_client_command(
	command: Commands,
	format: OutputFormat,
	data_dir: std::path::PathBuf,
	socket_path: std::path::PathBuf,
) -> Result<()> {
	let core = CoreClient::new(socket_path.clone());
	let ctx = Context::new(core, format, data_dir, socket_path);
	match command {
		Commands::Status => {
			let status: sd_core::ops::core::status::output::CoreStatus = ctx
				.core
				.query(&sd_core::ops::core::status::query::CoreStatusQuery)
				.await?;
			match ctx.format {
				OutputFormat::Human => {
					crate::ui::print_logo();
					println!("Core Version: {}", status.version);
					println!("Libraries: {}", status.library_count);
					println!("Status: Running");
				}
				OutputFormat::Json => println!("{}", serde_json::to_string_pretty(&status)?),
			}
		}
		Commands::Library(cmd) => library::run(&ctx, cmd).await?,
		Commands::File(cmd) => file::run(&ctx, cmd).await?,
		Commands::Index(cmd) => index::run(&ctx, cmd).await?,
		Commands::Location(cmd) => location::run(&ctx, cmd).await?,
		Commands::Network(cmd) => network::run(&ctx, cmd).await?,
		Commands::Job(cmd) => job::run(&ctx, cmd).await?,
		Commands::Logs(cmd) => logs::run(&ctx, cmd).await?,
		Commands::Search(cmd) => search::run(&ctx, cmd).await?,
		Commands::Tag(cmd) => tag::run(&ctx, cmd).await?,
		_ => {} // Start and Stop are handled in main
	}
	Ok(())
}
