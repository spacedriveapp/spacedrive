pub mod adapters;
pub mod commands;
pub mod daemon;
pub mod monitoring;
pub mod output;
pub mod pairing_ui;
pub mod state;
pub mod utils;

use crate::infrastructure::cli::commands::{
	daemon::{handle_daemon_command, DaemonCommands},
	file::{handle_file_command, FileCommands},
	job::{handle_job_command, JobCommands},
	library::{handle_library_command, LibraryCommands},
	location::{handle_location_command, LocationCommands},
	network::{handle_network_command, NetworkCommands},
	system::{handle_system_command, SystemCommands},
};
use crate::infrastructure::cli::output::{CliOutput, Message};
use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "spacedrive")]
#[command(about = "Spacedrive v2 CLI", long_about = None)]
pub struct Cli {
	/// Path to Spacedrive data directory
	#[arg(short, long, global = true)]
	pub data_dir: Option<PathBuf>,

	/// Enable debug logging (can be used multiple times for more verbosity)
	#[arg(short = 'v', long, global = true, action = clap::ArgAction::Count)]
	pub verbose: u8,

	/// Output format
	#[arg(short = 'f', long, global = true, value_enum, default_value = "human")]
	pub format: OutputFormatArg,

	/// Disable colors and emojis in output
	#[arg(long, global = true)]
	pub no_color: bool,

	/// Suppress all output except errors
	#[arg(short = 'q', long, global = true)]
	pub quiet: bool,

	/// Daemon instance name (for multiple daemon support)
	#[arg(long, global = true)]
	pub instance: Option<String>,

	#[command(subcommand)]
	pub command: Commands,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, clap::ValueEnum)]
pub enum OutputFormatArg {
	Human,
	Json,
}

#[derive(Subcommand)]
pub enum Commands {
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

	/// Check if the daemon is running and show status
	Status,

	/// Daemon lifecycle management (advanced)
	#[command(subcommand)]
	Daemon(DaemonCommands),

	/// Library management
	#[command(subcommand)]
	Library(LibraryCommands),

	/// Location management
	#[command(subcommand)]
	Location(LocationCommands),

	/// Job management and monitoring
	#[command(subcommand)]
	Job(JobCommands),

	/// Network operations and device management
	#[command(subcommand)]
	Network(NetworkCommands),

	/// File operations
	#[command(subcommand)]
	File(FileCommands),

	/// System monitoring and information
	#[command(subcommand)]
	System(SystemCommands),
}

pub async fn run() -> Result<(), Box<dyn std::error::Error>> {
	let cli = Cli::parse();

	// Set up logging - skip for daemon start commands as they handle their own logging
	let is_daemon_start = matches!(
		&cli.command,
		Commands::Start { .. } | Commands::Daemon(DaemonCommands::Start { .. })
	);
	if !is_daemon_start {
		let log_level = match cli.verbose {
			0 => "info",
			1 => "debug",
			_ => "trace",
		};
		let env_filter =
			tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| {
				// Fallback to hardcoded filters if RUST_LOG not set
				if cli.verbose > 0 {
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

	// Set up output context
	use output::{CliOutput, ColorMode, OutputFormat, VerbosityLevel};

	let output_format = if cli.quiet {
		OutputFormat::Quiet
	} else {
		match cli.format {
			OutputFormatArg::Human => OutputFormat::Human,
			OutputFormatArg::Json => OutputFormat::Json,
		}
	};

	let color_mode = if cli.no_color {
		ColorMode::Never
	} else {
		ColorMode::Auto
	};

	let verbosity = VerbosityLevel::from_occurrences(cli.verbose);

	let mut output = CliOutput::with_options(output_format, verbosity, color_mode);

	// Determine data directory with instance isolation
	let base_data_dir = cli
		.data_dir
		.unwrap_or_else(|| PathBuf::from("./data/spacedrive-cli-data"));

	let data_dir = if let Some(ref instance) = cli.instance {
		base_data_dir.join(format!("instance-{}", instance))
	} else {
		base_data_dir
	};

	// Route to appropriate domain handler
	match &cli.command {
		Commands::Start {
			foreground,
			enable_networking,
		} => {
			// Handle start command
			handle_daemon_command(
				DaemonCommands::Start {
					foreground: *foreground,
					enable_networking: *enable_networking,
				},
				data_dir,
				cli.instance.clone(),
				output,
			)
			.await
		}
		Commands::Stop => {
			// Handle stop command
			handle_daemon_command(DaemonCommands::Stop, data_dir, cli.instance.clone(), output)
				.await
		}
		Commands::Status => {
			// Handle status command
			handle_daemon_command(
				DaemonCommands::Status,
				data_dir,
				cli.instance.clone(),
				output,
			)
			.await
		}
		Commands::Daemon(daemon_cmd) => {
			// Daemon commands don't need daemon to be running
			handle_daemon_command(daemon_cmd.clone(), data_dir, cli.instance.clone(), output).await
		}
		Commands::Library(library_cmd) => {
			// Check if daemon is running
			if !daemon::Daemon::is_running_instance(cli.instance.clone()) {
				print_daemon_not_running(&cli.instance, &mut output)?;
				return Ok(());
			}
			handle_library_command(library_cmd.clone(), cli.instance.clone(), output).await
		}
		Commands::Location(location_cmd) => {
			// Check if daemon is running
			if !daemon::Daemon::is_running_instance(cli.instance.clone()) {
				print_daemon_not_running(&cli.instance, &mut output)?;
				return Ok(());
			}
			handle_location_command(location_cmd.clone(), cli.instance.clone(), output).await
		}
		Commands::Job(job_cmd) => {
			// Check if daemon is running
			if !daemon::Daemon::is_running_instance(cli.instance.clone()) {
				print_daemon_not_running(&cli.instance, &mut output)?;
				return Ok(());
			}
			handle_job_command(job_cmd.clone(), cli.instance.clone(), output).await
		}
		Commands::Network(network_cmd) => {
			// Check if daemon is running (except for init)
			if !matches!(network_cmd, NetworkCommands::Init) {
				if !daemon::Daemon::is_running_instance(cli.instance.clone()) {
					print_daemon_not_running(&cli.instance, &mut output)?;
					return Ok(());
				}
			}
			handle_network_command(network_cmd.clone(), cli.instance.clone(), output).await
		}
		Commands::File(file_cmd) => {
			// Check if daemon is running
			if !daemon::Daemon::is_running_instance(cli.instance.clone()) {
				print_daemon_not_running(&cli.instance, &mut output)?;
				return Ok(());
			}
			handle_file_command(file_cmd.clone(), cli.instance.clone(), output).await
		}
		Commands::System(system_cmd) => {
			// System commands may or may not need daemon depending on the command
			match system_cmd {
				SystemCommands::Logs { .. } => {
					// Logs command doesn't need daemon to be running
					handle_system_command(system_cmd.clone(), cli.instance.clone(), output).await
				}
				_ => {
					// Other system commands need daemon
					if !daemon::Daemon::is_running_instance(cli.instance.clone()) {
						print_daemon_not_running(&cli.instance, &mut output)?;
						return Ok(());
					}
					handle_system_command(system_cmd.clone(), cli.instance.clone(), output).await
				}
			}
		}
	}
}

fn print_daemon_not_running(
	instance_name: &Option<String>,
	output: &mut CliOutput,
) -> Result<(), Box<dyn std::error::Error>> {
	let instance_display = instance_name.as_deref().unwrap_or("default");
	output.error(Message::DaemonNotRunning {
		instance: instance_display.to_string(),
	})?;

	let start_cmd = if instance_name.is_some() {
		format!("spacedrive --instance {} start", instance_display)
	} else {
		"spacedrive start".to_string()
	};

	output
		.section()
		.help()
		.item(&format!("Start it with: {}", start_cmd))
		.render()?;

	Ok(())
}
