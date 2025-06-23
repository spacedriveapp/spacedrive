pub mod commands;
pub mod daemon;
pub mod monitor;
pub mod networking_commands;
pub mod pairing_ui;
pub mod state;

use crate::Core;
use clap::{Parser, Subcommand};
use std::collections::HashMap;
use std::path::PathBuf;
use uuid::Uuid;

#[derive(Parser)]
#[command(name = "spacedrive")]
#[command(about = "Spacedrive v2 CLI - Manage your libraries, locations, and jobs", long_about = None)]
pub struct Cli {
	/// Path to Spacedrive data directory
	#[arg(short, long, global = true)]
	pub data_dir: Option<PathBuf>,

	/// Enable debug logging
	#[arg(short = 'v', long, global = true)]
	pub verbose: bool,

	/// Daemon instance name (for multiple daemon support)
	#[arg(long, global = true)]
	pub instance: Option<String>,

	#[command(subcommand)]
	pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
	/// Manage libraries
	#[command(subcommand)]
	Library(commands::LibraryCommands),

	/// Manage locations
	#[command(subcommand)]
	Location(commands::LocationCommands),

	/// Manage and monitor jobs
	#[command(subcommand)]
	Job(commands::JobCommands),

	/// Enhanced indexing with scope and persistence options
	#[command(subcommand)]
	Index(commands::IndexCommands),

	/// Start a traditional indexing job (legacy)
	Scan {
		/// Path to index
		path: PathBuf,

		/// Indexing mode
		#[arg(short, long, value_enum, default_value = "content")]
		mode: commands::IndexMode,

		/// Monitor the job in real-time
		#[arg(short = 'w', long)]
		watch: bool,
	},

	/// Monitor all system activity in real-time
	Monitor,

	/// Show system status
	Status,

	/// Start the Spacedrive daemon in the background
	Start {
		/// Run in foreground instead of daemonizing
		#[arg(short, long)]
		foreground: bool,
		/// Enable networking on startup
		#[arg(long)]
		enable_networking: bool,
	},

	/// Stop the Spacedrive daemon
	Stop,

	/// Check if the daemon is running
	Daemon,

	/// Manage daemon instances
	#[command(subcommand)]
	Instance(InstanceCommands),

	/// Manage device networking and connections
	#[command(subcommand)]
	Network(commands::NetworkCommands),
}

#[derive(Subcommand, Clone)]
pub enum InstanceCommands {
	/// List all daemon instances
	List,
	/// Stop a specific daemon instance
	Stop { 
		/// Instance name to stop
		name: String 
	},
	/// Show currently targeted instance
	Current,
}

pub async fn run() -> Result<(), Box<dyn std::error::Error>> {
	let cli = Cli::parse();

	// Set up logging - respect RUST_LOG environment variable with fallback defaults
	let log_level = if cli.verbose { "debug" } else { "info" };
	let env_filter = tracing_subscriber::EnvFilter::try_from_default_env()
		.unwrap_or_else(|_| {
			// Fallback to hardcoded filters if RUST_LOG not set
			if cli.verbose {
				// Enable detailed networking and libp2p logging when verbose
				tracing_subscriber::EnvFilter::new(&format!(
					"sd_core_new={},spacedrive_cli={},libp2p=debug",
					log_level, log_level
				))
			} else {
				tracing_subscriber::EnvFilter::new(&format!(
					"sd_core_new={},spacedrive_cli={}",
					log_level, log_level
				))
			}
		});
	
	tracing_subscriber::fmt()
		.with_env_filter(env_filter)
		.init();

	// Determine data directory with instance isolation
	let base_data_dir = cli
		.data_dir
		.unwrap_or_else(|| PathBuf::from("./data/spacedrive-cli-data"));
		
	let data_dir = if let Some(ref instance) = cli.instance {
		base_data_dir.join(format!("instance-{}", instance))
	} else {
		base_data_dir
	};

	// Handle daemon commands first (they don't need Core)
	match &cli.command {
		Commands::Start {
			foreground,
			enable_networking,
		} => {
			return handle_start_daemon(data_dir, *foreground, *enable_networking, cli.instance.clone()).await;
		}
		Commands::Stop => {
			return handle_stop_daemon(cli.instance.clone()).await;
		}
		Commands::Daemon => {
			return handle_daemon_status(cli.instance.clone()).await;
		}
		Commands::Instance(instance_cmd) => {
			return handle_instance_command(instance_cmd.clone()).await;
		}
		_ => {
			// For all other commands, check if daemon is running
			if !daemon::Daemon::is_running_instance(cli.instance.clone()) {
				let instance_display = cli.instance.as_deref().unwrap_or("default");
				println!("❌ Spacedrive daemon instance '{}' is not running", instance_display);
				if cli.instance.is_some() {
					println!("   Start it with: spacedrive --instance {} start", instance_display);
				} else {
					println!("   Start it with: spacedrive start");
				}
				return Ok(());
			}
		}
	}

	// For library, location, and job commands, use the daemon
	match &cli.command {
		Commands::Library(library_cmd) => {
			return handle_library_daemon_command(library_cmd.clone(), cli.instance.clone()).await;
		}
		Commands::Location(location_cmd) => {
			return handle_location_daemon_command(location_cmd.clone(), cli.instance.clone()).await;
		}
		Commands::Job(job_cmd) => {
			return handle_job_daemon_command(job_cmd.clone(), cli.instance.clone()).await;
		}
		Commands::Network(network_cmd) => {
			return handle_network_daemon_command(network_cmd.clone(), cli.instance.clone()).await;
		}
		Commands::Monitor => {
			// Special case - monitor needs event streaming
			println!("📊 Job monitor not yet implemented for daemon mode");
			println!("   Use 'spacedrive job list' to see current jobs");
			return Ok(());
		}
		_ => {}
	}

	// Initialize core (temporary - for commands not yet converted to daemon)
	let core = Core::new_with_config(data_dir.clone()).await?;

	// Load CLI state (instance-specific)
	let state_path = if cli.instance.is_some() {
		data_dir.join("cli_state.json")
	} else {
		data_dir.join("cli_state.json")
	};
	let mut state = state::CliState::load(&data_dir)?;

	// Execute command
	match cli.command {
		Commands::Library(cmd) => commands::handle_library_command(cmd, &core, &mut state).await?,
		Commands::Location(cmd) => {
			commands::handle_location_command(cmd, &core, &mut state).await?
		}
		Commands::Job(cmd) => commands::handle_job_command(cmd, &core, &mut state).await?,
		Commands::Index(cmd) => commands::handle_index_command(cmd, &core, &mut state).await?,
		Commands::Network(cmd) => commands::handle_network_command(cmd, &core, &mut state).await?,
		Commands::Scan { path, mode, watch } => {
			commands::handle_legacy_scan_command(path, mode, watch, &core, &mut state).await?
		}
		Commands::Monitor => monitor::run_monitor(&core).await?,
		Commands::Status => commands::handle_status_command(&core, &state).await?,
		Commands::Start { .. } | Commands::Stop | Commands::Daemon | Commands::Instance(_) | Commands::Network(_) => {
			// These are handled above, should never reach here
			unreachable!()
		}
	}

	// Save state
	state.save(&data_dir)?;

	// Shutdown core
	core.shutdown().await?;

	Ok(())
}

async fn handle_start_daemon(
	data_dir: PathBuf,
	foreground: bool,
	enable_networking: bool,
	instance_name: Option<String>,
) -> Result<(), Box<dyn std::error::Error>> {
	if daemon::Daemon::is_running_instance(instance_name.clone()) {
		let instance_display = instance_name.as_deref().unwrap_or("default");
		println!("⚠️  Spacedrive daemon instance '{}' is already running", instance_display);
		return Ok(());
	}

	println!("🚀 Starting Spacedrive daemon...");

	if foreground {
		// Run in foreground
		if enable_networking {
			// For networking enabled startup, we need a default password
			// In production, this would be handled more securely
			let default_password = "spacedrive_default_key"; // This should be configurable
			println!("🔐 Starting daemon with networking enabled...");
			println!("   Using default networking configuration.");
			println!("   Use 'spacedrive network init --password <your_password>' to set a custom password.");

			match daemon::Daemon::new_with_networking_and_instance(data_dir.clone(), default_password, instance_name.clone()).await {
				Ok(daemon) => daemon.start().await?,
				Err(e) => {
					println!("❌ Failed to start daemon with networking: {}", e);
					println!("   Falling back to daemon without networking...");
					let daemon = daemon::Daemon::new_with_instance(data_dir, instance_name.clone()).await?;
					daemon.start().await?;
				}
			}
		} else {
			let daemon = daemon::Daemon::new_with_instance(data_dir, instance_name.clone()).await?;
			daemon.start().await?;
		}
	} else {
		// Daemonize (simplified version - in production use proper daemonization)
		use std::process::Command;

		let exe = std::env::current_exe()?;
		let mut cmd = Command::new(exe);
		cmd.arg("start")
			.arg("--foreground")
			.arg("--data-dir")
			.arg(data_dir);

		if let Some(ref instance) = instance_name {
			cmd.arg("--instance").arg(instance);
		}

		if enable_networking {
			cmd.arg("--enable-networking");
		}

		// Detach from terminal
		#[cfg(unix)]
		{
			use std::os::unix::process::CommandExt;
			cmd.stdin(std::process::Stdio::null())
				.stdout(std::process::Stdio::null())
				.stderr(std::process::Stdio::null());

			unsafe {
				cmd.pre_exec(|| {
					// Create new session
					libc::setsid();
					Ok(())
				});
			}
		}

		cmd.spawn()?;

		// Wait a bit to see if it started
		tokio::time::sleep(std::time::Duration::from_secs(2)).await;

		if daemon::Daemon::is_running_instance(instance_name.clone()) {
			let instance_display = instance_name.as_deref().unwrap_or("default");
			println!("✅ Spacedrive daemon instance '{}' started successfully", instance_display);
		} else {
			let instance_display = instance_name.as_deref().unwrap_or("default");
			println!("❌ Failed to start Spacedrive daemon instance '{}'", instance_display);
		}
	}

	Ok(())
}

async fn handle_stop_daemon(instance_name: Option<String>) -> Result<(), Box<dyn std::error::Error>> {
	if !daemon::Daemon::is_running_instance(instance_name.clone()) {
		let instance_display = instance_name.as_deref().unwrap_or("default");
		println!("⚠️  Spacedrive daemon instance '{}' is not running", instance_display);
		return Ok(());
	}

	let instance_display = instance_name.as_deref().unwrap_or("default");
	println!("🛑 Stopping Spacedrive daemon instance '{}'...", instance_display);
	daemon::Daemon::stop_instance(instance_name.clone()).await?;

	// Wait a bit to ensure it's stopped
	tokio::time::sleep(std::time::Duration::from_secs(1)).await;

	if !daemon::Daemon::is_running_instance(instance_name.clone()) {
		println!("✅ Spacedrive daemon instance '{}' stopped", instance_display);
	} else {
		println!("❌ Failed to stop Spacedrive daemon instance '{}'", instance_display);
	}

	Ok(())
}

async fn handle_daemon_status(instance_name: Option<String>) -> Result<(), Box<dyn std::error::Error>> {
	use colored::Colorize;

	let instance_display = instance_name.as_deref().unwrap_or("default");
	
	if daemon::Daemon::is_running_instance(instance_name.clone()) {
		println!("✅ Spacedrive daemon instance '{}' is running", instance_display);

		// Try to get more info from daemon
		let client = daemon::DaemonClient::new_with_instance(instance_name);

		// Get status
		match client.send_command(daemon::DaemonCommand::GetStatus).await {
			Ok(daemon::DaemonResponse::Status(status)) => {
				println!("\n📊 Status:");
				println!("   Version: {}", status.version.bright_blue());
				println!(
					"   Uptime: {} seconds",
					status.uptime_secs.to_string().bright_yellow()
				);
				println!(
					"   Active Jobs: {}",
					status.active_jobs.to_string().bright_green()
				);
				println!("   Total Locations: {}", status.total_locations);
			}
			Err(e) => {
				println!("   ⚠️  Could not get status: {}", e);
			}
			_ => {}
		}

		// Get libraries
		match client
			.send_command(daemon::DaemonCommand::ListLibraries)
			.await
		{
			Ok(daemon::DaemonResponse::Libraries(libraries)) => {
				println!("\n📚 Libraries:");
				if libraries.is_empty() {
					println!("   No libraries found");
				} else {
					for lib in &libraries {
						println!(
							"   • {} ({})",
							lib.name.bright_cyan(),
							lib.id.to_string().bright_yellow()
						);
					}
				}
			}
			Err(e) => {
				println!("   ⚠️  Could not get libraries: {}", e);
			}
			_ => {}
		}

		// Get current library
		match client
			.send_command(daemon::DaemonCommand::GetCurrentLibrary)
			.await
		{
			Ok(daemon::DaemonResponse::CurrentLibrary(Some(lib))) => {
				println!("\n🔍 Current Library:");
				println!(
					"   {} ({})",
					lib.name.bright_cyan().bold(),
					lib.id.to_string().bright_yellow()
				);
				println!("   Path: {}", lib.path.display().to_string().bright_blue());
			}
			Ok(daemon::DaemonResponse::CurrentLibrary(None)) => {
				println!("\n🔍 Current Library: None selected");
			}
			Err(e) => {
				println!("   ⚠️  Could not get current library: {}", e);
			}
			_ => {}
		}
	} else {
		println!("❌ Spacedrive daemon instance '{}' is not running", instance_display);
		if instance_name.is_some() {
			println!("   Start it with: spacedrive --instance {} start", instance_display);
		} else {
			println!("   Start it with: spacedrive start");
		}
	}

	Ok(())
}

async fn handle_instance_command(
	cmd: InstanceCommands,
) -> Result<(), Box<dyn std::error::Error>> {
	use colored::Colorize;

	match cmd {
		InstanceCommands::List => {
			match daemon::Daemon::list_instances() {
				Ok(instances) => {
					if instances.is_empty() {
						println!("📭 No daemon instances found");
					} else {
						use comfy_table::Table;
						let mut table = Table::new();
						table.set_header(vec!["Instance", "Status", "Socket Path"]);

						for instance in instances {
							let status = if instance.is_running {
								"Running".green()
							} else {
								"Stopped".red()
							};

							table.add_row(vec![
								instance.display_name().to_string(),
								status.to_string(),
								instance.socket_path.display().to_string(),
							]);
						}

						println!("{}", table);
					}
				}
				Err(e) => {
					println!("❌ Failed to list instances: {}", e);
				}
			}
		}

		InstanceCommands::Stop { name } => {
			let instance_name = if name == "default" { None } else { Some(name.clone()) };
			match daemon::Daemon::stop_instance(instance_name).await {
				Ok(_) => {
					println!("✅ Daemon instance '{}' stopped", name);
				}
				Err(e) => {
					println!("❌ Failed to stop instance '{}': {}", name, e);
				}
			}
		}

		InstanceCommands::Current => {
			// This would show the current instance based on CLI args or context
			println!("Current instance functionality not yet implemented");
			println!("Use --instance <name> flag to target specific instances");
		}
	}

	Ok(())
}

async fn handle_library_daemon_command(
	cmd: commands::LibraryCommands,
	instance_name: Option<String>,
) -> Result<(), Box<dyn std::error::Error>> {
	use colored::Colorize;

	let client = daemon::DaemonClient::new_with_instance(instance_name.clone());

	match cmd {
		commands::LibraryCommands::Create { name, path } => {
			println!("📚 Creating library '{}'...", name.bright_cyan());

			match client
				.send_command(daemon::DaemonCommand::CreateLibrary {
					name: name.clone(),
					path,
				})
				.await
			{
				Ok(daemon::DaemonResponse::LibraryCreated { id, name, path }) => {
					println!("✅ Library created successfully!");
					println!("   ID: {}", id.to_string().bright_yellow());
					println!("   Path: {}", path.display().to_string().bright_blue());
					println!("   Status: {}", "Active".bright_green());
				}
				Ok(daemon::DaemonResponse::Error(e)) => {
					println!("❌ Failed to create library: {}", e);
				}
				Err(e) => {
					println!("❌ Failed to communicate with daemon: {}", e);
				}
				_ => {
					println!("❌ Unexpected response from daemon");
				}
			}
		}

		commands::LibraryCommands::List => {
			match client
				.send_command(daemon::DaemonCommand::ListLibraries)
				.await
			{
				Ok(daemon::DaemonResponse::Libraries(libraries)) => {
					if libraries.is_empty() {
						println!("📭 No libraries found. Create one with: spacedrive library create <name>");
					} else {
						use comfy_table::Table;
						let mut table = Table::new();
						table.set_header(vec!["ID", "Name", "Path"]);

						for lib in libraries {
							table.add_row(vec![
								lib.id.to_string(),
								lib.name,
								lib.path.display().to_string(),
							]);
						}

						println!("{}", table);
					}
				}
				Ok(daemon::DaemonResponse::Error(e)) => {
					println!("❌ Failed to list libraries: {}", e);
				}
				Err(e) => {
					println!("❌ Failed to communicate with daemon: {}", e);
				}
				_ => {
					println!("❌ Unexpected response from daemon");
				}
			}
		}

		commands::LibraryCommands::Switch { identifier } => {
			match client
				.send_command(daemon::DaemonCommand::SwitchLibrary {
					id: identifier.parse()?,
				})
				.await
			{
				Ok(daemon::DaemonResponse::Ok) => {
					println!("✅ Switched library successfully");
				}
				Ok(daemon::DaemonResponse::Error(e)) => {
					println!("❌ Failed to switch library: {}", e);
				}
				Err(e) => {
					println!("❌ Failed to communicate with daemon: {}", e);
				}
				_ => {
					println!("❌ Unexpected response from daemon");
				}
			}
		}

		commands::LibraryCommands::Current => {
			match client
				.send_command(daemon::DaemonCommand::GetCurrentLibrary)
				.await
			{
				Ok(daemon::DaemonResponse::CurrentLibrary(Some(lib))) => {
					println!("📚 Current library: {}", lib.name.bright_cyan());
					println!("   ID: {}", lib.id.to_string().bright_yellow());
					println!("   Path: {}", lib.path.display().to_string().bright_blue());
				}
				Ok(daemon::DaemonResponse::CurrentLibrary(None)) => {
					println!("⚠️  No current library selected");
				}
				Ok(daemon::DaemonResponse::Error(e)) => {
					println!("❌ Error: {}", e);
				}
				Err(e) => {
					println!("❌ Failed to communicate with daemon: {}", e);
				}
				_ => {
					println!("❌ Unexpected response from daemon");
				}
			}
		}

		_ => {
			println!("❌ Command not yet implemented for daemon mode");
		}
	}

	Ok(())
}

async fn handle_location_daemon_command(
	cmd: commands::LocationCommands,
	instance_name: Option<String>,
) -> Result<(), Box<dyn std::error::Error>> {
	use colored::Colorize;

	let client = daemon::DaemonClient::new_with_instance(instance_name.clone());

	match cmd {
		commands::LocationCommands::Add { path, name, mode } => {
			println!(
				"📁 Adding location {}...",
				path.display().to_string().bright_blue()
			);

			match client
				.send_command(daemon::DaemonCommand::AddLocation {
					path: path.clone(),
					name,
				})
				.await
			{
				Ok(daemon::DaemonResponse::LocationAdded {
					location_id,
					job_id,
				}) => {
					println!("✅ Location added successfully!");
					println!("   Path: {}", path.display().to_string().bright_blue());
					println!(
						"   Location ID: {}",
						location_id.to_string().bright_yellow()
					);

					if !job_id.is_empty() {
						println!(
							"   Job ID: {}",
							job_id.chars().take(8).collect::<String>().bright_yellow()
						);

						// Automatically show brief progress info
						println!("\n📊 Indexing started...");
						println!(
							"   To monitor detailed progress, run: {}",
							"spacedrive job monitor".bright_cyan()
						);

						// Show basic progress by checking job status periodically
						let mut last_status = String::new();
						for _ in 0..10 {
							// Check for 10 seconds
							tokio::time::sleep(std::time::Duration::from_secs(1)).await;

							if let Ok(uuid) = job_id.parse::<uuid::Uuid>() {
								match client
									.send_command(daemon::DaemonCommand::GetJobInfo { id: uuid })
									.await
								{
									Ok(daemon::DaemonResponse::JobInfo(Some(job))) => {
										if job.status != last_status {
											println!(
												"   Status: {} ({}%)",
												job.status.bright_yellow(),
												(job.progress * 100.0) as u32
											);
											last_status = job.status.clone();
										}

										if job.status == "completed" || job.status == "failed" {
											break;
										}
									}
									_ => {}
								}
							}
						}
					} else {
						println!("   Status: Location added but indexing failed to start");
					}
				}
				Ok(daemon::DaemonResponse::Error(e)) => {
					println!("❌ Failed to add location: {}", e);
				}
				Err(e) => {
					println!("❌ Failed to communicate with daemon: {}", e);
				}
				_ => {
					println!("❌ Unexpected response from daemon");
				}
			}
		}

		commands::LocationCommands::List => {
			match client
				.send_command(daemon::DaemonCommand::ListLocations)
				.await
			{
				Ok(daemon::DaemonResponse::Locations(locations)) => {
					if locations.is_empty() {
						println!(
							"📭 No locations found. Add one with: spacedrive location add <path>"
						);
					} else {
						use comfy_table::Table;
						let mut table = Table::new();
						table.set_header(vec!["ID", "Name", "Path", "Status"]);

						for loc in locations {
							table.add_row(vec![
								loc.id.to_string(),
								loc.name,
								loc.path.display().to_string(),
								loc.status,
							]);
						}

						println!("{}", table);
					}
				}
				Ok(daemon::DaemonResponse::Error(e)) => {
					println!("❌ Failed to list locations: {}", e);
				}
				Err(e) => {
					println!("❌ Failed to communicate with daemon: {}", e);
				}
				_ => {
					println!("❌ Unexpected response from daemon");
				}
			}
		}

		commands::LocationCommands::Remove { identifier } => {
			println!("🗑️  Removing location {}...", identifier.bright_yellow());

			// Try to parse as UUID
			let id = match identifier.parse::<Uuid>() {
				Ok(id) => id,
				Err(_) => {
					println!("❌ Invalid location ID: {}", identifier);
					return Ok(());
				}
			};

			match client
				.send_command(daemon::DaemonCommand::RemoveLocation { id })
				.await
			{
				Ok(daemon::DaemonResponse::Ok) => {
					println!("✅ Location removed successfully");
				}
				Ok(daemon::DaemonResponse::Error(e)) => {
					println!("❌ Failed to remove location: {}", e);
				}
				Err(e) => {
					println!("❌ Failed to communicate with daemon: {}", e);
				}
				_ => {
					println!("❌ Unexpected response from daemon");
				}
			}
		}

		commands::LocationCommands::Rescan { identifier, force } => {
			println!("🔄 Rescanning location {}...", identifier.bright_yellow());

			// Try to parse as UUID
			let id = match identifier.parse::<Uuid>() {
				Ok(id) => id,
				Err(_) => {
					println!("❌ Invalid location ID: {}", identifier);
					return Ok(());
				}
			};

			match client
				.send_command(daemon::DaemonCommand::RescanLocation { id })
				.await
			{
				Ok(daemon::DaemonResponse::Ok) => {
					println!("✅ Rescan started successfully");
					println!("   Use 'spacedrive job monitor' to track progress");
				}
				Ok(daemon::DaemonResponse::Error(e)) => {
					println!("❌ Failed to rescan location: {}", e);
				}
				Err(e) => {
					println!("❌ Failed to communicate with daemon: {}", e);
				}
				_ => {
					println!("❌ Unexpected response from daemon");
				}
			}
		}

		commands::LocationCommands::Info { identifier } => {
			println!("❌ Location info command not yet implemented");
		}
	}

	Ok(())
}

async fn handle_job_daemon_command(
	cmd: commands::JobCommands,
	instance_name: Option<String>,
) -> Result<(), Box<dyn std::error::Error>> {
	use colored::Colorize;

	let client = daemon::DaemonClient::new_with_instance(instance_name.clone());

	match cmd {
		commands::JobCommands::List { status, recent: _ } => {
			let status_filter = status.map(|s| s.to_lowercase());

			match client
				.send_command(daemon::DaemonCommand::ListJobs {
					status: status_filter,
				})
				.await
			{
				Ok(daemon::DaemonResponse::Jobs(jobs)) => {
					if jobs.is_empty() {
						println!("📭 No jobs found");
					} else {
						use comfy_table::Table;
						let mut table = Table::new();
						table.set_header(vec!["ID", "Name", "Status", "Progress"]);

						for job in jobs {
							let progress_str = if job.status == "running" {
								format!("{}%", (job.progress * 100.0) as u32)
							} else {
								"-".to_string()
							};

							let status_colored = match job.status.as_str() {
								"running" => job.status.bright_yellow(),
								"completed" => job.status.bright_green(),
								"failed" => job.status.bright_red(),
								_ => job.status.normal(),
							};

							table.add_row(vec![
								job.id.to_string(),
								job.name,
								status_colored.to_string(),
								progress_str,
							]);
						}

						println!("{}", table);
					}
				}
				Ok(daemon::DaemonResponse::Error(e)) => {
					println!("❌ Failed to list jobs: {}", e);
				}
				Err(e) => {
					println!("❌ Failed to communicate with daemon: {}", e);
				}
				_ => {
					println!("❌ Unexpected response from daemon");
				}
			}
		}

		commands::JobCommands::Info { id } => {
			match client
				.send_command(daemon::DaemonCommand::GetJobInfo { id })
				.await
			{
				Ok(daemon::DaemonResponse::JobInfo(Some(job))) => {
					println!("📋 Job Information");
					println!("   ID: {}", job.id.to_string().bright_yellow());
					println!("   Name: {}", job.name.bright_cyan());
					println!(
						"   Status: {}",
						match job.status.as_str() {
							"running" => job.status.bright_yellow(),
							"completed" => job.status.bright_green(),
							"failed" => job.status.bright_red(),
							_ => job.status.normal(),
						}
					);
					println!("   Progress: {}%", (job.progress * 100.0) as u32);
				}
				Ok(daemon::DaemonResponse::JobInfo(None)) => {
					println!("❌ Job not found");
				}
				Ok(daemon::DaemonResponse::Error(e)) => {
					println!("❌ Error: {}", e);
				}
				Err(e) => {
					println!("❌ Failed to communicate with daemon: {}", e);
				}
				_ => {
					println!("❌ Unexpected response from daemon");
				}
			}
		}

		commands::JobCommands::Monitor { job_id } => {
			println!(
				"📡 {} - Press Ctrl+C to exit",
				"Spacedrive Job Monitor".bright_cyan()
			);
			println!("═══════════════════════════════════════════");
			println!();

			// Create progress bars for active jobs
			use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
			let multi_progress = MultiProgress::new();
			let mut job_bars: HashMap<String, ProgressBar> = HashMap::new();

			let style = ProgressStyle::with_template(
				"{spinner:.green} {prefix:.bold.cyan} [{bar:40.green/blue}] {percent}% | {msg}",
			)
			.unwrap()
			.progress_chars("█▓▒░");

			// If monitoring specific job
			if let Some(ref specific_job_id) = job_id {
				println!(
					"📊 Monitoring job {}...\n",
					specific_job_id
						.chars()
						.take(8)
						.collect::<String>()
						.bright_yellow()
				);
			} else {
				println!("⏳ Monitoring all jobs...\n");
			}

			// Poll for job updates
			loop {
				tokio::select! {
					_ = tokio::time::sleep(std::time::Duration::from_secs(1)) => {
						// Get job list
						match client.send_command(daemon::DaemonCommand::ListJobs { status: Some("running".to_string()) }).await {
							Ok(daemon::DaemonResponse::Jobs(jobs)) => {
								for job in &jobs {
									// Filter by specific job if requested
									if let Some(ref specific_id) = job_id {
										if !job.id.to_string().starts_with(specific_id) {
											continue;
										}
									}

									// Get or create progress bar
									let pb = job_bars.entry(job.id.to_string()).or_insert_with(|| {
										let bar = multi_progress.add(ProgressBar::new(100));
										bar.set_style(style.clone());
										bar.set_prefix(format!("{} [{}]",
											job.name.bright_cyan(),
											job.id.to_string().chars().take(8).collect::<String>()
										));
										bar
									});

									// Update progress
									pb.set_position((job.progress * 100.0) as u64);
									pb.set_message(format!("Status: {}", job.status.bright_yellow()));
								}

								// Clean up completed jobs
								let active_job_ids: std::collections::HashSet<String> = jobs.iter()
									.map(|j| j.id.to_string())
									.collect();

								job_bars.retain(|job_id, pb| {
									let should_keep = active_job_ids.contains(job_id);
									if !should_keep {
										pb.finish_with_message("✅ Completed".bright_green().to_string());
									}
									should_keep
								});

								if jobs.is_empty() && job_bars.is_empty() {
									println!("📭 No active jobs found");
								}
							}
							_ => {}
						}
					}

					_ = tokio::signal::ctrl_c() => {
						println!("\n\n👋 Exiting monitor...");
						break;
					}
				}
			}
		}

		_ => {
			println!("❌ Command not yet implemented for daemon mode");
		}
	}

	Ok(())
}

async fn handle_network_daemon_command(
	cmd: commands::NetworkCommands,
	instance_name: Option<String>,
) -> Result<(), Box<dyn std::error::Error>> {
	use colored::Colorize;

	let client = daemon::DaemonClient::new_with_instance(instance_name.clone());

	// Check if daemon is running for most commands
	match &cmd {
		commands::NetworkCommands::Init { .. } => {
			// Init doesnt require daemon to be running
		}
		_ => {
			if !client.is_running() {
				println!(
					"{} Daemon is not running. Start it with: {}",
					"✗".red(),
					"spacedrive start".bright_blue()
				);
				return Ok(());
			}
		}
	}

	match cmd {
		commands::NetworkCommands::Init { password } => {
			match client
				.send_command(daemon::DaemonCommand::InitNetworking { password })
				.await?
			{
				daemon::DaemonResponse::Ok => {
					println!("{} Networking initialized successfully", "✓".green());
				}
				daemon::DaemonResponse::Error(err) => {
					println!("{} {}", "✗".red(), err);
				}
				_ => {
					println!("{} Unexpected response", "✗".red());
				}
			}
		}

		commands::NetworkCommands::Start => {
			match client
				.send_command(daemon::DaemonCommand::StartNetworking)
				.await?
			{
				daemon::DaemonResponse::Ok => {
					println!("{} Networking service started", "✓".green());
				}
				daemon::DaemonResponse::Error(err) => {
					println!("{} {}", "✗".red(), err);
				}
				_ => {
					println!("{} Unexpected response", "✗".red());
				}
			}
		}

		commands::NetworkCommands::Stop => {
			match client
				.send_command(daemon::DaemonCommand::StopNetworking)
				.await?
			{
				daemon::DaemonResponse::Ok => {
					println!("{} Networking service stopped", "✓".green());
				}
				daemon::DaemonResponse::Error(err) => {
					println!("{} {}", "✗".red(), err);
				}
				_ => {
					println!("{} Unexpected response", "✗".red());
				}
			}
		}

		commands::NetworkCommands::Devices => {
			match client
				.send_command(daemon::DaemonCommand::ListConnectedDevices)
				.await?
			{
				daemon::DaemonResponse::ConnectedDevices(devices) => {
					if devices.is_empty() {
						println!("No devices currently connected");
					} else {
						println!("Connected devices:");
						use comfy_table::{presets::UTF8_FULL, Table};
						let mut table = Table::new();
						table.load_preset(UTF8_FULL);
						table.set_header(vec!["Device ID", "Name", "Status", "Last Seen"]);

						for device in devices {
							table.add_row(vec![
								&device.device_id.to_string()[..8],
								&device.device_name,
								&device.status,
								&device.last_seen,
							]);
						}

						println!("{}", table);
					}
				}
				daemon::DaemonResponse::Error(err) => {
					println!("{} {}", "✗".red(), err);
				}
				_ => {
					println!("{} Unexpected response", "✗".red());
				}
			}
		}

		commands::NetworkCommands::Revoke { device_id } => {
			match client
				.send_command(daemon::DaemonCommand::RevokeDevice { device_id })
				.await?
			{
				daemon::DaemonResponse::Ok => {
					println!("{} Device {} revoked", "✓".green(), device_id);
				}
				daemon::DaemonResponse::Error(err) => {
					println!("{} {}", "✗".red(), err);
				}
				_ => {
					println!("{} Unexpected response", "✗".red());
				}
			}
		}

		commands::NetworkCommands::Spacedrop {
			device_id,
			file_path,
			sender,
			message,
		} => {
			match client
				.send_command(daemon::DaemonCommand::SendSpacedrop {
					device_id,
					file_path: file_path.to_string_lossy().to_string(),
					sender_name: sender,
					message,
				})
				.await?
			{
				daemon::DaemonResponse::SpacedropStarted { transfer_id } => {
					println!(
						"{} Spacedrop started with transfer ID: {}",
						"✓".green(),
						transfer_id
					);
				}
				daemon::DaemonResponse::Error(err) => {
					println!("{} {}", "✗".red(), err);
				}
				_ => {
					println!("{} Unexpected response", "✗".red());
				}
			}
		}

		commands::NetworkCommands::Pair { action } => {
			// Delegate to the complete implementation in commands.rs
			commands::handle_pairing_command(action, &client).await?;
		}
	}

	Ok(())
}
