use anyhow::Result;
use clap::{Parser, Subcommand};
use sd_core::client::CoreClient;

mod context;
mod domains;
mod util;

use crate::context::{Context, OutputFormat};
use crate::domains::{
	file::{self, FileCmd},
	index::{self, IndexCmd},
	job::{self, JobCmd},
	library::{self, LibraryCmd},
	location::{self, LocationCmd},
	network::{self, NetworkCmd},
};

// OutputFormat is defined in context.rs and shared across domains

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
	},
	/// Stop the Spacedrive daemon
	Stop,
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
		data_dir.join("daemon").join(format!("daemon-{}.sock", inst))
	} else {
		data_dir.join("daemon/daemon.sock")
	};

	match cli.command {
		Commands::Start { enable_networking } => {
			println!("Starting daemon...");
			sd_core::infra::daemon::bootstrap::start_default_server(socket_path, data_dir, enable_networking).await.map_err(|e| anyhow::anyhow!(e.to_string()))?;
		}
		Commands::Stop => {
			println!("Stopping daemon...");
            let core = CoreClient::new(socket_path.clone());
            let _ = core.send_raw_request(&sd_core::infra::daemon::types::DaemonRequest::Shutdown).await.map_err(|e| anyhow::anyhow!(e.to_string()))?;
			println!("Daemon stopped.");
		}
		_ => {
			run_client_command(cli.command, cli.format, data_dir, socket_path).await?;
		}
	}

	Ok(())
}

async fn run_client_command(command: Commands, format: OutputFormat, data_dir: std::path::PathBuf, socket_path: std::path::PathBuf) -> Result<()> {
	let core = CoreClient::new(socket_path.clone());
	let ctx = Context::new(core, format, data_dir, socket_path);
	match command {
		Commands::Status => {
			let status: sd_core::ops::core::status::output::CoreStatus = ctx
				.core
				.query(&sd_core::ops::core::status::query::CoreStatusQuery)
				.await?;
			match ctx.format {
				OutputFormat::Human => println!(
					"Spacedrive Core {} (libraries: {})",
					status.version, status.library_count
				),
				OutputFormat::Json => println!("{}", serde_json::to_string_pretty(&status)?),
			}
		}
		Commands::Library(cmd) => library::run(&ctx, cmd).await?,
		Commands::File(cmd) => file::run(&ctx, cmd).await?,
		Commands::Index(cmd) => index::run(&ctx, cmd).await?,
		Commands::Location(cmd) => location::run(&ctx, cmd).await?,
		Commands::Network(cmd) => network::run(&ctx, cmd).await?,
		Commands::Job(cmd) => job::run(&ctx, cmd).await?,
		_ => {} // Start and Stop are handled in main
	}
	Ok(())
}
