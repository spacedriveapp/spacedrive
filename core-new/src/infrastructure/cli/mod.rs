pub mod adapters;
pub mod daemon;
pub mod domains;
pub mod monitoring;
pub mod networking_commands;
pub mod pairing_ui;
pub mod state;
pub mod utils;

use crate::infrastructure::cli::domains::{
    daemon::{handle_daemon_command, DaemonCommands},
    library::{handle_library_command, LibraryCommands},
    location::{handle_location_command, LocationCommands},
    job::{handle_job_command, JobCommands},
    network::{handle_network_command, NetworkCommands},
    file::{handle_file_command, FileCommands},
    system::{handle_system_command, SystemCommands},
};
use clap::{Parser, Subcommand};
use std::path::PathBuf;

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
    /// Daemon lifecycle management
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
    let is_daemon_start = matches!(&cli.command, Commands::Daemon(DaemonCommands::Start { .. }));
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

    // Route to appropriate domain handler
    match &cli.command {
        Commands::Daemon(daemon_cmd) => {
            // Daemon commands don't need daemon to be running
            handle_daemon_command(daemon_cmd.clone(), data_dir, cli.instance.clone()).await
        }
        Commands::Library(library_cmd) => {
            // Check if daemon is running
            if !daemon::Daemon::is_running_instance(cli.instance.clone()) {
                print_daemon_not_running(&cli.instance);
                return Ok(());
            }
            handle_library_command(library_cmd.clone(), cli.instance.clone()).await
        }
        Commands::Location(location_cmd) => {
            // Check if daemon is running
            if !daemon::Daemon::is_running_instance(cli.instance.clone()) {
                print_daemon_not_running(&cli.instance);
                return Ok(());
            }
            handle_location_command(location_cmd.clone(), cli.instance.clone()).await
        }
        Commands::Job(job_cmd) => {
            // Check if daemon is running
            if !daemon::Daemon::is_running_instance(cli.instance.clone()) {
                print_daemon_not_running(&cli.instance);
                return Ok(());
            }
            handle_job_command(job_cmd.clone(), cli.instance.clone()).await
        }
        Commands::Network(network_cmd) => {
            // Check if daemon is running (except for init)
            if !matches!(network_cmd, NetworkCommands::Init) {
                if !daemon::Daemon::is_running_instance(cli.instance.clone()) {
                    print_daemon_not_running(&cli.instance);
                    return Ok(());
                }
            }
            handle_network_command(network_cmd.clone(), cli.instance.clone()).await
        }
        Commands::File(file_cmd) => {
            // Check if daemon is running
            if !daemon::Daemon::is_running_instance(cli.instance.clone()) {
                print_daemon_not_running(&cli.instance);
                return Ok(());
            }
            handle_file_command(file_cmd.clone(), cli.instance.clone()).await
        }
        Commands::System(system_cmd) => {
            // System commands may or may not need daemon depending on the command
            match system_cmd {
                SystemCommands::Logs { .. } => {
                    // Logs command doesn't need daemon to be running
                    handle_system_command(system_cmd.clone(), cli.instance.clone()).await
                }
                _ => {
                    // Other system commands need daemon
                    if !daemon::Daemon::is_running_instance(cli.instance.clone()) {
                        print_daemon_not_running(&cli.instance);
                        return Ok(());
                    }
                    handle_system_command(system_cmd.clone(), cli.instance.clone()).await
                }
            }
        }
    }
}

fn print_daemon_not_running(instance_name: &Option<String>) {
    let instance_display = instance_name.as_deref().unwrap_or("default");
    println!(
        "❌ Spacedrive daemon instance '{}' is not running",
        instance_display
    );
    if instance_name.is_some() {
        println!(
            "   Start it with: spacedrive --instance {} daemon start",
            instance_display
        );
    } else {
        println!("   Start it with: spacedrive daemon start");
    }
}