use anyhow::Result;
use clap::{Parser, Subcommand, ValueEnum};
use sd_core::client::CoreClient;

#[derive(Debug, Clone, ValueEnum)]
enum OutputFormat {
	Human,
	Json,
}

#[derive(Parser, Debug)]
#[command(name = "spacedrive", about = "Spacedrive v2 CLI (daemon client)")]
struct Cli {
	/// Path to spacedrive data directory
	#[arg(long)]
	data_dir: Option<std::path::PathBuf>,

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
	Library(LibraryCommands),
	/// File operations
	#[command(subcommand)]
	File(FileCommands),
}

#[derive(Subcommand, Debug)]
enum LibraryCommands {
	/// List libraries
	List,
}

#[derive(Subcommand, Debug)]
enum FileCommands {
	/// Copy files
	Copy(FileCopyArgs),
}

#[derive(Parser, Debug, Clone)]
struct FileCopyArgs {
	/// Source files or directories to copy (one or more)
	pub sources: Vec<std::path::PathBuf>,

	/// Destination path
	#[arg(long)]
	pub destination: std::path::PathBuf,

	/// Overwrite existing files
	#[arg(long, default_value_t = false)]
	pub overwrite: bool,

	/// Verify checksums during copy
	#[arg(long, default_value_t = false)]
	pub verify_checksum: bool,

	/// Preserve file timestamps
	#[arg(long, default_value_t = true)]
	pub preserve_timestamps: bool,

	/// Delete source files after copy (move)
	#[arg(long, default_value_t = false)]
	pub move_files: bool,
}

impl FileCopyArgs {
	fn to_input(&self) -> sd_core::ops::files::copy::input::FileCopyInput {
		use sd_core::ops::files::copy::input::{CopyMethod, FileCopyInput};
		FileCopyInput {
			library_id: None,
			sources: self.sources.clone(),
			destination: self.destination.clone(),
			overwrite: self.overwrite,
			verify_checksum: self.verify_checksum,
			preserve_timestamps: self.preserve_timestamps,
			move_files: self.move_files,
			copy_method: CopyMethod::Auto,
		}
	}
}

#[tokio::main]
async fn main() -> Result<()> {
	let cli = Cli::parse();
	let data_dir = cli.data_dir.unwrap_or(sd_core::config::default_data_dir()?);
	let socket = data_dir.join("daemon/daemon.sock");
	let core = CoreClient::new(socket);

	match cli.command {
		Commands::Status => {
			let status: sd_core::ops::core::status::output::CoreStatus = core
				.query(&sd_core::ops::core::status::query::CoreStatusQuery)
				.await?;
			match cli.format {
				OutputFormat::Human => println!(
					"Spacedrive Core {} (libraries: {})",
					status.version, status.library_count
				),
				OutputFormat::Json => println!("{}", serde_json::to_string_pretty(&status)?),
			}
		}
		Commands::Library(LibraryCommands::List) => {
			let libs: Vec<sd_core::ops::libraries::list::output::LibraryInfo> = core
				.query(&sd_core::ops::libraries::list::query::ListLibrariesQuery::basic())
				.await?;
			match cli.format {
				OutputFormat::Human => {
					if libs.is_empty() {
						println!("No libraries found");
					}
					for l in libs {
						println!("- {} {}", l.id, l.path.display());
					}
				}
				OutputFormat::Json => println!("{}", serde_json::to_string_pretty(&libs)?),
			}
		}
		Commands::File(FileCommands::Copy(args)) => {
			let input = args.to_input();
			// Basic validation via core input
			if let Err(errors) = input.validate() {
				anyhow::bail!(errors.join("; "));
			}
			let _bytes = core.action(&input).await?;
			println!("Copy request submitted");
		}
	}

	Ok(())
}
