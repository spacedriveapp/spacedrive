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
	let socket = if let Some(inst) = cli.instance {
		data_dir.join("daemon").join(format!("daemon-{}.sock", inst))
	} else {
		data_dir.join("daemon/daemon.sock")
	};
	let core = CoreClient::new(socket);

	let ctx = Context::new(core, cli.format);

	match cli.command {
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
	}

	Ok(())
}
