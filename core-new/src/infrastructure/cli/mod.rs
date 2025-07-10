pub mod adapters;
pub mod commands;
pub mod daemon;
pub mod monitoring;
pub mod networking_commands;
pub mod pairing_ui;
pub mod state;
pub mod utils;

use crate::Core;
use clap::{Parser, Subcommand};
use std::collections::HashMap;
use std::path::PathBuf;
use uuid::Uuid;

#[derive(Parser)]
#[command(name = "spacedrive")]
#[command(about = "Spacedrive v2 CLI", long_about = None)]
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
		mode: commands::CliIndexMode,

		/// Monitor the job in real-time
		#[arg(short = 'w', long)]
		watch: bool,
	},

	/// Monitor all system activity in real-time
	Monitor,

	/// Monitor daemon logs in real-time
	Logs {
		/// Number of lines to show initially
		#[arg(short, long, default_value = "50")]
		lines: usize,
		/// Follow logs in real-time
		#[arg(short, long)]
		follow: bool,
	},

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

	/// Copy files using the action system
	Copy(adapters::FileCopyCliArgs),
}

#[derive(Subcommand, Clone)]
pub enum InstanceCommands {
	/// List all daemon instances
	List,
	/// Stop a specific daemon instance
	Stop {
		/// Instance name to stop
		name: String,
	},
	/// Show currently targeted instance
	Current,
}

pub async fn run() -> Result<(), Box<dyn std::error::Error>> {
	let cli = Cli::parse();

	// Set up logging - skip for daemon start commands as they handle their own logging
	let is_daemon_start = matches!(&cli.command, Commands::Start { .. });
	if !is_daemon_start {
		let log_level = if cli.verbose { "debug" } else { "info" };
		let env_filter =
			tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| {
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

		tracing_subscriber::fmt().with_env_filter(env_filter).init();
	}

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
			return handle_start_daemon(
				data_dir,
				*foreground,
				*enable_networking,
				cli.instance.clone(),
			)
			.await;
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
				println!(
					"❌ Spacedrive daemon instance '{}' is not running",
					instance_display
				);
				if cli.instance.is_some() {
					println!(
						"   Start it with: spacedrive --instance {} start",
						instance_display
					);
				} else {
					println!("   Start it with: spacedrive start");
				}
				return Ok(());
			}
		}
	}

	// All commands require daemon to be running - no fallback Core creation
	match &cli.command {
		Commands::Library(library_cmd) => {
			return handle_library_daemon_command(library_cmd.clone(), cli.instance.clone()).await;
		}
		Commands::Location(location_cmd) => {
			return handle_location_daemon_command(location_cmd.clone(), cli.instance.clone())
				.await;
		}
		Commands::Job(job_cmd) => {
			return handle_job_daemon_command(job_cmd.clone(), cli.instance.clone()).await;
		}
		Commands::Network(network_cmd) => {
			return handle_network_daemon_command(network_cmd.clone(), cli.instance.clone()).await;
		}
		Commands::Copy(args) => {
			return handle_copy_daemon_command(args.clone(), cli.instance.clone()).await;
		}
		Commands::Monitor => {
			// Special case - monitor needs event streaming
			println!("📊 Job monitor not yet implemented for daemon mode");
			println!("   Use 'spacedrive job list' to see current jobs");
			return Ok(());
		}
		Commands::Logs { lines, follow } => {
			return handle_logs_command(*lines, *follow, cli.instance.clone()).await;
		}
		Commands::Index(cmd) => {
			println!("❌ Index command not yet implemented for daemon mode");
			println!("   This command will be available in a future update");
			return Ok(());
		}
		Commands::Scan { .. } => {
			println!("❌ Scan command not yet implemented for daemon mode");
			println!("   Use 'spacedrive location add' and 'spacedrive index' instead");
			return Ok(());
		}
		Commands::Status => {
			println!("❌ Status command not yet implemented for daemon mode");
			println!("   Use 'spacedrive daemon' to check daemon status");
			return Ok(());
		}
		Commands::Start { .. } | Commands::Stop | Commands::Daemon | Commands::Instance(_) => {
			// These are handled above, should never reach here
			unreachable!()
		}
	}

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
		println!(
			"⚠️  Spacedrive daemon instance '{}' is already running",
			instance_display
		);
		return Ok(());
	}

	println!("🚀 Starting Spacedrive daemon...");

	if foreground {
		// Run in foreground
		if enable_networking {
			// For networking enabled startup, we need a default password
			println!("🔐 Starting daemon with networking enabled...");
			println!("   Using master key for secure device authentication.");

			match daemon::Daemon::new_with_networking_and_instance(
				data_dir.clone(),
				instance_name.clone(),
			)
			.await
			{
				Ok(daemon) => daemon.start().await?,
				Err(e) => {
					println!("❌ Failed to start daemon with networking: {}", e);
					println!("   Falling back to daemon without networking...");
					let daemon =
						daemon::Daemon::new_with_instance(data_dir, instance_name.clone()).await?;
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
			println!(
				"✅ Spacedrive daemon instance '{}' started successfully",
				instance_display
			);
		} else {
			let instance_display = instance_name.as_deref().unwrap_or("default");
			println!(
				"❌ Failed to start Spacedrive daemon instance '{}'",
				instance_display
			);
		}
	}

	Ok(())
}

async fn handle_stop_daemon(
	instance_name: Option<String>,
) -> Result<(), Box<dyn std::error::Error>> {
	if !daemon::Daemon::is_running_instance(instance_name.clone()) {
		let instance_display = instance_name.as_deref().unwrap_or("default");
		println!(
			"⚠️  Spacedrive daemon instance '{}' is not running",
			instance_display
		);
		return Ok(());
	}

	let instance_display = instance_name.as_deref().unwrap_or("default");
	println!(
		"🛑 Stopping Spacedrive daemon instance '{}'...",
		instance_display
	);
	daemon::Daemon::stop_instance(instance_name.clone()).await?;

	// Wait a bit to ensure it's stopped
	tokio::time::sleep(std::time::Duration::from_secs(1)).await;

	if !daemon::Daemon::is_running_instance(instance_name.clone()) {
		println!(
			"✅ Spacedrive daemon instance '{}' stopped",
			instance_display
		);
	} else {
		println!(
			"❌ Failed to stop Spacedrive daemon instance '{}'",
			instance_display
		);
	}

	Ok(())
}

async fn handle_daemon_status(
	instance_name: Option<String>,
) -> Result<(), Box<dyn std::error::Error>> {
	use colored::Colorize;

	let instance_display = instance_name.as_deref().unwrap_or("default");

	if daemon::Daemon::is_running_instance(instance_name.clone()) {
		println!(
			"✅ Spacedrive daemon instance '{}' is running",
			instance_display
		);

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
		println!(
			"❌ Spacedrive daemon instance '{}' is not running",
			instance_display
		);
		if instance_name.is_some() {
			println!(
				"   Start it with: spacedrive --instance {} start",
				instance_display
			);
		} else {
			println!("   Start it with: spacedrive start");
		}
	}

	Ok(())
}

async fn handle_instance_command(cmd: InstanceCommands) -> Result<(), Box<dyn std::error::Error>> {
	use colored::Colorize;

	match cmd {
		InstanceCommands::List => match daemon::Daemon::list_instances() {
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
		},

		InstanceCommands::Stop { name } => {
			let instance_name = if name == "default" {
				None
			} else {
				Some(name.clone())
			};
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

	let mut client = daemon::DaemonClient::new_with_instance(instance_name.clone());

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

	let mut client = daemon::DaemonClient::new_with_instance(instance_name.clone());

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

	let mut client = daemon::DaemonClient::new_with_instance(instance_name.clone());

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
			// Parse the job ID string to UUID
			let job_id = match id.parse::<Uuid>() {
				Ok(uuid) => uuid,
				Err(_) => {
					println!("❌ Invalid job ID format");
					return Ok(());
				}
			};
			
			match client
				.send_command(daemon::DaemonCommand::GetJobInfo { id: job_id })
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
			monitoring::daemon_monitor::monitor_jobs(&mut client, job_id).await?;
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

	let mut client = daemon::DaemonClient::new_with_instance(instance_name.clone());

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
		commands::NetworkCommands::Init => {
			match client
				.send_command(daemon::DaemonCommand::InitNetworking)
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
			// Parse the device ID string to UUID
			let device_uuid = match device_id.parse::<Uuid>() {
				Ok(uuid) => uuid,
				Err(_) => {
					println!("❌ Invalid device ID format");
					return Ok(());
				}
			};
			
			match client
				.send_command(daemon::DaemonCommand::RevokeDevice { device_id: device_uuid })
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
			// Parse device_id to UUID
			let device_uuid = match device_id.parse::<Uuid>() {
				Ok(uuid) => uuid,
				Err(_) => {
					println!("❌ Invalid device ID format");
					return Ok(());
				}
			};
			
			// Use sender name or default
			let sender_name = sender.unwrap_or_else(|| "Anonymous".to_string());
			
			match client
				.send_command(daemon::DaemonCommand::SendSpacedrop {
					device_id: device_uuid,
					file_path: file_path.to_string_lossy().to_string(),
					sender_name,
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
			// Convert to PairingAction and use the networking commands handler
			let pairing_action = match action {
				commands::PairingCommands::Generate => {
					crate::infrastructure::cli::networking_commands::PairingAction::Generate
				}
				commands::PairingCommands::Join { code } => {
					// Code is already a String, not Optional
					crate::infrastructure::cli::networking_commands::PairingAction::Join { code }
				}
				commands::PairingCommands::Status => {
					crate::infrastructure::cli::networking_commands::PairingAction::Status
				}
				commands::PairingCommands::ListPending => {
					crate::infrastructure::cli::networking_commands::PairingAction::List
				}
				commands::PairingCommands::Accept { request_id } => {
					// Parse request_id to UUID
					let uuid = match request_id.parse::<Uuid>() {
						Ok(id) => id,
						Err(_) => {
							println!("❌ Invalid request ID format");
							return Ok(());
						}
					};
					crate::infrastructure::cli::networking_commands::PairingAction::Accept {
						request_id: uuid,
					}
				}
				commands::PairingCommands::Reject { request_id } => {
					// Parse request_id to UUID
					let uuid = match request_id.parse::<Uuid>() {
						Ok(id) => id,
						Err(_) => {
							println!("❌ Invalid request ID format");
							return Ok(());
						}
					};
					crate::infrastructure::cli::networking_commands::PairingAction::Reject {
						request_id: uuid,
					}
				}
			};

			crate::infrastructure::cli::networking_commands::handle_pairing_command(
				pairing_action,
				&client,
			)
			.await?;
		}
	}

	Ok(())
}

async fn handle_copy_daemon_command(
	args: adapters::FileCopyCliArgs,
	instance_name: Option<String>,
) -> Result<(), Box<dyn std::error::Error>> {
	use colored::Colorize;

	let mut client = daemon::DaemonClient::new_with_instance(instance_name.clone());

	// Convert CLI args to daemon command format
	let input = match args.validate_and_convert() {
		Ok(input) => input,
		Err(e) => {
			println!("❌ Invalid copy operation: {}", e);
			return Ok(());
		}
	};

	println!("📁 {}", input.summary().bright_cyan());

	// Send copy command to daemon
	match client
		.send_command(daemon::DaemonCommand::Copy {
			sources: input.sources.clone(),
			destination: input.destination.clone(),
			overwrite: input.overwrite,
			verify: input.verify_checksum,
			preserve_timestamps: input.preserve_timestamps,
			move_files: input.move_files,
		})
		.await
	{
		Ok(daemon::DaemonResponse::CopyStarted {
			job_id,
			sources_count,
		}) => {
			println!("✅ Copy operation started successfully!");
			println!("   Job ID: {}", job_id.to_string().bright_yellow());
			println!("   Sources: {} file(s)", sources_count);
			println!(
				"   Destination: {}",
				input.destination.display().to_string().bright_blue()
			);

			if input.overwrite {
				println!("   Mode: {} existing files", "Overwrite".bright_red());
			}
			if input.verify_checksum {
				println!("   Verification: {}", "Enabled".bright_green());
			}
			if input.move_files {
				println!(
					"   Type: {} (delete source after copy)",
					"Move".bright_yellow()
				);
			}

			println!(
				"\n💡 Tip: Monitor progress with: {}",
				"sd job monitor".bright_cyan()
			);
		}
		Ok(daemon::DaemonResponse::Ok) => {
			println!("✅ Copy operation completed successfully!");
		}
		Ok(daemon::DaemonResponse::Error(e)) => {
			println!("❌ Failed to copy files: {}", e);
		}
		Err(e) => {
			println!("❌ Failed to communicate with daemon: {}", e);
		}
		_ => {
			println!("❌ Unexpected response from daemon");
		}
	}

	Ok(())
}

async fn handle_logs_command(
	lines: usize,
	follow: bool,
	instance_name: Option<String>,
) -> Result<(), Box<dyn std::error::Error>> {
	use colored::Colorize;
	use std::fs::File;
	use std::io::{BufRead, BufReader, Seek, SeekFrom};
	use std::time::Duration;
	use tokio::time::sleep;

	// Get the daemon config to find the log file path
	let config = daemon::DaemonConfig::new(instance_name.clone());

	let log_file_path = config.log_file.ok_or("No log file configured for daemon")?;

	if !log_file_path.exists() {
		let instance_display = instance_name.as_deref().unwrap_or("default");
		println!(
			"❌ Log file not found for daemon instance '{}'",
			instance_display
		);
		println!("   Expected at: {}", log_file_path.display());
		println!("   Make sure the daemon is running with logging enabled");
		return Ok(());
	}

	println!(
		"📋 {} - Press Ctrl+C to exit",
		format!(
			"Spacedrive Daemon Logs ({})",
			instance_name.as_deref().unwrap_or("default")
		)
		.bright_cyan()
	);
	println!(
		"   Log file: {}",
		log_file_path.display().to_string().bright_blue()
	);
	println!("═══════════════════════════════════════════");

	// Read initial lines
	let file = File::open(&log_file_path)?;
	let reader = BufReader::new(file);
	let all_lines: Vec<String> = reader.lines().collect::<Result<Vec<_>, _>>()?;

	// Show last N lines
	let start_index = if all_lines.len() > lines {
		all_lines.len() - lines
	} else {
		0
	};

	for line in &all_lines[start_index..] {
		println!("{}", format_log_line(line));
	}

	if follow {
		// Follow mode - watch for new lines
		let mut file = File::open(&log_file_path)?;
		file.seek(SeekFrom::End(0))?;
		let mut reader = BufReader::new(file);

		loop {
			let mut line = String::new();
			match reader.read_line(&mut line) {
				Ok(0) => {
					// No new data, sleep and try again
					sleep(Duration::from_millis(100)).await;
				}
				Ok(_) => {
					// New line found
					print!("{}", format_log_line(&line));
				}
				Err(e) => {
					println!("❌ Error reading log file: {}", e);
					break;
				}
			}
		}
	}

	Ok(())
}

fn format_log_line(line: &str) -> String {
	use colored::Colorize;

	// Basic log formatting - colorize by log level
	if line.contains("ERROR") {
		line.red().to_string()
	} else if line.contains("WARN") {
		line.yellow().to_string()
	} else if line.contains("INFO") {
		line.normal().to_string()
	} else if line.contains("DEBUG") {
		line.bright_black().to_string()
	} else {
		line.to_string()
	}
}
