#![allow(warnings)]
use anyhow::Result;
use clap::{Parser, Subcommand};
use comfy_table::{presets::UTF8_BORDERS_ONLY, Attribute, Cell, Table};
use sd_core::client::CoreClient;
use std::path::Path;

fn format_bytes(bytes: u64) -> String {
	const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB"];
	let mut size = bytes as f64;
	let mut unit_index = 0;

	while size >= 1024.0 && unit_index < UNITS.len() - 1 {
		size /= 1024.0;
		unit_index += 1;
	}

	if unit_index == 0 {
		format!("{} {}", bytes, UNITS[unit_index])
	} else {
		format!("{:.1} {}", size, UNITS[unit_index])
	}
}

/// Validate instance name to prevent path traversal attacks
fn validate_instance_name(instance: &str) -> Result<(), String> {
	if instance.is_empty() {
		return Err("Instance name cannot be empty".to_string());
	}
	if instance.len() > 64 {
		return Err("Instance name too long (max 64 characters)".to_string());
	}
	if !instance
		.chars()
		.all(|c| c.is_alphanumeric() || c == '-' || c == '_')
	{
		return Err("Instance name contains invalid characters. Only alphanumeric, dash, and underscore allowed".to_string());
	}
	Ok(())
}

mod config;
mod context;
mod domains;
mod ui;
mod util;

use crate::context::{Context, OutputFormat};
use crate::domains::{
	devices::{self, DevicesCmd},
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
		/// Run daemon in foreground after restart (show logs)
		#[arg(long)]
		foreground: bool,
		/// Reset all data before restart (requires confirmation)
		#[arg(long)]
		reset: bool,
	},
	/// Core info
	Status,
	/// Device operations (library database)
	#[command(subcommand)]
	Devices(DevicesCmd),
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
		validate_instance_name(inst)
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
		Commands::Start { foreground } => {
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

			// Pass data directory
			command.arg("--data-dir").arg(&data_dir);

			// Pass instance name if specified
			if let Some(ref inst) = instance {
				command.arg("--instance").arg(inst);
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
				Ok(_) => {
					println!("Daemon shutdown initiated.");
					println!("Note: If jobs are running, the daemon will wait for them to pause before fully shutting down.");
					println!("Use 'sd logs follow' to monitor shutdown progress.");
				}
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
		Commands::Restart { foreground, reset } => {
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
				Ok(_) => {
					println!("Daemon shutdown initiated.");
					println!("Waiting for daemon to fully shut down before restart...");
					// Give some time for shutdown to complete
					tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
				}
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
			let current_exe = std::env::current_exe()?;
			let daemon_path = current_exe.parent().unwrap().join("daemon");
			let mut cmd = std::process::Command::new(daemon_path);

			// Pass data directory
			cmd.arg("--data-dir").arg(&data_dir);

			// Pass instance name if specified
			if let Some(ref inst) = instance {
				cmd.arg("--instance").arg(inst);
			}

			// Set working directory to current directory
			cmd.current_dir(std::env::current_dir()?);

			if foreground {
				// Foreground mode: inherit stdout/stderr so logs are visible
				println!("Starting daemon in foreground mode...");
				println!("Press Ctrl+C to stop the daemon");
				println!("═══════════════════════════════════════════════════════");

				match cmd.status() {
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
				// Run in background
				cmd.stdout(std::process::Stdio::null());
				cmd.stderr(std::process::Stdio::null());

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
	// Initialize device ID from device.json if it exists
	if let Ok(device_config) = std::fs::read_to_string(data_dir.join("device.json")) {
		if let Ok(device_json) = serde_json::from_str::<serde_json::Value>(&device_config) {
			if let Some(device_id_str) = device_json.get("id").and_then(|v| v.as_str()) {
				if let Ok(device_id) = uuid::Uuid::parse_str(device_id_str) {
					sd_core::device::set_current_device_id(device_id);
				}
			}
		}
	}

	let core = CoreClient::new(socket_path.clone());
	let mut ctx = Context::new(core, format, data_dir, socket_path)?;

	ctx.validate_and_fix_library().await?;

	match command {
		Commands::Status => {
			let status: sd_core::ops::core::status::output::CoreStatus =
				execute_core_query!(ctx, ());
			match ctx.format {
				OutputFormat::Human => {
					// Display logo
					crate::ui::logo::print_logo_colored();
					println!();

					// Device Information
					let mut device_table = Table::new();
					device_table.load_preset(UTF8_BORDERS_ONLY);
					device_table.set_header(vec![
						Cell::new("Device Information").add_attribute(Attribute::Bold),
						Cell::new(""),
					]);
					device_table.add_row(vec!["Name", &status.device_info.name]);
					device_table.add_row(vec!["ID", &status.device_info.id.to_string()]);
					device_table.add_row(vec!["OS", &status.device_info.os]);
					if let Some(model) = &status.device_info.hardware_model {
						device_table.add_row(vec!["Hardware", model]);
					}
					device_table.add_row(vec![
						"Created",
						&status
							.device_info
							.created_at
							.format("%Y-%m-%d %H:%M:%S UTC")
							.to_string(),
					]);
					println!("{}", device_table);
					println!();

					// System Status
					let mut system_table = Table::new();
					system_table.load_preset(UTF8_BORDERS_ONLY);
					system_table.set_header(vec![
						Cell::new("System Status").add_attribute(Attribute::Bold),
						Cell::new(""),
					]);
					system_table.add_row(vec!["Core Version", &status.version]);
					system_table.add_row(vec!["Built At", &status.built_at]);
					system_table.add_row(vec!["Data Directory", &status.system.data_directory]);
					if let Some(instance) = &status.system.instance_name {
						system_table.add_row(vec!["Instance", instance]);
					}
					if let Some(current_lib) = &status.system.current_library {
						system_table.add_row(vec!["Current Library", current_lib]);
					} else {
						system_table.add_row(vec!["Current Library", "None"]);
					}
					if let Some(uptime) = status.system.uptime {
						let hours = uptime / 3600;
						let minutes = (uptime % 3600) / 60;
						system_table.add_row(vec!["Uptime", &format!("{}h {}m", hours, minutes)]);
					}
					system_table.add_row(vec!["Status", "● Running"]);
					println!("{}", system_table);
					println!();

					// Libraries
					let mut libraries_table = Table::new();
					libraries_table.load_preset(UTF8_BORDERS_ONLY);
					libraries_table.set_header(vec![
						Cell::new(format!("Libraries ({})", status.library_count))
							.add_attribute(Attribute::Bold),
						Cell::new(""),
					]);
					if status.libraries.is_empty() {
						libraries_table
							.add_row(vec!["No libraries found".to_string(), "".to_string()]);
					} else {
						for lib in &status.libraries {
							let lib_name = format!("● {}", lib.name);
							libraries_table.add_row(vec![lib_name, lib.id.to_string()]);
							libraries_table.add_row(vec![
								format!("  Path: {}", lib.path.display()),
								"".to_string(),
							]);
							if let Some(stats) = &lib.stats {
								libraries_table.add_row(vec![
									format!("  Files: {}", stats.total_files),
									"".to_string(),
								]);
								libraries_table.add_row(vec![
									format!("  Size: {}", format_bytes(stats.total_size)),
									"".to_string(),
								]);
								libraries_table.add_row(vec![
									format!("  Locations: {}", stats.location_count),
									"".to_string(),
								]);
							}
						}
					}
					println!("{}", libraries_table);
					println!();

					// Services
					let mut services_table = Table::new();
					services_table.load_preset(UTF8_BORDERS_ONLY);
					services_table.set_header(vec![
						Cell::new("Services").add_attribute(Attribute::Bold),
						Cell::new(""),
					]);
					let services = &status.services;

					let watcher_status = if services.location_watcher.running {
						"● Running"
					} else {
						"○ Stopped"
					};
					services_table.add_row(vec!["Location Watcher", watcher_status]);

					let net_status = if services.networking.running {
						"● Running"
					} else {
						"○ Stopped"
					};
					services_table.add_row(vec!["Networking", net_status]);

					let vol_status = if services.volume_monitor.running {
						"● Running"
					} else {
						"○ Stopped"
					};
					services_table.add_row(vec!["Volume Monitor", vol_status]);

					let share_status = if services.file_sharing.running {
						"● Running"
					} else {
						"○ Stopped"
					};
					services_table.add_row(vec!["File Sharing", share_status]);
					println!("{}", services_table);
					println!();

					// Network
					let mut network_table = Table::new();
					network_table.load_preset(UTF8_BORDERS_ONLY);
					network_table.set_header(vec![
						Cell::new("Network").add_attribute(Attribute::Bold),
						Cell::new(""),
					]);
					if status.network.running {
						network_table.add_row(vec!["Status", "● Running"]);

						if let Some(node_id) = &status.network.node_id {
							let node_id_display = if node_id.len() > 50 {
								format!("{}...", &node_id[..47])
							} else {
								node_id.clone()
							};
							network_table.add_row(vec!["Node ID", &node_id_display]);
						}

						network_table.add_row(vec![
							"Connected Devices",
							&status.network.connected_devices.to_string(),
						]);

						network_table.add_row(vec![
							"Paired Devices",
							&status.network.paired_devices.to_string(),
						]);

						if !status.network.addresses.is_empty() {
							network_table.add_row(vec![
								"Addresses",
								&format!("{} address(es)", status.network.addresses.len()),
							]);
							for addr in &status.network.addresses {
								network_table.add_row(vec![format!("  {}", addr), "".to_string()]);
							}
						}

						network_table.add_row(vec!["Version", &status.network.version]);
					} else {
						network_table.add_row(vec!["Status", "○ Stopped"]);
					}
					println!("{}", network_table);
				}
				OutputFormat::Json => println!("{}", serde_json::to_string_pretty(&status)?),
			}
		}
		Commands::Devices(cmd) => devices::run(&ctx, cmd).await?,
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
